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
    CatalogError, IndexStore, OcrText, PdfCatalog, PdfFile, Reconciler, refresh_from_filename,
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
    assert!(plan.additions().is_empty(), "the loser is never inserted");

    let conflict = &plan.conflicts()[0];
    assert_eq!(conflict.id(), NoteId::new(12));
    assert_eq!(conflict.existing(), &path("12 - Autumn Leaves.pdf"));
    assert_eq!(conflict.conflicting(), &path("12 - Autumn Leaves (copy).pdf"));

    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(report.conflicts().len(), 1);
    assert!(report.failures().is_empty());

    let stored = only_note(&store);
    assert_eq!(stored.path(), &path("12 - Autumn Leaves.pdf"));
    assert_eq!(
        stored.status(),
        IndexStatus::DuplicateIdConflict,
        "the surviving record must show that its ID is contested"
    );
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