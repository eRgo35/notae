//! Integration tests for the SQLite schema, record CRUD, and FTS5 search.

use std::collections::HashSet;
use std::path::PathBuf;

use notae_domain::{
    EpochTimestamp, IndexStatus, IndexedNote, MetadataField, MetadataOrigin, NoteId,
    NoteMetadata, PdfFingerprint, RelativePdfPath, Sha256Digest,
};
use notae_index::{IndexError, IndexStore, OcrText, SCHEMA_VERSION};

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
    let mut stored = IndexedNote::new(
        path("scores/12 - Autumn Leaves.pdf"),
        fingerprint,
        metadata,
    );
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
        loaded
            .metadata()
            .tags()
            .contains("transcription"),
        "manually added tag must survive"
    );
}

#[test]
fn update_replaces_the_stored_record() {
    let store = IndexStore::open(":memory:").expect("open in memory");
    let first = note("a.pdf", 1, Some(NoteId::new(4)), "Prelude", None, &[]);
    store.upsert_note(&first).expect("insert");

    let mut second = note("a.pdf", 1, Some(NoteId::new(4)), "Prelude in C", None, &["piano"]);
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
        .upsert_note(&note(
            "b.pdf",
            2,
            Some(NoteId::new(9)),
            "Second",
            None,
            &[],
        ))
        .expect_err("a duplicate id must not be silently hidden");

    match error {
        IndexError::DuplicateNoteId(id) => assert_eq!(id, NoteId::new(9)),
        other => panic!("unexpected error: {other}"),
    }
    assert_eq!(store.list_all().expect("list").len(), 1);
    assert!(
        store
            .get_by_hash(&digest(2))
            .expect("lookup")
            .is_none(),
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

    for query in ["Autumn", "jazz", "Sonatina", "Sonatina.pdf", "chromatic", "sharp"] {
        let hits = store.search(query, 10).expect("search");
        assert!(!hits.is_empty(), "expected a match for {query}");
        assert!(
            hits.iter().any(|hit| match query {
                "Autumn" => hit.note.id() == Some(NoteId::new(12)),
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
    assert_eq!(hits[1].note.id(), Some(NoteId::new(88)), "exact title second");
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
        vec![
            Some(NoteId::new(2)),
            Some(NoteId::new(10)),
            None,
            None,
        ]
    );
    let listed = store.list_all().expect("list");
    let titles: Vec<&str> = listed.iter().map(IndexedNote::title).collect();
    assert_eq!(titles[2], "Alpha", "unnumbered records sort by title");
    assert_eq!(titles[3], "beta");
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
