//! Integration tests for fingerprint reconciliation and metadata provenance.
//!
//! A rescan is planned in full against an existing index and applied
//! afterwards, so every test here first checks what the *plan* says — it is the
//! read-only preview the Android UI shows — and then what the applied report
//! says.

use std::path::{Path, PathBuf};

use notae_domain::{
    IndexStatus, IndexedNote, MetadataField, MetadataOrigin, NoteId, NoteMetadata, PdfFingerprint,
    RelativePdfPath, Sha256Digest,
};
use notae_index::{
    CatalogError, FileCondition, FileFailureReason, IndexStore, OcrText, PdfCatalog, PdfFile,
    Reconciler, refresh_from_filename,
};

fn digest(seed: u8) -> Sha256Digest {
    Sha256Digest::from_bytes([seed; 32])
}

fn path(value: &str) -> RelativePdfPath {
    RelativePdfPath::new(value).expect("valid relative path")
}

fn file(relative: &str, seed: u8) -> PdfFile {
    PdfFile::from_path(path(relative), PdfFingerprint::new(digest(seed), 1_024, None))
}

fn note(relative: &str, seed: u8, id: Option<NoteId>, title: &str) -> IndexedNote {
    let mut metadata = NoteMetadata::new(title, None, []).expect("valid title");
    metadata.assign_id(id, MetadataOrigin::FilenameDerived);
    IndexedNote::new(path(relative), PdfFingerprint::new(digest(seed), 1_024, None), metadata)
}

fn temp_db(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "notae-reconcile-{label}-{}-{nanos}.sqlite",
        std::process::id()
    ))
}

fn open(label: &str) -> (IndexStore, PathBuf) {
    let file = temp_db(label);
    let store = IndexStore::open(&file).expect("open creates the database");
    (store, file)
}

/// Reads the write timestamp of one record, which is the observable proof that
/// a rescan did — or did not — write it.
fn indexed_at(database: &Path, note_path: &RelativePdfPath) -> String {
    let connection = rusqlite::Connection::open(database).expect("open raw");
    connection
        .query_row(
            "SELECT indexed_at FROM notes WHERE path = ?1",
            [note_path.to_string()],
            |row| row.get(0),
        )
        .expect("record is present")
}

fn only_note(store: &IndexStore) -> IndexedNote {
    let notes = store.list_all().expect("list");
    assert_eq!(notes.len(), 1, "expected exactly one indexed record");
    notes.into_iter().next().expect("one note")
}

/// A catalog over a fixed list, standing in for the Android Storage Access
/// Framework tree.
struct StubCatalog {
    files: Vec<PdfFile>,
}

impl PdfCatalog for StubCatalog {
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError> {
        Ok(self.files.clone())
    }
}

#[test]
fn catalog_is_object_safe_and_drives_a_rescan() {
    let (mut store, database) = open("catalog");
    let catalog = StubCatalog {
        files: vec![file("12 - Autumn Leaves.pdf", 1)],
    };

    let files = (&catalog as &dyn PdfCatalog)
        .enumerate()
        .expect("enumerate");
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.additions().len(), 1);

    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(report.additions().len(), 1);
    let _ = std::fs::remove_file(&database);
}

#[test]
fn new_pdf_is_added() {
    let (mut store, database) = open("add");
    let files = vec![file("12 - Autumn Leaves.pdf", 1)];

    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.additions().len(), 1);
    assert!(plan.updates().is_empty());
    assert!(plan.moves().is_empty());
    assert!(plan.deletions().is_empty());
    assert!(plan.conflicts().is_empty());
    assert!(plan.unchanged().is_empty());
    assert!(plan.failures().is_empty());

    let added = &plan.additions()[0];
    assert_eq!(added.note().path(), &path("12 - Autumn Leaves.pdf"));
    assert_eq!(added.note().id(), Some(NoteId::new(12)));
    assert_eq!(added.note().title(), "Autumn Leaves");
    assert_eq!(added.note().status(), IndexStatus::Discovered);
    assert_eq!(
        added.note().metadata().origin_of(MetadataField::Title),
        MetadataOrigin::FilenameDerived
    );

    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(report.additions().len(), 1);
    assert!(report.failures().is_empty());

    let stored = only_note(&store);
    assert_eq!(stored.id(), Some(NoteId::new(12)));
    assert_eq!(stored.title(), "Autumn Leaves");
    let _ = std::fs::remove_file(&database);
}

#[test]
fn unchanged_pdf_is_reused_without_a_write() {
    let (mut store, database) = open("unchanged");
    let mut original = note("12 - Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves");
    original.set_status(IndexStatus::Ready);
    store
        .upsert_note_with_ocr(&original, Some(&OcrText::with_language("alto clef", "eng")))
        .expect("seed");
    let before = indexed_at(&database, &path("12 - Autumn Leaves.pdf"));

    let files = vec![file("12 - Autumn Leaves.pdf", 1)];
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.unchanged().len(), 1);
    assert!(plan.additions().is_empty());
    assert!(plan.updates().is_empty());
    assert!(plan.moves().is_empty());
    assert!(plan.deletions().is_empty());
    assert!(plan.failures().is_empty());

    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(report.unchanged().len(), 1);
    assert!(report.additions().is_empty());
    assert!(report.updates().is_empty());
    assert!(report.moves().is_empty());
    assert!(report.deletions().is_empty());

    let stored = only_note(&store);
    assert_eq!(stored.status(), IndexStatus::Ready);
    assert_eq!(
        store
            .ocr_text(&digest(1))
            .expect("ocr")
            .expect("ocr text")
            .text(),
        "alto clef"
    );
    assert_eq!(
        indexed_at(&database, &path("12 - Autumn Leaves.pdf")),
        before,
        "an unchanged file must not be written"
    );
    let _ = std::fs::remove_file(&database);
}

#[test]
fn same_hash_rename_is_a_move_that_keeps_one_record() {
    let (mut store, database) = open("move");
    let mut original = note("12 - Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves");
    original.set_status(IndexStatus::OcrPending);
    store
        .upsert_note_with_ocr(&original, Some(&OcrText::new("alto clef")))
        .expect("seed");

    let files = vec![file("12 - Autumn Leaves (revised).pdf", 1)];
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.moves().len(), 1);
    assert!(plan.additions().is_empty());
    assert!(plan.updates().is_empty());
    assert!(plan.deletions().is_empty());

    let moved = &plan.moves()[0];
    assert_eq!(moved.from(), &path("12 - Autumn Leaves.pdf"));
    assert_eq!(
        moved.note().path(),
        &path("12 - Autumn Leaves (revised).pdf")
    );
    assert_eq!(
        moved.note().status(),
        IndexStatus::OcrPending,
        "the content is unchanged, so the pipeline status carries over"
    );

    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(report.moves().len(), 1);
    assert!(report.deletions().is_empty(), "the old path is moved, not deleted");

    let stored = only_note(&store);
    assert_eq!(
        stored.path(),
        &path("12 - Autumn Leaves (revised).pdf"),
        "a rename must keep one record, not create a second"
    );
    assert_eq!(stored.id(), Some(NoteId::new(12)));
    assert_eq!(
        stored.fingerprint().sha256(),
        digest(1),
        "identity is the content hash, not the path"
    );
    assert_eq!(
        store
            .ocr_text(&digest(1))
            .expect("ocr")
            .expect("recognized text")
            .text(),
        "alto clef"
    );
    let _ = std::fs::remove_file(&database);
}

#[test]
fn changed_content_at_a_known_path_updates_and_preserves_manual_metadata() {
    let (mut store, database) = open("update");
    let mut original = note("12 - Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves");
    let mut metadata = original.metadata().clone();
    metadata.set_title("Autumn Leaves (my edition)").expect("title");
    metadata.set_composer_or_artist(Some("Miles Davis".to_owned()));
    metadata.set_tags(["jazz".to_owned(), "slow".to_owned()]);
    metadata.set_id(Some(NoteId::new(99)));
    original = IndexedNote::new(
        path("12 - Autumn Leaves.pdf"),
        PdfFingerprint::new(digest(1), 1_024, None),
        metadata,
    );
    original.set_status(IndexStatus::Ready);
    store
        .upsert_note_with_ocr(&original, Some(&OcrText::new("alto clef")))
        .expect("seed");

    let files = vec![file("12 - Autumn Leaves.pdf", 2)];
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.updates().len(), 1);
    assert!(plan.moves().is_empty());
    assert!(plan.additions().is_empty());

    let updated = &plan.updates()[0];
    assert_eq!(updated.previous(), original.fingerprint());
    assert_eq!(updated.note().fingerprint().sha256(), digest(2));
    assert_eq!(updated.note().id(), Some(NoteId::new(99)));
    assert_eq!(updated.note().title(), "Autumn Leaves (my edition)");
    assert_eq!(
        updated.note().metadata().composer_or_artist(),
        Some("Miles Davis")
    );

    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(report.updates().len(), 1);

    let stored = only_note(&store);
    assert_eq!(stored.id(), Some(NoteId::new(99)));
    assert_eq!(stored.title(), "Autumn Leaves (my edition)");
    assert_eq!(stored.metadata().composer_or_artist(), Some("Miles Davis"));
    assert_eq!(
        stored.metadata().tags().iter().cloned().collect::<Vec<_>>(),
        vec!["jazz".to_owned(), "slow".to_owned()]
    );
    assert_eq!(stored.fingerprint().sha256(), digest(2));
    assert_eq!(
        store.ocr_text(&digest(1)).expect("ocr"),
        None,
        "text recognized from replaced bytes is dropped"
    );
    let _ = std::fs::remove_file(&database);
}

#[test]
fn deleted_pdf_is_removed() {
    let (mut store, database) = open("delete");
    store
        .upsert_note(&note("12 - Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves"))
        .expect("seed kept");
    store
        .upsert_note(&note("13 - So What.pdf", 2, Some(NoteId::new(13)), "So What"))
        .expect("seed deleted");

    let files = vec![file("12 - Autumn Leaves.pdf", 1)];
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.deletions().len(), 1);
    assert_eq!(plan.deletions()[0].path(), &path("13 - So What.pdf"));
    assert!(plan.additions().is_empty());
    assert!(plan.unchanged().len() == 1);

    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(report.deletions().len(), 1);

    let stored = only_note(&store);
    assert_eq!(stored.path(), &path("12 - Autumn Leaves.pdf"));
    let _ = std::fs::remove_file(&database);
}

#[test]
fn duplicate_ids_are_reported_as_conflicts_with_both_paths() {
    let (mut store, database) = open("conflict");
    store
        .upsert_note(&note("12 - Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves"))
        .expect("seed");

    let files = vec![
        file("12 - Autumn Leaves.pdf", 1),
        file("12 - Autumn Leaves (copy).pdf", 2),
    ];
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.conflicts().len(), 1);
    assert_eq!(
        plan.additions().len(),
        1,
        "the newcomer is indexed, not hidden"
    );

    let conflict = &plan.conflicts()[0];
    assert_eq!(conflict.id(), NoteId::new(12));
    assert_eq!(conflict.existing(), &path("12 - Autumn Leaves.pdf"));
    assert_eq!(conflict.conflicting(), &path("12 - Autumn Leaves (copy).pdf"));

    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(report.conflicts().len(), 1);
    assert!(report.failures().is_empty());

    let notes = store.list_all().expect("list");
    assert_eq!(notes.len(), 2, "both files stay in the library");
    let kept = store
        .get_by_id(NoteId::new(12))
        .expect("query")
        .expect("the indexed record keeps the ID");
    assert_eq!(kept.path(), &path("12 - Autumn Leaves.pdf"));
    let withheld = notes
        .into_iter()
        .find(|note| note.path() == &path("12 - Autumn Leaves (copy).pdf"))
        .expect("the newcomer is indexed too");
    assert_eq!(
        withheld.id(),
        None,
        "the newcomer holds no ID until the user resolves the conflict"
    );
    assert_eq!(withheld.title(), "Autumn Leaves (copy)");
    for stored in [kept, withheld] {
        assert_eq!(
            stored.status(),
            IndexStatus::DuplicateIdConflict,
            "both sides show that the ID is contested"
        );
    }
    let _ = std::fs::remove_file(&database);
}

#[test]
fn ocr_status_is_retained_across_a_no_op_rescan() {
    let (mut store, database) = open("ocr");
    let mut original = note("12 - Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves");
    original.set_status(IndexStatus::OcrIncomplete);
    store
        .upsert_note_with_ocr(&original, Some(&OcrText::with_language("coda", "eng")))
        .expect("seed");

    let files = vec![file("12 - Autumn Leaves.pdf", 1)];
    for _ in 0..2 {
        let plan = Reconciler::plan(&store, &files).expect("plan");
        assert_eq!(plan.unchanged().len(), 1);
        let report = Reconciler::apply(plan, &mut store).expect("apply");
        assert_eq!(report.unchanged().len(), 1);
    }

    let stored = only_note(&store);
    assert_eq!(stored.status(), IndexStatus::OcrIncomplete);
    let ocr = store
        .ocr_text(&digest(1))
        .expect("ocr")
        .expect("recognized text survives");
    assert_eq!(ocr.text(), "coda");
    assert_eq!(ocr.language(), Some("eng"));
    let _ = std::fs::remove_file(&database);
}

#[test]
fn manual_fields_survive_a_filename_refresh_while_others_update() {
    let (mut store, database) = open("provenance");
    let mut manual = note("old name.pdf", 1, Some(NoteId::new(1)), "Old Title");
    let mut metadata = manual.metadata().clone();
    metadata.set_title("Hand Written Title").expect("valid title");
    metadata.set_composer_or_artist(Some("Bill Evans".to_owned()));
    metadata.set_tags(["piano".to_owned(), "modal".to_owned()]);
    metadata.set_id(Some(NoteId::new(42)));
    manual = IndexedNote::new(
        path("old name.pdf"),
        PdfFingerprint::new(digest(1), 1_024, None),
        metadata,
    );
    let derived = note("derived.pdf", 3, None, "Derived Title");
    store.upsert_note(&manual).expect("seed manual");
    store.upsert_note(&derived).expect("seed derived");

    // The filenames now both carry the convention; only the record whose
    // fields are all filename-derived may take the new values.
    let files = vec![
        file("7 - Prelude in C.pdf", 1),
        file("8 - Blue Bossa.pdf", 3),
    ];
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.moves().len(), 2);

    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(report.moves().len(), 2);
    assert!(report.failures().is_empty());

    let mut notes = store.list_all().expect("list");
    notes.sort_by_key(|note| note.path().as_str().to_owned());
    let [manual_note, derived_note] = notes.as_slice() else {
        panic!("expected two records");
    };

    assert_eq!(manual_note.id(), Some(NoteId::new(42)));
    assert_eq!(manual_note.title(), "Hand Written Title");
    assert_eq!(
        manual_note.metadata().composer_or_artist(),
        Some("Bill Evans")
    );
    assert_eq!(
        manual_note.metadata().tags().iter().cloned().collect::<Vec<_>>(),
        vec!["modal".to_owned(), "piano".to_owned()]
    );
    for field in MetadataField::ALL {
        assert!(
            manual_note.metadata().provenance().is_manual(field),
            "{field:?} was edited by hand and must survive a filename refresh"
        );
    }

    assert_eq!(derived_note.id(), Some(NoteId::new(8)));
    assert_eq!(derived_note.title(), "Blue Bossa");
    assert_eq!(
        derived_note.metadata().origin_of(MetadataField::Title),
        MetadataOrigin::FilenameDerived
    );
    assert_eq!(
        derived_note.metadata().origin_of(MetadataField::Id),
        MetadataOrigin::FilenameDerived
    );
    let _ = std::fs::remove_file(&database);
}

#[test]
fn per_file_failure_is_isolated_rather_than_aborting_the_plan() {
    let (mut store, database) = open("failure");
    store
        .upsert_note(&note("12 - Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves"))
        .expect("seed");

    // `.pdf` has an empty stem, so it can never seed a title.
    let files = vec![
        file("12 - Autumn Leaves.pdf", 1),
        file(".pdf", 2),
        file("13 - So What.pdf", 3),
    ];
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.failures().len(), 1);
    assert_eq!(plan.failures()[0].path(), &path(".pdf"));
    assert_eq!(plan.additions().len(), 1);
    assert_eq!(plan.unchanged().len(), 1);

    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(report.failures().len(), 1);
    assert_eq!(report.additions().len(), 1);
    assert_eq!(report.unchanged().len(), 1);

    let notes = store.list_all().expect("list");
    assert_eq!(notes.len(), 2, "the healthy files still indexed");
    let _ = std::fs::remove_file(&database);
}

#[test]
fn planning_never_mutates_the_store() {
    let (store, database) = open("readonly");
    store
        .upsert_note(&note("12 - Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves"))
        .expect("seed");
    let before = store.list_all().expect("list");
    let stamp = indexed_at(&database, &path("12 - Autumn Leaves.pdf"));

    let files = vec![
        file("12 - Autumn Leaves.pdf", 1),
        file("13 - So What.pdf", 2),
        file("14 - Blue in Green.pdf", 3),
    ];
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.additions().len(), 2);

    assert_eq!(store.list_all().expect("list"), before);
    assert_eq!(
        indexed_at(&database, &path("12 - Autumn Leaves.pdf")),
        stamp,
        "the plan is a preview, not a write"
    );
    let _ = std::fs::remove_file(&database);
}

#[test]
fn refresh_reports_which_fields_it_preserved() {
    let mut metadata = NoteMetadata::new("Old", None, []).expect("title");
    metadata.set_title("Hand Written").expect("valid title");
    metadata.assign_id(Some(NoteId::new(5)), MetadataOrigin::FilenameDerived);

    let (refreshed, outcome) = refresh_from_filename(&metadata, "9 - New Name.pdf")
        .expect("refresh");
    assert_eq!(refreshed.id(), Some(NoteId::new(9)));
    assert_eq!(refreshed.title(), "Hand Written");
    assert_eq!(refreshed.provenance().origin_of(MetadataField::Title), MetadataOrigin::Manual);
    assert!(outcome.preserved().contains(&MetadataField::Title));
    assert!(outcome.refreshed().contains(&MetadataField::Id));
    assert!(!outcome.preserved().contains(&MetadataField::Id));
}

/// Defect 1: two unseen files whose names parse to one ID must both stay in
/// the library. Dropping the loser hides a PDF the user owns, so the conflict
/// is reported and *both* records are indexed.
#[test]
fn two_new_files_claiming_one_id_are_both_kept_with_the_conflict_surfaced() {
    let (mut store, database) = open("duplicate-new");
    let files = vec![file("12 - One.pdf", 1), file("12 - Two.pdf", 2)];

    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.conflicts().len(), 1);
    assert_eq!(
        plan.additions().len(),
        2,
        "both PDFs stay in the plan; neither is hidden"
    );
    assert!(plan.failures().is_empty());

    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(report.conflicts().len(), 1);
    assert!(report.failures().is_empty());
    assert_eq!(report.additions().len(), 2);

    let mut notes = store.list_all().expect("list");
    notes.sort_by_key(|note| note.path().as_str().to_owned());
    assert_eq!(notes.len(), 2, "both files are visible in the library");
    let [one, two] = notes.as_slice() else {
        panic!("expected both conflicting files");
    };
    assert_eq!(one.path(), &path("12 - One.pdf"));
    assert_eq!(two.path(), &path("12 - Two.pdf"));
    assert_eq!(one.title(), "One");
    assert_eq!(two.title(), "Two");
    assert_eq!(
        store.get_by_id(NoteId::new(12)).expect("query").map(|n| n.path().clone()),
        Some(path("12 - One.pdf")),
        "exactly one record holds the contested ID"
    );
    assert!(
        two.id().is_none(),
        "the newcomer holds no ID until the user resolves the conflict"
    );
    for note in notes {
        assert_eq!(
            note.status(),
            IndexStatus::DuplicateIdConflict,
            "both sides of a contested ID show the conflict"
        );
    }
    let _ = std::fs::remove_file(&database);
}

/// Defect 2: two unseen paths holding the same bytes are one file, so the
/// second is reported instead of planned as a second record — and the
/// classification is the same on every rescan.
#[test]
fn identical_content_at_two_new_paths_is_deduplicated_stably() {
    let (mut store, database) = open("duplicate-hash");
    let files = vec![file("12 - One.pdf", 1), file("12 - Two.pdf", 1)];

    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.additions().len(), 1);
    assert!(plan.moves().is_empty());
    assert_eq!(plan.failures().len(), 1);
    assert_eq!(plan.failures()[0].path(), &path("12 - Two.pdf"));

    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(report.additions().len(), 1);
    assert_eq!(only_note(&store).path(), &path("12 - One.pdf"));

    // A second identical rescan must not oscillate between the two paths.
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert!(plan.moves().is_empty(), "no oscillation");
    assert!(plan.additions().is_empty());
    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert!(report.moves().is_empty());
    assert_eq!(report.failures().len(), 1);
    assert_eq!(only_note(&store).path(), &path("12 - One.pdf"));
    let _ = std::fs::remove_file(&database);
}

/// Defect 3: once the colliding file is gone the status must stop claiming a
/// conflict, because the stored record then says something untrue.
#[test]
fn duplicate_conflict_status_clears_once_the_collision_is_gone() {
    let (mut store, database) = open("conflict-clear");
    let mut contested = note("12 - Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves");
    contested.set_status(IndexStatus::DuplicateIdConflict);
    store.upsert_note(&contested).expect("seed");

    let files = vec![file("12 - Autumn Leaves.pdf", 1)];
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert!(plan.conflicts().is_empty(), "the collision is over");
    assert_eq!(
        plan.updates().len(),
        1,
        "clearing the status is a write"
    );
    assert_ne!(
        plan.updates()[0].note().status(),
        IndexStatus::DuplicateIdConflict
    );

    Reconciler::apply(plan, &mut store).expect("apply");
    let stored = only_note(&store);
    assert_eq!(stored.id(), Some(NoteId::new(12)));
    assert_ne!(
        stored.status(),
        IndexStatus::DuplicateIdConflict,
        "the status must reflect that no conflict exists"
    );
    let _ = std::fs::remove_file(&database);
}

/// Defect 4: a bare `.pdf` has no title. A manual stored title must not let an
/// unnameable file through as a silent relocation.
#[test]
fn a_bare_pdf_name_fails_even_when_the_stored_title_is_manual() {
    let (mut store, database) = open("empty-title");
    let mut manual = note(".pdf", 1, None, "Autumn Leaves");
    let mut metadata = manual.metadata().clone();
    metadata.set_title("Hand Written Title").expect("valid title");
    manual = IndexedNote::new(
        path(".pdf"),
        PdfFingerprint::new(digest(1), 1_024, None),
        metadata,
    );
    store.upsert_note(&manual).expect("seed");

    let files = vec![file(".pdf", 2)];
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.failures().len(), 1, "the empty title is a per-file failure");
    assert_eq!(plan.failures()[0].path(), &path(".pdf"));
    assert!(matches!(
        plan.failures()[0].reason(),
        FileFailureReason::Metadata(_)
    ));
    assert!(plan.updates().is_empty());
    assert!(plan.additions().is_empty());

    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(report.failures().len(), 1);
    assert_eq!(
        only_note(&store).fingerprint().sha256(),
        digest(1),
        "the stored record is untouched"
    );
    let _ = std::fs::remove_file(&database);
}

/// Defect 5: a record whose path and bytes are unchanged still has
/// filename-derived metadata to refresh, because a catalog's display name can
/// differ from the path.
#[test]
fn same_path_and_hash_refreshes_filename_derived_metadata() {
    let (mut store, database) = open("refresh-unchanged");
    store
        .upsert_note(&note("stable.pdf", 1, None, "Old Title"))
        .expect("seed");

    // The provider knows the document as `9 - New.pdf` while Notae files it at
    // `stable.pdf`, and the bytes have not changed.
    let files = vec![PdfFile::with_condition(
        path("stable.pdf"),
        "9 - New.pdf".to_owned(),
        FileCondition::Readable(PdfFingerprint::new(digest(1), 1_024, None)),
    )];

    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert!(
        plan.unchanged().is_empty(),
        "stale metadata must not be reported as untouched"
    );
    let refreshed = plan.updates().first().expect("a metadata refresh");
    assert_eq!(refreshed.note().title(), "New");
    assert_eq!(refreshed.note().id(), Some(NoteId::new(9)));

    Reconciler::apply(plan, &mut store).expect("apply");
    let stored = only_note(&store);
    assert_eq!(stored.title(), "New");
    assert_eq!(stored.id(), Some(NoteId::new(9)));
    let _ = std::fs::remove_file(&database);
}

/// Defect 6: the catalog must be able to report a file it could not read or
/// could not parse, and such a file becomes a per-file failure rather than an
/// indexed record.
#[test]
fn unreadable_and_invalid_pdfs_are_reported_per_file() {
    let (mut store, database) = open("conditions");
    let files = vec![
        file("12 - Autumn Leaves.pdf", 1),
        PdfFile::from_path_with_condition(path("13 - Locked.pdf"), FileCondition::Unreadable),
        PdfFile::from_path_with_condition(path("14 - Corrupt.pdf"), FileCondition::InvalidPdf),
    ];

    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.additions().len(), 1, "only the readable file is indexed");
    assert_eq!(plan.failures().len(), 2);
    assert_eq!(plan.failures()[0].path(), &path("13 - Locked.pdf"));
    assert!(matches!(
        plan.failures()[0].reason(),
        FileFailureReason::Unreadable
    ));
    assert_eq!(plan.failures()[1].path(), &path("14 - Corrupt.pdf"));
    assert!(matches!(
        plan.failures()[1].reason(),
        FileFailureReason::InvalidPdf
    ));

    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(report.failures().len(), 2);
    assert_eq!(only_note(&store).path(), &path("12 - Autumn Leaves.pdf"));
    let _ = std::fs::remove_file(&database);
}

/// Defect 6, continued: a file that becomes unreadable keeps its record and is
/// not mistaken for a deletion.
#[test]
fn an_unreadable_file_keeps_its_record_and_is_not_deleted() {
    let (mut store, database) = open("conditions-kept");
    let mut original = note("12 - Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves");
    original.set_status(IndexStatus::Ready);
    store
        .upsert_note_with_ocr(&original, Some(&OcrText::new("alto clef")))
        .expect("seed");

    let files = vec![PdfFile::from_path_with_condition(
        path("12 - Autumn Leaves.pdf"),
        FileCondition::Unreadable,
    )];
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert!(plan.deletions().is_empty(), "a locked file is not a deletion");
    assert_eq!(plan.failures().len(), 1);

    Reconciler::apply(plan, &mut store).expect("apply");
    let stored = only_note(&store);
    assert_eq!(stored.path(), &path("12 - Autumn Leaves.pdf"));
    assert_eq!(stored.id(), Some(NoteId::new(12)));
    assert_eq!(
        store.ocr_text(&digest(1)).expect("ocr").expect("text survives").text(),
        "alto clef"
    );
    let _ = std::fs::remove_file(&database);
}

/// A file the catalog could not read still exists on disk, so the deletion
/// pass must keep its record even while another file really is gone.
#[test]
fn an_unreadable_file_survives_the_deletion_pass() {
    let (mut store, database) = open("conditions-delete");
    store
        .upsert_note(&note("12 - Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves"))
        .expect("seed kept");
    store
        .upsert_note(&note("13 - So What.pdf", 2, Some(NoteId::new(13)), "So What"))
        .expect("seed gone");
    store
        .upsert_note(&note("14 - Locked.pdf", 3, Some(NoteId::new(14)), "Locked"))
        .expect("seed locked");

    let files = vec![
        PdfFile::from_path_with_condition(path("12 - Autumn Leaves.pdf"), FileCondition::Unreadable),
        file("14 - Locked.pdf", 3),
    ];
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.deletions().len(), 1);
    assert_eq!(plan.deletions()[0].path(), &path("13 - So What.pdf"));
    assert_eq!(plan.failures().len(), 1);

    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(report.deletions().len(), 1);
    assert_eq!(report.failures().len(), 1);

    let mut paths: Vec<String> = store
        .list_all()
        .expect("list")
        .into_iter()
        .map(|note| note.path().as_str().to_owned())
        .collect();
    paths.sort();
    assert_eq!(
        paths,
        vec![
            "12 - Autumn Leaves.pdf".to_owned(),
            "14 - Locked.pdf".to_owned(),
        ],
        "the unreadable file keeps its record; only the deleted file is removed"
    );
    let _ = std::fs::remove_file(&database);
}