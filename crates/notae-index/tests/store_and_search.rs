//! Integration tests for the SQLite schema, record CRUD, and FTS5 search.
#![allow(clippy::expect_used)] // Test fixtures and assertions may panic on setup failure.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use notae_domain::{
    EpochTimestamp, IndexStatus, IndexedNote, MetadataField, MetadataOrigin, NoteId, NoteMetadata,
    PdfFingerprint, RelativePdfPath, Sha256Digest,
};
use notae_index::{
    IndexError, IndexStore, MIGRATIONS, OcrText, ReindexOutcome, SCHEMA_VERSION, SearchScope,
};

fn digest(seed: u8) -> Sha256Digest {
    Sha256Digest::from_bytes([seed; 32])
}

fn path(value: &str) -> RelativePdfPath {
    RelativePdfPath::new(value).expect("valid relative path")
}

fn note(
    relative: &str,
    seed: u8,
    id: Option<NoteId>,
    title: &str,
    composer: Option<&str>,
    tags: &[&str],
) -> IndexedNote {
    let mut metadata = NoteMetadata::new(
        title,
        composer.map(str::to_owned),
        tags.iter().map(|tag| (*tag).to_owned()),
    )
    .expect("valid title");
    metadata.assign_id(id, MetadataOrigin::FilenameDerived);
    let fingerprint = PdfFingerprint::new(digest(seed), 1_024 + u64::from(seed), None);
    IndexedNote::new(path(relative), fingerprint, metadata)
}

fn temp_db(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "notae-index-{label}-{}-{nanos}.sqlite",
        std::process::id()
    ))
}

fn keep_only(paths: &[RelativePdfPath]) -> HashSet<RelativePdfPath> {
    paths.iter().cloned().collect()
}

#[test]
fn open_creates_schema_at_the_current_version() {
    let file = temp_db("schema");
    let store = IndexStore::open(&file).expect("open creates the database");

    assert_eq!(store.schema_version().expect("version"), SCHEMA_VERSION);

    drop(store);
    let conn = rusqlite::Connection::open(&file).expect("reopen raw");
    for table in ["schema_migrations", "notes", "notes_fts"] {
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = ?1",
                [table],
                |row| row.get(0),
            )
            .expect("query sqlite_master");
        assert_eq!(count, 1, "expected table {table} to exist");
    }
    let _ = std::fs::remove_file(&file);
}

#[test]
fn reopening_an_existing_database_reapplies_no_migration() {
    let file = temp_db("reopen");
    let store = IndexStore::open(&file).expect("open creates the database");
    store
        .upsert_note(&note("a.pdf", 1, Some(NoteId::new(3)), "Aria", None, &[]))
        .expect("upsert");
    drop(store);

    let reopened = IndexStore::open(&file).expect("reopen applies nothing new");
    assert_eq!(reopened.schema_version().expect("version"), SCHEMA_VERSION);
    assert!(
        reopened
            .get_by_id(NoteId::new(3))
            .expect("lookup")
            .is_some()
    );
    drop(reopened);
    let _ = std::fs::remove_file(&file);
}

#[test]
fn opening_a_database_from_a_newer_schema_is_refused() {
    let file = temp_db("newer");
    let conn = rusqlite::Connection::open(&file).expect("create raw");
    conn.execute_batch(
        "CREATE TABLE schema_migrations (
             version INTEGER PRIMARY KEY,
             applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         );
         INSERT INTO schema_migrations (version) VALUES (99);",
    )
    .expect("seed a future version");
    drop(conn);

    let error = IndexStore::open(&file).expect_err("a newer schema must not be downgraded");
    match error {
        IndexError::UnsupportedSchemaVersion { found, supported } => {
            assert_eq!(found, 99);
            assert_eq!(supported, SCHEMA_VERSION);
        }
        other => panic!("unexpected error: {other}"),
    }
    let _ = std::fs::remove_file(&file);
}

#[test]
fn metadata_provenance_and_status_round_trip() {
    let store = IndexStore::open(":memory:").expect("open in memory");
    let mut metadata = NoteMetadata::new(
        "Autumn Leaves",
        Some("Erroll Garner".to_owned()),
        ["jazz".to_owned(), "medium tempo".to_owned()],
    )
    .expect("valid title");
    metadata.assign_id(Some(NoteId::new(12)), MetadataOrigin::FilenameDerived);
    metadata.set_title("Autumn Leaves (take 2)").expect("valid");
    metadata.add_tag("transcription");
    metadata.mark_manual(MetadataField::ComposerOrArtist);
    let fingerprint = PdfFingerprint::new(digest(7), 4_096, EpochTimestamp::new(-12, 250));
    let mut stored = IndexedNote::new(path("scores/12 - Autumn Leaves.pdf"), fingerprint, metadata);
    stored.set_status(IndexStatus::OcrIncomplete);

    store.upsert_note(&stored).expect("upsert");

    let loaded = store
        .get_by_hash(&digest(7))
        .expect("lookup")
        .expect("record exists");
    assert_eq!(loaded.path(), stored.path());
    assert_eq!(loaded.fingerprint(), stored.fingerprint());
    assert_eq!(loaded.metadata(), stored.metadata());
    assert_eq!(loaded.metadata().title(), "Autumn Leaves (take 2)");
    assert_eq!(loaded.id(), Some(NoteId::new(12)));
    assert_eq!(loaded.status(), IndexStatus::OcrIncomplete);
    assert_eq!(
        loaded.metadata().origin_of(MetadataField::Title),
        MetadataOrigin::Manual
    );
    assert_eq!(
        loaded.metadata().origin_of(MetadataField::Id),
        MetadataOrigin::FilenameDerived
    );
    assert_eq!(
        loaded.metadata().origin_of(MetadataField::ComposerOrArtist),
        MetadataOrigin::Manual
    );
    assert_eq!(
        loaded.metadata().origin_of(MetadataField::Tags),
        MetadataOrigin::Manual
    );
    assert!(
        loaded.metadata().tags().contains("transcription"),
        "manually added tag must survive"
    );
}

#[test]
fn update_replaces_the_stored_record() {
    let store = IndexStore::open(":memory:").expect("open in memory");
    let first = note("a.pdf", 1, Some(NoteId::new(4)), "Prelude", None, &[]);
    store.upsert_note(&first).expect("insert");

    let mut second = note(
        "a.pdf",
        1,
        Some(NoteId::new(4)),
        "Prelude in C",
        None,
        &["piano"],
    );
    second.set_status(IndexStatus::Ready);
    store.upsert_note(&second).expect("update");

    assert_eq!(store.list_all().expect("list").len(), 1);
    let loaded = store
        .get_by_hash(&digest(1))
        .expect("lookup")
        .expect("record exists");
    assert_eq!(loaded.title(), "Prelude in C");
    assert_eq!(loaded.status(), IndexStatus::Ready);
    assert!(loaded.metadata().tags().contains("piano"));
}

#[test]
fn duplicate_note_ids_are_refused_with_the_contested_id() {
    let store = IndexStore::open(":memory:").expect("open in memory");
    store
        .upsert_note(&note("a.pdf", 1, Some(NoteId::new(9)), "First", None, &[]))
        .expect("first record claims id 9");

    let error = store
        .upsert_note(&note("b.pdf", 2, Some(NoteId::new(9)), "Second", None, &[]))
        .expect_err("a duplicate id must not be silently hidden");

    match error {
        IndexError::DuplicateNoteId(id) => assert_eq!(id, NoteId::new(9)),
        other => panic!("unexpected error: {other}"),
    }
    assert_eq!(store.list_all().expect("list").len(), 1);
    assert!(
        store.get_by_hash(&digest(2)).expect("lookup").is_none(),
        "the rejected record must not be stored"
    );
}

#[test]
fn records_without_an_id_do_not_conflict() {
    let store = IndexStore::open(":memory:").expect("open in memory");
    store
        .upsert_note(&note("a.pdf", 1, None, "Unnumbered A", None, &[]))
        .expect("insert");
    store
        .upsert_note(&note("b.pdf", 2, None, "Unnumbered B", None, &[]))
        .expect("insert");

    assert_eq!(store.list_all().expect("list").len(), 2);
}

#[test]
fn a_new_path_for_the_same_hash_updates_one_record() {
    let store = IndexStore::open(":memory:").expect("open in memory");
    store
        .upsert_note(&note(
            "old/12 - Autumn Leaves.pdf",
            5,
            Some(NoteId::new(12)),
            "Autumn Leaves",
            None,
            &[],
        ))
        .expect("insert");

    store
        .upsert_note(&note(
            "new/12 - Autumn Leaves.pdf",
            5,
            Some(NoteId::new(12)),
            "Autumn Leaves",
            None,
            &[],
        ))
        .expect("move");

    let notes = store.list_all().expect("list");
    assert_eq!(notes.len(), 1, "a move must not create a second record");
    assert_eq!(notes[0].path(), &path("new/12 - Autumn Leaves.pdf"));
}

#[test]
fn search_matches_title_tags_filename_and_ocr_text() {
    let store = IndexStore::open(":memory:").expect("open in memory");
    store
        .upsert_note(&note(
            "songs/12 - Autumn Leaves.pdf",
            1,
            Some(NoteId::new(12)),
            "Autumn Leaves",
            Some("Erroll Garner"),
            &["jazz"],
        ))
        .expect("insert");
    store
        .upsert_note(&note(
            "etudes/Sonatina.pdf",
            2,
            Some(NoteId::new(31)),
            "Sonatina",
            Some("Dezille"),
            &["classical"],
        ))
        .expect("insert");
    store
        .upsert_note_with_ocr(
            &note(
                "misc/44 - Scans.pdf",
                3,
                Some(NoteId::new(44)),
                "Scans",
                None,
                &[],
            ),
            Some(&OcrText::new("chromatic  double  sharp   exercise")),
        )
        .expect("insert with ocr");

    for query in [
        "Autumn",
        "Autu",
        "jazz",
        "Sonatina",
        "Sonatina.pdf",
        "chromatic",
        "sharp",
    ] {
        let hits = store.search(query, 10).expect("search");
        assert!(!hits.is_empty(), "expected a match for {query}");
        assert!(
            hits.iter().any(|hit| match query {
                "Autumn" | "Autu" => hit.note.id() == Some(NoteId::new(12)),
                "jazz" => hit.note.id() == Some(NoteId::new(12)),
                "Sonatina" | "Sonatina.pdf" => hit.note.id() == Some(NoteId::new(31)),
                _ => hit.note.id() == Some(NoteId::new(44)),
            }),
            "expected a match for {query}"
        );
    }

    let composer = store.search("Erroll", 10).expect("search");
    assert_eq!(composer.len(), 1);
    assert_eq!(composer[0].note.id(), Some(NoteId::new(12)));
    assert_eq!(store.search("nothinghere", 10).expect("search").len(), 0);
    assert_eq!(store.search("   ", 10).expect("search").len(), 0);
    assert_eq!(store.search("Autumn", 0).expect("search").len(), 0);
}

#[test]
fn an_exact_title_match_ranks_above_an_ocr_only_match() {
    let store = IndexStore::open(":memory:").expect("open in memory");
    store
        .upsert_note(&note(
            "occasions/12 - Etude.pdf",
            1,
            Some(NoteId::new(12)),
            "Etude",
            None,
            &[],
        ))
        .expect("insert");
    store
        .upsert_note(&note(
            "pieces/Etude in E minor.pdf",
            2,
            Some(NoteId::new(88)),
            "Etude in E minor",
            None,
            &[],
        ))
        .expect("insert");
    store
        .upsert_note_with_ocr(
            &note("misc/handwriting.pdf", 3, None, "Unlabelled", None, &[]),
            Some(&OcrText::new("etude studies for the whole etude sequence")),
        )
        .expect("insert with ocr");

    let hits = store.search("Etude", 10).expect("search");
    assert_eq!(hits.len(), 3, "every record mentions the term");
    assert_eq!(hits[0].note.title(), "Etude", "exact title first");
    assert_eq!(
        hits[1].note.title(),
        "Etude in E minor",
        "a title field hit outranks printed text"
    );
    assert_eq!(hits[2].note.title(), "Unlabelled", "ocr-only match last");
    assert!(hits[0].rank > hits[1].rank);
    assert!(hits[1].rank > hits[2].rank);
}

#[test]
fn an_exact_id_match_outranks_an_exact_title_match() {
    let store = IndexStore::open(":memory:").expect("open in memory");
    // The second record is titled `12` on purpose: for the query `12` it is an
    // exact title match, which must still lose to the record whose number is 12.
    store
        .upsert_note(&note(
            "occasions/12 - Etudes.pdf",
            1,
            Some(NoteId::new(12)),
            "12 Etudes",
            None,
            &[],
        ))
        .expect("insert");
    store
        .upsert_note(&note(
            "pieces/twelve.pdf",
            2,
            Some(NoteId::new(88)),
            "12",
            None,
            &[],
        ))
        .expect("insert");
    store
        .upsert_note_with_ocr(
            &note("misc/scans.pdf", 3, None, "Unlabelled", None, &[]),
            Some(&OcrText::new("footer of page 12")),
        )
        .expect("insert with ocr");

    let hits = store.search("12", 10).expect("search");
    assert_eq!(hits.len(), 3, "every record mentions the number");
    assert_eq!(hits[0].note.id(), Some(NoteId::new(12)), "exact id first");
    assert_eq!(
        hits[1].note.id(),
        Some(NoteId::new(88)),
        "exact title second"
    );
    assert_eq!(hits[2].note.id(), None, "ocr-only match last");
    assert!(hits[0].rank > hits[1].rank);
    assert!(hits[1].rank > hits[2].rank);
}

#[test]
fn ocr_text_is_stored_separately_and_survives_later_writes() {
    let store = IndexStore::open(":memory:").expect("open in memory");
    let record = note(
        "misc/5 - Scans.pdf",
        8,
        Some(NoteId::new(5)),
        "Scans",
        None,
        &[],
    );
    store
        .upsert_note_with_ocr(
            &record,
            Some(&OcrText::with_language("Page one text", "eng")),
        )
        .expect("insert with ocr");
    assert_eq!(
        store.ocr_text(&digest(8)).expect("read ocr"),
        Some(OcrText::with_language("Page one text", "eng"))
    );

    store.upsert_note(&record).expect("metadata-only upsert");
    assert_eq!(
        store.ocr_text(&digest(8)).expect("read ocr"),
        Some(OcrText::with_language("Page one text", "eng")),
        "an upsert that says nothing about ocr must keep the stored text"
    );

    store
        .set_ocr(&digest(8), &OcrText::new("replaced text"))
        .expect("replace ocr");
    assert_eq!(
        store.ocr_text(&digest(8)).expect("read ocr"),
        Some(OcrText::new("replaced text"))
    );

    let error = store
        .set_ocr(&digest(99), &OcrText::new("orphan"))
        .expect_err("ocr for an unknown hash must fail");
    assert!(matches!(error, IndexError::UnknownNote { .. }), "{error}");

    assert!(
        store
            .search("replaced", 10)
            .expect("search")
            .iter()
            .any(|hit| hit.note.id() == Some(NoteId::new(5))),
        "replaced text must be searchable"
    );
}

#[test]
fn delete_missing_leaves_only_the_supplied_paths() {
    let store = IndexStore::open(":memory:").expect("open in memory");
    store
        .upsert_note(&note("a.pdf", 1, Some(NoteId::new(1)), "A", None, &[]))
        .expect("insert");
    store
        .upsert_note(&note("b.pdf", 2, Some(NoteId::new(2)), "B", None, &[]))
        .expect("insert");
    store
        .upsert_note(&note("sub/c.pdf", 3, Some(NoteId::new(3)), "C", None, &[]))
        .expect("insert");

    store
        .delete_missing(&keep_only(&[path("sub/c.pdf")]))
        .expect("prune");

    let remaining = store.list_all().expect("list");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].path(), &path("sub/c.pdf"));
    assert_eq!(
        store.search("A", 10).expect("search").len(),
        0,
        "pruned rows must leave the fts index too"
    );
}

#[test]
fn list_all_uses_the_library_order() {
    let store = IndexStore::open(":memory:").expect("open in memory");
    store
        .upsert_note(&note("ten.pdf", 1, Some(NoteId::new(10)), "Ten", None, &[]))
        .expect("insert");
    store
        .upsert_note(&note("two.pdf", 2, Some(NoteId::new(2)), "Two", None, &[]))
        .expect("insert");
    store
        .upsert_note(&note("beta.pdf", 3, None, "beta", None, &[]))
        .expect("insert");
    store
        .upsert_note(&note("alpha.pdf", 4, None, "Alpha", None, &[]))
        .expect("insert");

    let order: Vec<Option<NoteId>> = store
        .list_all()
        .expect("list")
        .iter()
        .map(IndexedNote::id)
        .collect();
    assert_eq!(
        order,
        vec![Some(NoteId::new(2)), Some(NoteId::new(10)), None, None,]
    );
    let listed = store.list_all().expect("list");
    let titles: Vec<&str> = listed.iter().map(IndexedNote::title).collect();
    assert_eq!(titles[2], "Alpha", "unnumbered records sort by title");
    assert_eq!(titles[3], "beta");
}

#[test]
fn reindexing_changed_content_at_a_known_path_keeps_one_record() {
    let store = IndexStore::open(":memory:").expect("open in memory");
    let mut first = note("a.pdf", 1, Some(NoteId::new(4)), "Prelude", None, &[]);
    first.set_status(IndexStatus::Ready);
    store
        .upsert_note(&first)
        .expect("insert the original content");
    store
        .set_ocr(
            &digest(1),
            &OcrText::new("printed text of the first revision"),
        )
        .expect("ocr the original");

    let mut second = note("a.pdf", 2, Some(NoteId::new(4)), "Prelude", None, &[]);
    second.set_status(IndexStatus::OcrPending);
    let outcome = store
        .reindex_note(&second)
        .expect("reindexing changed content at a known path");

    assert_eq!(
        outcome,
        ReindexOutcome::Replaced {
            previous: digest(1),
            duplicate_at: None,
        },
        "a same-path content change is a replacement, not a move"
    );
    let notes = store.list_all().expect("list");
    assert_eq!(notes.len(), 1, "the changed file keeps exactly one record");
    assert_eq!(
        notes[0].fingerprint().sha256(),
        digest(2),
        "the record must carry the new fingerprint"
    );
    assert_eq!(notes[0].id(), Some(NoteId::new(4)), "metadata is retained");
    assert_eq!(notes[0].title(), "Prelude");
    assert_eq!(notes[0].status(), IndexStatus::OcrPending);
    assert!(
        store.get_by_hash(&digest(1)).expect("lookup").is_none(),
        "the stale fingerprint must not survive"
    );
    assert_eq!(
        store.ocr_text(&digest(2)).expect("read ocr"),
        Some(OcrText::new("")),
        "recognized text of the replaced content does not describe the new bytes"
    );
    assert!(
        store.search("printed", 10).expect("search").is_empty(),
        "the derived index must not keep the replaced content"
    );
    assert_eq!(
        store
            .search("Prelude", 10)
            .expect("search")
            .first()
            .map(|hit| hit.note.fingerprint().sha256()),
        Some(digest(2)),
        "the derived index must point at the new content"
    );
}

#[test]
fn reindexing_distinguishes_a_move_from_a_content_change() {
    let store = IndexStore::open(":memory:").expect("open in memory");
    store
        .upsert_note(&note(
            "old/12 - Autumn Leaves.pdf",
            5,
            Some(NoteId::new(12)),
            "Autumn Leaves",
            None,
            &[],
        ))
        .expect("insert");
    store
        .upsert_note(&note("b.pdf", 6, Some(NoteId::new(13)), "B", None, &[]))
        .expect("insert");

    assert_eq!(
        store
            .reindex_note(&note(
                "new/12 - Autumn Leaves.pdf",
                5,
                Some(NoteId::new(12)),
                "Autumn Leaves",
                None,
                &[],
            ))
            .expect("same hash at a new path"),
        ReindexOutcome::Moved {
            from: path("old/12 - Autumn Leaves.pdf"),
        },
        "the same content at a new path is a move"
    );
    assert_eq!(
        store
            .reindex_note(&note("b.pdf", 7, Some(NoteId::new(13)), "B", None, &[],))
            .expect("changed content at a known path"),
        ReindexOutcome::Replaced {
            previous: digest(6),
            duplicate_at: None
        },
        "changed content at a known path is a replacement"
    );
    assert_eq!(
        store
            .reindex_note(&note(
                "new/12 - Autumn Leaves.pdf",
                5,
                Some(NoteId::new(12)),
                "Autumn Leaves",
                None,
                &[],
            ))
            .expect("rescanning an unchanged file"),
        ReindexOutcome::Unchanged,
        "an unchanged file is not a move and not a replacement"
    );
    assert_eq!(
        store
            .reindex_note(&note("c.pdf", 8, Some(NoteId::new(14)), "C", None, &[]))
            .expect("a new file"),
        ReindexOutcome::Inserted
    );

    let notes = store.list_all().expect("list");
    assert_eq!(notes.len(), 3);
    assert_eq!(notes[0].path(), &path("new/12 - Autumn Leaves.pdf"));
    assert_eq!(notes[0].fingerprint().sha256(), digest(5));
    assert_eq!(notes[1].fingerprint().sha256(), digest(7));
    assert_eq!(notes[2].fingerprint().sha256(), digest(8));
}

#[test]
fn reindexing_content_that_lives_at_another_path_collapses_the_duplicate() {
    let store = IndexStore::open(":memory:").expect("open in memory");
    store
        .upsert_note_with_ocr(
            &note("first.pdf", 9, Some(NoteId::new(21)), "Copy", None, &[]),
            Some(&OcrText::new("shared content")),
        )
        .expect("insert");
    store
        .upsert_note(&note(
            "second.pdf",
            10,
            Some(NoteId::new(22)),
            "Other",
            None,
            &[],
        ))
        .expect("insert");

    let outcome = store
        .reindex_note(&note(
            "second.pdf",
            9,
            Some(NoteId::new(22)),
            "Copy",
            None,
            &[],
        ))
        .expect("the new content is already indexed elsewhere");

    assert_eq!(
        outcome,
        ReindexOutcome::Replaced {
            previous: digest(10),
            duplicate_at: Some(path("first.pdf")),
        },
        "the record of the path being written stays and takes the new fingerprint"
    );
    let notes = store.list_all().expect("list");
    assert_eq!(notes.len(), 1, "one record per content hash");
    assert_eq!(notes[0].path(), &path("second.pdf"));
    assert_eq!(notes[0].fingerprint().sha256(), digest(9));
    assert_eq!(notes[0].id(), Some(NoteId::new(22)));
    assert_eq!(
        store.ocr_text(&digest(9)).expect("read ocr"),
        Some(OcrText::new("shared content")),
        "text recognized for the content is inherited by whichever path holds it"
    );
}

#[test]
fn a_duplicate_id_is_still_refused_after_a_same_path_replacement() {
    let store = IndexStore::open(":memory:").expect("open in memory");
    store
        .upsert_note(&note("a.pdf", 1, Some(NoteId::new(9)), "First", None, &[]))
        .expect("first record claims id 9");
    store
        .reindex_note(&note(
            "a.pdf",
            2,
            Some(NoteId::new(9)),
            "First, revised",
            None,
            &[],
        ))
        .expect("reindex the changed file");

    let error = store
        .reindex_note(&note("b.pdf", 3, Some(NoteId::new(9)), "Second", None, &[]))
        .expect_err("a contested id must be refused");

    match error {
        IndexError::DuplicateNoteId(id) => assert_eq!(id, NoteId::new(9)),
        other => panic!("unexpected error: {other}"),
    }
    let notes = store.list_all().expect("list");
    assert_eq!(notes.len(), 1, "the rejected record must not be stored");
    assert_eq!(
        notes[0].fingerprint().sha256(),
        digest(2),
        "the replacement survives the refused write"
    );
}

#[test]
fn opening_a_database_that_claims_a_version_without_the_schema_is_refused() {
    for recorded in 1..=SCHEMA_VERSION {
        let file = temp_db(&format!("hollow-{recorded}"));
        let conn = rusqlite::Connection::open(&file).expect("create raw");
        let versions: String = (1..=recorded)
            .map(|version| format!("INSERT INTO schema_migrations (version) VALUES ({version});"))
            .collect();
        conn.execute_batch(&format!(
            "CREATE TABLE schema_migrations (
                 version INTEGER PRIMARY KEY,
                 applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             );
             {versions}"
        ))
        .expect("seed the bookkeeping only");
        drop(conn);

        let error = IndexStore::open(&file)
            .expect_err("a version row without the schema it names must not be trusted");
        match error {
            IndexError::IncompleteSchema {
                version,
                ref missing,
            } => {
                assert_eq!(version, recorded);
                assert!(
                    missing.contains("notes") && missing.contains("notes_fts"),
                    "the missing objects must be named: {missing}"
                );
            }
            other => panic!("unexpected error at recorded version {recorded}: {other}"),
        }
        let _ = std::fs::remove_file(&file);
    }
}

#[test]
fn a_failed_migration_leaves_the_database_unchanged() {
    let file = temp_db("failed-migration");
    let conn = rusqlite::Connection::open(&file).expect("create raw");
    conn.execute_batch("CREATE TABLE notes (unrelated TEXT);")
        .expect("an incompatible table blocks the initial migration");
    drop(conn);

    IndexStore::open(&file).expect_err("the initial migration must fail");

    let conn = rusqlite::Connection::open(&file).expect("reopen raw");
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .expect("prepare")
        .query_map([], |row| row.get(0))
        .expect("query")
        .collect::<Result<Vec<String>, _>>()
        .expect("rows");
    assert_eq!(
        tables,
        vec!["notes".to_owned()],
        "a failed migration must leave no bookkeeping and no partial schema"
    );
    drop(conn);
    let _ = std::fs::remove_file(&file);
}

#[test]
fn get_by_id_returns_none_for_unknown_ids() {
    let store = IndexStore::open(":memory:").expect("open in memory");
    store
        .upsert_note(&note("a.pdf", 1, Some(NoteId::new(1)), "A", None, &[]))
        .expect("insert");
    assert!(store.get_by_id(NoteId::new(2)).expect("lookup").is_none());
    assert!(store.get_by_hash(&digest(42)).expect("lookup").is_none());
}

fn scoped_store() -> IndexStore {
    let store = IndexStore::open(":memory:").expect("open");
    let schubert = note(
        "ave.pdf",
        1,
        Some(NoteId::new(13)),
        "Ave Maria",
        Some("Schubert"),
        &["solo"],
    );
    let antiphons = note(
        "antiphons.pdf",
        2,
        Some(NoteId::new(77)),
        "Marian antiphons",
        Some("Traditional"),
        &["choir"],
    );
    store.upsert_note(&schubert).expect("insert");
    store
        .upsert_note_with_ocr(
            &antiphons,
            Some(&OcrText::new(
                "Regina caeli laetare alleluia. Ave Maria gratia plena Dominus tecum",
            )),
        )
        .expect("insert");
    store
}

#[test]
fn text_scope_matches_only_page_text_and_returns_a_marked_snippet() {
    let hits = scoped_store()
        .search_in("ave maria", SearchScope::Text, 10)
        .expect("search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].note.title(), "Marian antiphons");
    let snippet = hits[0].text_snippet.as_deref().expect("snippet");
    assert!(snippet.contains("\u{2}Ave\u{3}"), "{snippet:?}");
}

#[test]
fn titles_scope_ignores_page_text() {
    let hits = scoped_store()
        .search_in("ave", SearchScope::Titles, 10)
        .expect("search");
    let titles: Vec<&str> = hits.iter().map(|hit| hit.note.title()).collect();
    assert_eq!(titles, ["Ave Maria"]);
    assert!(hits[0].text_snippet.is_none());
}

#[test]
fn composers_and_tags_scopes_use_their_columns() {
    let store = scoped_store();
    assert_eq!(
        store
            .search_in("traditional", SearchScope::Composers, 10)
            .expect("search")
            .len(),
        1
    );
    assert_eq!(
        store
            .search_in("choir", SearchScope::Tags, 10)
            .expect("search")
            .len(),
        1
    );
    assert!(
        store
            .search_in("choir", SearchScope::Composers, 10)
            .expect("search")
            .is_empty()
    );
}

#[test]
fn everything_scope_matches_search() {
    let store = scoped_store();
    let scoped = store
        .search_in("ave", SearchScope::Everything, 10)
        .expect("search");
    let plain = store.search("ave", 10).expect("search");
    let titles = |hits: &[notae_index::SearchHit]| {
        hits.iter()
            .map(|hit| hit.note.title().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(titles(&scoped), titles(&plain));
}

#[test]
fn punctuation_only_query_in_any_scope_returns_nothing() {
    let store = scoped_store();
    for scope in [
        SearchScope::Everything,
        SearchScope::Titles,
        SearchScope::Composers,
        SearchScope::Tags,
        SearchScope::Text,
    ] {
        assert!(
            store
                .search_in("?!…", scope, 10)
                .expect("search")
                .is_empty()
        );
    }
}

#[test]
fn scope_keys_round_trip() {
    for scope in [
        SearchScope::Everything,
        SearchScope::Titles,
        SearchScope::Composers,
        SearchScope::Tags,
        SearchScope::Text,
    ] {
        assert_eq!(SearchScope::from_key(scope.key()), scope);
    }
    assert_eq!(SearchScope::from_key("bogus"), SearchScope::Everything);
}

#[test]
fn polish_titles_match_exact_and_case_varied_queries_in_every_scope() {
    let store = IndexStore::open(":memory:").expect("open");
    let polish = note(
        "score.pdf",
        3,
        Some(NoteId::new(303)),
        "Łódź — Pieśń żółta",
        None,
        &[],
    );
    store.upsert_note(&polish).expect("insert");

    for query in [
        "Łódź — Pieśń żółta",
        "łódź",
        "ŁÓDŹ",
        "Łódź",
        "łÓdŹ",
        "pieśń",
        "PIEŚŃ",
        "żółta",
        "ŻÓŁTA",
        "ł",
        "Łó",
    ] {
        for scope in [SearchScope::Titles, SearchScope::Everything] {
            let hits = store.search_in(query, scope, 10).expect("search");
            assert_eq!(hits.len(), 1, "query {query:?} in {scope:?}");
            assert_eq!(hits[0].note.title(), "Łódź — Pieśń żółta");
        }
    }
    assert_eq!(store.search("łódź", 10).expect("search").len(), 1);
}

const LODZ_TITLE: &str = "Łódź — Pieśń żółta";
const LODZ_LYRICS: &str =
    "Pałka zagrała kołysankę dla Gałuszki. Części stałe mszy: zmartwychwstał Pan.";

/// The real-store fixture of issue #35: one score whose title, composer, tag and
/// lyrics all contain `ł`, and one control score without any.
fn lodz_store() -> IndexStore {
    let store = IndexStore::open(":memory:").expect("open");
    store
        .upsert_note_with_ocr(
            &note(
                "score.pdf",
                3,
                Some(NoteId::new(303)),
                LODZ_TITLE,
                Some("Stanisław Moniuszko"),
                &["kołysanka"],
            ),
            Some(&OcrText::new(LODZ_LYRICS)),
        )
        .expect("insert");
    store
        .upsert_note_with_ocr(
            &note(
                "ave.pdf",
                4,
                Some(NoteId::new(13)),
                "Ave Maria",
                Some("Schubert"),
                &["solo"],
            ),
            Some(&OcrText::new("Ave Maria gratia plena")),
        )
        .expect("insert");
    store
}

fn titles(store: &IndexStore, query: &str, scope: SearchScope) -> Vec<String> {
    store
        .search_in(query, scope, 10)
        .expect("search")
        .iter()
        .map(|hit| hit.note.title().to_owned())
        .collect()
}

#[test]
fn queries_with_and_without_the_stroke_find_polish_text_in_every_scope() {
    let store = lodz_store();
    let cases: [(SearchScope, &[&str]); 5] = [
        (
            SearchScope::Titles,
            &[
                // Without the stroke or the other diacritics.
                "lodz",
                "Lodz",
                "LODZ",
                "zolta",
                "zolta piesn",
                "piesn",
                // As typed on a Polish keyboard, any case.
                "łódź",
                "Łódź",
                "ŁÓDŹ",
                "łÓdŹ",
                "żółta",
                "ŻÓŁTA",
                "Łódź — Pieśń żółta",
                // Mixed: stroke kept, other diacritics dropped, and the reverse.
                "łodz",
                "ŁODZ",
                "lódź",
                "zołta",
            ],
        ),
        (
            SearchScope::Composers,
            &[
                "stanislaw",
                "Stanislaw",
                "STANISLAW",
                "Stanisław",
                "STANISŁAW",
                "moniuszko",
            ],
        ),
        (
            SearchScope::Tags,
            &["kolysanka", "Kolysanka", "kołysanka", "KOŁYSANKA", "kolys"],
        ),
        (
            SearchScope::Text,
            &[
                "palka",
                "Palka",
                "Pałka",
                "galuszki",
                "Galuszki",
                "Gałuszki",
                "czesci stale",
                "części stałe",
                "Czesci Stale",
                "zmartwychwstal",
                "zmartwychwstał",
                "ZMARTWYCHWSTAŁ",
            ],
        ),
        (
            SearchScope::Everything,
            &[
                "lodz",
                "ŁÓDŹ",
                "stanislaw",
                "Stanisław",
                "kolysanka",
                "palka",
                "Pałka",
                "czesci stale",
            ],
        ),
    ];
    for (scope, queries) in cases {
        for query in queries {
            assert_eq!(
                titles(&store, query, scope),
                [LODZ_TITLE],
                "query {query:?} in {scope:?}"
            );
        }
    }
}

#[test]
fn the_examples_of_issue_35_find_their_scores() {
    let store = IndexStore::open(":memory:").expect("open");
    let titles_and_queries = [
        ("Pałka", "palka"),
        ("Gałuszka", "Galuszka"),
        ("Części stałe", "czesci stale"),
        ("Zmartwychwstał", "zmartwychwstal"),
        ("Łódź", "lodz"),
    ];
    for (seed, (title, _)) in (1_u8..).zip(titles_and_queries) {
        store
            .upsert_note(&note(&format!("s{seed}.pdf"), seed, None, title, None, &[]))
            .expect("insert");
    }
    for (title, query) in titles_and_queries {
        assert_eq!(
            titles(&store, query, SearchScope::Everything),
            [title],
            "{query:?}"
        );
        assert_eq!(
            titles(&store, title, SearchScope::Everything),
            [title],
            "{title:?}"
        );
    }
}

#[test]
fn plain_l_and_stroke_l_are_one_letter_in_both_directions() {
    let store = IndexStore::open(":memory:").expect("open");
    store
        .upsert_note(&note("a.pdf", 1, None, "Lodz", None, &[]))
        .expect("insert");
    store
        .upsert_note(&note("b.pdf", 2, None, "Łódź", None, &[]))
        .expect("insert");
    for query in ["lodz", "Lodz", "łodz", "Łódź", "ŁÓDŹ"] {
        let mut found = titles(&store, query, SearchScope::Titles);
        found.sort();
        assert_eq!(found, ["Lodz", "Łódź"], "{query:?}");
    }
}

#[test]
fn scopes_stay_separate_after_folding() {
    let store = lodz_store();
    assert!(titles(&store, "lodz", SearchScope::Composers).is_empty());
    assert!(titles(&store, "palka", SearchScope::Titles).is_empty());
    assert!(titles(&store, "stanislaw", SearchScope::Tags).is_empty());
    assert!(titles(&store, "kolysanka", SearchScope::Text).is_empty());
    assert_eq!(
        titles(&store, "ave", SearchScope::Everything),
        ["Ave Maria"]
    );
}

#[test]
fn the_exact_title_bonus_ignores_the_stroke() {
    // Only the stroke differs between the query and the exact title: the exact
    // title bonus has never folded the other diacritics.
    let store = IndexStore::open(":memory:").expect("open");
    store
        .upsert_note(&note("a.pdf", 1, None, "Łaska łaskawa", None, &[]))
        .expect("insert");
    store
        .upsert_note(&note("b.pdf", 2, None, "Łaska", None, &[]))
        .expect("insert");
    for query in ["laska", "Laska", "łaska", "ŁASKA", " Łaska "] {
        let hits = store
            .search_in(query, SearchScope::Titles, 10)
            .expect("search");
        assert_eq!(hits.len(), 2, "{query:?}");
        assert_eq!(hits[0].note.title(), "Łaska", "{query:?}");
        assert!(
            hits[0].rank > 50.0,
            "exact title bonus for {query:?}: {}",
            hits[0].rank
        );
        assert!(
            hits[1].rank < 50.0,
            "no bonus for the longer title: {}",
            hits[1].rank
        );
        let limited = store
            .search_in(query, SearchScope::Titles, 1)
            .expect("search");
        assert_eq!(limited[0].note.title(), "Łaska", "limit 1 for {query:?}");
    }
}

#[test]
fn only_the_l_stroke_is_folded() {
    // Scope of issue #35: `ł`/`Ł` only. These letters have no decomposition either
    // and stay distinct until a separate change decides otherwise.
    let store = IndexStore::open(":memory:").expect("open");
    for (seed, title) in (1_u8..).zip(["Øre", "Đorđe", "Cœli", "Cæli", "Straße"]) {
        store
            .upsert_note(&note(&format!("s{seed}.pdf"), seed, None, title, None, &[]))
            .expect("insert");
    }
    for query in ["ore", "dorde", "coeli", "caeli", "strasse"] {
        assert!(
            titles(&store, query, SearchScope::Titles).is_empty(),
            "{query:?} must not match yet"
        );
    }
}

#[test]
fn the_schema_version_is_the_newest_migration() {
    let versions: Vec<u32> = MIGRATIONS
        .iter()
        .map(|migration| migration.version)
        .collect();
    assert!(
        versions.windows(2).all(|pair| pair[0] < pair[1]),
        "{versions:?}"
    );
    assert_eq!(versions.last().copied(), Some(SCHEMA_VERSION));
    assert_eq!(SCHEMA_VERSION, 2, "the ł/Ł fold is schema version 2");
}

/// One SQL expression per `notes_fts` row, comparable across databases.
fn fts_dump(file: &Path) -> Vec<String> {
    let conn = rusqlite::Connection::open(file).expect("open raw");
    let mut statement = conn
        .prepare(
            "SELECT rowid || '|' || quote(CAST(note_id AS TEXT)) || '|' || quote(title)
                    || '|' || quote(composer) || '|' || quote(tags) || '|' || quote(filename)
                    || '|' || quote(ocr_text)
             FROM notes_fts ORDER BY rowid",
        )
        .expect("prepare");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("rows")
}

fn upgrade_fixture() -> (IndexedNote, OcrText) {
    (
        note(
            "Pałka.pdf",
            5,
            Some(NoteId::new(35)),
            LODZ_TITLE,
            Some("Stanisław Moniuszko"),
            &["kołysanka"],
        ),
        OcrText::new(LODZ_LYRICS),
    )
}

#[test]
fn a_version_1_index_is_folded_in_place_when_it_is_opened() {
    // Put a current database back into the state a version 1 build left behind:
    // unfolded FTS rows and no version 2 row in the bookkeeping.
    let legacy = temp_db("legacy-v1");
    let (record, ocr) = upgrade_fixture();
    let store = IndexStore::open(&legacy).expect("create");
    store
        .upsert_note_with_ocr(&record, Some(&ocr))
        .expect("insert");
    drop(store);
    let conn = rusqlite::Connection::open(&legacy).expect("open raw");
    conn.execute_batch(
        "DELETE FROM schema_migrations WHERE version = 2;
         UPDATE notes_fts SET
             title    = (SELECT title FROM notes WHERE notes.row_id = notes_fts.rowid),
             composer = (SELECT composer_or_artist FROM notes WHERE notes.row_id = notes_fts.rowid),
             tags     = (SELECT tags FROM notes WHERE notes.row_id = notes_fts.rowid),
             filename = (SELECT path FROM notes WHERE notes.row_id = notes_fts.rowid),
             ocr_text = (SELECT ocr_text FROM notes WHERE notes.row_id = notes_fts.rowid);",
    )
    .expect("rewrite the rows the way version 1 stored them");
    let unfolded: i64 = conn
        .query_row(
            "SELECT count(*) FROM notes_fts WHERE notes_fts MATCH '\"lodz\"*'",
            [],
            |row| row.get(0),
        )
        .expect("count");
    assert_eq!(unfolded, 0, "the simulated version 1 rows must be unfolded");
    drop(conn);

    let upgraded = IndexStore::open(&legacy).expect("opening migrates");
    assert_eq!(upgraded.schema_version().expect("version"), SCHEMA_VERSION);
    for query in [
        "lodz",
        "palka",
        "stanislaw",
        "kolysanka",
        "czesci stale",
        "Pałka",
    ] {
        assert_eq!(
            upgraded.search(query, 10).expect("search").len(),
            1,
            "{query:?}"
        );
    }
    // Canonical data is untouched: only the derived index was folded.
    let kept = upgraded
        .get_by_hash(&digest(5))
        .expect("read")
        .expect("record present");
    assert_eq!(kept.title(), LODZ_TITLE);
    assert_eq!(
        upgraded
            .ocr_text(&digest(5))
            .expect("ocr")
            .expect("text")
            .text(),
        LODZ_LYRICS
    );
    drop(upgraded);

    // The frozen SQL in the migration and the Rust writer produce the same rows.
    let fresh = temp_db("fresh-v2");
    let store = IndexStore::open(&fresh).expect("create");
    store
        .upsert_note_with_ocr(&record, Some(&ocr))
        .expect("insert");
    drop(store);
    let migrated = fts_dump(&legacy);
    assert_eq!(migrated, fts_dump(&fresh));
    assert!(migrated[0].contains("'Palka.pdf'"), "{migrated:?}");
    let _ = std::fs::remove_file(&legacy);
    let _ = std::fs::remove_file(&fresh);
}

#[test]
fn reopening_a_migrated_index_does_not_change_it() {
    let file = temp_db("migrated-twice");
    let (record, ocr) = upgrade_fixture();
    let store = IndexStore::open(&file).expect("create");
    store
        .upsert_note_with_ocr(&record, Some(&ocr))
        .expect("insert");
    drop(store);
    let first = fts_dump(&file);
    drop(IndexStore::open(&file).expect("reopen"));
    assert_eq!(fts_dump(&file), first);
    let _ = std::fs::remove_file(&file);
}
