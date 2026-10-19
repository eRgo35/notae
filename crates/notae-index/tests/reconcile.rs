//! Integration tests for fingerprint reconciliation and metadata provenance.
//!
//! A rescan is planned in full against an existing index and applied
//! afterwards, so every test here first checks what the *plan* says — it is the
//! read-only preview the Android UI shows — and then what the applied report
//! says.
#![allow(clippy::expect_used)]

use std::collections::HashMap;
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
    PdfFile::from_path(
        path(relative),
        PdfFingerprint::new(digest(seed), 1_024, None),
    )
}

fn note(relative: &str, seed: u8, id: Option<NoteId>, title: &str) -> IndexedNote {
    let mut metadata = NoteMetadata::new(title, None, []).expect("valid title");
    metadata.assign_id(id, MetadataOrigin::FilenameDerived);
    IndexedNote::new(
        path(relative),
        PdfFingerprint::new(digest(seed), 1_024, None),
        metadata,
    )
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
    let mut original = note(
        "12 - Autumn Leaves.pdf",
        1,
        Some(NoteId::new(12)),
        "Autumn Leaves",
    );
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
    let mut original = note(
        "12 - Autumn Leaves.pdf",
        1,
        Some(NoteId::new(12)),
        "Autumn Leaves",
    );
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
    assert!(
        report.deletions().is_empty(),
        "the old path is moved, not deleted"
    );

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
    let mut original = note(
        "12 - Autumn Leaves.pdf",
        1,
        Some(NoteId::new(12)),
        "Autumn Leaves",
    );
    let mut metadata = original.metadata().clone();
    metadata
        .set_title("Autumn Leaves (my edition)")
        .expect("title");
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
        .upsert_note(&note(
            "12 - Autumn Leaves.pdf",
            1,
            Some(NoteId::new(12)),
            "Autumn Leaves",
        ))
        .expect("seed kept");
    store
        .upsert_note(&note(
            "13 - So What.pdf",
            2,
            Some(NoteId::new(13)),
            "So What",
        ))
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
        .upsert_note(&note(
            "12 - Autumn Leaves.pdf",
            1,
            Some(NoteId::new(12)),
            "Autumn Leaves",
        ))
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
    assert_eq!(
        conflict.conflicting(),
        &path("12 - Autumn Leaves (copy).pdf")
    );

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
    let mut original = note(
        "12 - Autumn Leaves.pdf",
        1,
        Some(NoteId::new(12)),
        "Autumn Leaves",
    );
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
    metadata
        .set_title("Hand Written Title")
        .expect("valid title");
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
        manual_note
            .metadata()
            .tags()
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
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
        .upsert_note(&note(
            "12 - Autumn Leaves.pdf",
            1,
            Some(NoteId::new(12)),
            "Autumn Leaves",
        ))
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
        .upsert_note(&note(
            "12 - Autumn Leaves.pdf",
            1,
            Some(NoteId::new(12)),
            "Autumn Leaves",
        ))
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

    let (refreshed, outcome) =
        refresh_from_filename(&metadata, "9 - New Name.pdf").expect("refresh");
    assert_eq!(refreshed.id(), Some(NoteId::new(9)));
    assert_eq!(refreshed.title(), "Hand Written");
    assert_eq!(
        refreshed.provenance().origin_of(MetadataField::Title),
        MetadataOrigin::Manual
    );
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
        store
            .get_by_id(NoteId::new(12))
            .expect("query")
            .map(|n| n.path().clone()),
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
    let mut contested = note(
        "12 - Autumn Leaves.pdf",
        1,
        Some(NoteId::new(12)),
        "Autumn Leaves",
    );
    contested.set_status(IndexStatus::DuplicateIdConflict);
    store.upsert_note(&contested).expect("seed");

    let files = vec![file("12 - Autumn Leaves.pdf", 1)];
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert!(plan.conflicts().is_empty(), "the collision is over");
    assert_eq!(plan.updates().len(), 1, "clearing the status is a write");
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
    metadata
        .set_title("Hand Written Title")
        .expect("valid title");
    manual = IndexedNote::new(
        path(".pdf"),
        PdfFingerprint::new(digest(1), 1_024, None),
        metadata,
    );
    store.upsert_note(&manual).expect("seed");

    let files = vec![file(".pdf", 2)];
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(
        plan.failures().len(),
        1,
        "the empty title is a per-file failure"
    );
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
    assert_eq!(
        plan.additions().len(),
        1,
        "only the readable file is indexed"
    );
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
    let mut original = note(
        "12 - Autumn Leaves.pdf",
        1,
        Some(NoteId::new(12)),
        "Autumn Leaves",
    );
    original.set_status(IndexStatus::Ready);
    store
        .upsert_note_with_ocr(&original, Some(&OcrText::new("alto clef")))
        .expect("seed");

    let files = vec![PdfFile::from_path_with_condition(
        path("12 - Autumn Leaves.pdf"),
        FileCondition::Unreadable,
    )];
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert!(
        plan.deletions().is_empty(),
        "a locked file is not a deletion"
    );
    assert_eq!(plan.failures().len(), 1);

    Reconciler::apply(plan, &mut store).expect("apply");
    let stored = only_note(&store);
    assert_eq!(stored.path(), &path("12 - Autumn Leaves.pdf"));
    assert_eq!(stored.id(), Some(NoteId::new(12)));
    assert_eq!(
        store
            .ocr_text(&digest(1))
            .expect("ocr")
            .expect("text survives")
            .text(),
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
        .upsert_note(&note(
            "12 - Autumn Leaves.pdf",
            1,
            Some(NoteId::new(12)),
            "Autumn Leaves",
        ))
        .expect("seed kept");
    store
        .upsert_note(&note(
            "13 - So What.pdf",
            2,
            Some(NoteId::new(13)),
            "So What",
        ))
        .expect("seed gone");
    store
        .upsert_note(&note("14 - Locked.pdf", 3, Some(NoteId::new(14)), "Locked"))
        .expect("seed locked");

    let files = vec![
        PdfFile::from_path_with_condition(
            path("12 - Autumn Leaves.pdf"),
            FileCondition::Unreadable,
        ),
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
/// Defect 7: the record that already holds an ID must keep it, whatever order
/// the paths sort in. A newcomer whose path is lexicographically smaller must
/// not take the ID away from the record that was indexed first, and repeated
/// identical rescans must reach the same state every time.
#[test]
fn an_indexed_record_keeps_its_id_against_a_smaller_path_newcomer_on_every_rescan() {
    let (mut store, database) = open("ownership");
    store
        .upsert_note(&note("12 - Zeta.pdf", 1, Some(NoteId::new(12)), "Zeta"))
        .expect("seed");

    let files = vec![file("12 - Zeta.pdf", 1), file("12 - Alpha.pdf", 2)];

    for round in 1..=3 {
        let plan = Reconciler::plan(&store, &files).expect("plan");
        assert_eq!(plan.conflicts().len(), 1, "round {round}");
        assert_eq!(
            plan.conflicts()[0].existing(),
            &path("12 - Zeta.pdf"),
            "round {round}: the indexed record keeps the ID"
        );
        assert_eq!(
            plan.conflicts()[0].conflicting(),
            &path("12 - Alpha.pdf"),
            "round {round}: the newcomer is the one written without an ID"
        );

        let report = Reconciler::apply(plan, &mut store).expect("apply");
        assert!(
            report.failures().is_empty(),
            "round {round}: an unresolved conflict is not a write failure"
        );

        let holder = store
            .get_by_id(NoteId::new(12))
            .expect("query")
            .expect("round {round}: exactly one record holds the ID");
        assert_eq!(
            holder.path(),
            &path("12 - Zeta.pdf"),
            "round {round}: ownership does not follow path order"
        );
        let newcomer = store
            .list_all()
            .expect("list")
            .into_iter()
            .find(|note| note.path() == &path("12 - Alpha.pdf"))
            .expect("round {round}: the newcomer stays visible");
        assert!(
            newcomer.id().is_none(),
            "round {round}: the newcomer still holds no ID"
        );
    }
    let _ = std::fs::remove_file(&database);
}

/// Defect 7, continued: ownership is a rule, not a first-pass accident, so it
/// survives a path that sorts before the incumbent's. Once the collision is
/// gone both records go back to a status that is true.
#[test]
fn duplicate_ownership_is_stable_when_the_newcomer_sorts_first() {
    let (mut store, database) = open("ownership-stable");
    let mut incumbent = note("z/12 - Zeta.pdf", 1, Some(NoteId::new(12)), "Zeta");
    incumbent.set_status(IndexStatus::DuplicateIdConflict);
    store.upsert_note(&incumbent).expect("seed");
    let mut newcomer = note("a/12 - Alpha.pdf", 2, None, "Alpha");
    newcomer.set_status(IndexStatus::DuplicateIdConflict);
    store.upsert_note(&newcomer).expect("seed");

    // `a/12 - Alpha.pdf` sorts before `z/12 - Zeta.pdf`, so path order alone
    // would hand the ID to the wrong record.
    let files = vec![file("z/12 - Zeta.pdf", 1), file("a/12 - Alpha.pdf", 2)];
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.conflicts().len(), 1);
    assert_eq!(plan.conflicts()[0].existing(), &path("z/12 - Zeta.pdf"));
    assert_eq!(plan.conflicts()[0].conflicting(), &path("a/12 - Alpha.pdf"));

    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert!(report.failures().is_empty(), "the holder keeps its ID");
    assert_eq!(
        store
            .get_by_id(NoteId::new(12))
            .expect("query")
            .expect("held")
            .path(),
        &path("z/12 - Zeta.pdf")
    );
    let _ = std::fs::remove_file(&database);
}

/// Defect 7, continued: a conflict the user resolves by renaming or deleting
/// the newcomer clears completely — the ID is no longer contested and both
/// statuses stop claiming a conflict that no longer exists.
#[test]
fn a_renamed_or_deleted_newcomer_clears_the_conflict() {
    // Renaming the newcomer to a number of its own moves the collision onto it
    // and away from the indexed record.
    let (mut store, database) = open("resolve-rename");
    store
        .upsert_note(&note("12 - Zeta.pdf", 1, Some(NoteId::new(12)), "Zeta"))
        .expect("seed incumbent");
    let files = vec![file("12 - Zeta.pdf", 1), file("12 - Alpha.pdf", 2)];
    Reconciler::apply(Reconciler::plan(&store, &files).expect("plan"), &mut store).expect("apply");

    let renamed = vec![file("12 - Zeta.pdf", 1), file("13 - Alpha-renamed.pdf", 2)];
    let plan = Reconciler::plan(&store, &renamed).expect("plan");
    assert!(plan.conflicts().is_empty(), "the rename ends the collision");
    assert_eq!(plan.moves().len(), 1, "the record follows its file");

    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert!(report.failures().is_empty());
    let holder = store
        .get_by_id(NoteId::new(12))
        .expect("query")
        .expect("the incumbent keeps 12");
    assert_eq!(holder.path(), &path("12 - Zeta.pdf"));
    let renamed_note = store
        .get_by_id(NoteId::new(13))
        .expect("query")
        .expect("the newcomer keeps its record under the new number");
    assert_eq!(renamed_note.path(), &path("13 - Alpha-renamed.pdf"));
    for stored in [holder, renamed_note] {
        assert_ne!(
            stored.status(),
            IndexStatus::DuplicateIdConflict,
            "a resolved conflict stops marking the record"
        );
    }
    let _ = std::fs::remove_file(&database);

    // Deleting the newcomer clears the same conflict.
    let (mut store, database) = open("resolve-delete");
    store
        .upsert_note(&note("12 - Zeta.pdf", 1, Some(NoteId::new(12)), "Zeta"))
        .expect("seed incumbent");
    Reconciler::apply(Reconciler::plan(&store, &files).expect("plan"), &mut store).expect("apply");

    let plan = Reconciler::plan(&store, &[file("12 - Zeta.pdf", 1)]).expect("plan");
    assert!(plan.conflicts().is_empty());
    assert_eq!(plan.deletions().len(), 1, "the newcomer is deleted");

    Reconciler::apply(plan, &mut store).expect("apply");
    let stored = only_note(&store);
    assert_eq!(stored.id(), Some(NoteId::new(12)));
    assert_ne!(
        stored.status(),
        IndexStatus::DuplicateIdConflict,
        "the survivor is no longer in conflict"
    );
    let _ = std::fs::remove_file(&database);
}

/// Defect 7, continued: the other way to resolve a conflict is to give the
/// newcomer a number of its own by hand. A manual ID is authoritative, so the
/// rescan keeps it and the collision does not come back.
#[test]
fn a_manual_id_edit_clears_the_conflict_and_survives_the_next_rescan() {
    let (mut store, database) = open("resolve-manual");
    store
        .upsert_note(&note("12 - Zeta.pdf", 1, Some(NoteId::new(12)), "Zeta"))
        .expect("seed incumbent");
    let files = vec![file("12 - Zeta.pdf", 1), file("12 - Alpha.pdf", 2)];
    Reconciler::apply(Reconciler::plan(&store, &files).expect("plan"), &mut store).expect("apply");

    // The user types a number for the newcomer by hand.
    let mut edited = store
        .list_all()
        .expect("list")
        .into_iter()
        .find(|note| note.path() == &path("12 - Alpha.pdf"))
        .expect("the newcomer is indexed");
    let mut metadata = edited.metadata().clone();
    metadata.assign_id(Some(NoteId::new(31)), MetadataOrigin::Manual);
    edited = IndexedNote::new(
        path("12 - Alpha.pdf"),
        edited.fingerprint().clone(),
        metadata,
    )
    .with_status(IndexStatus::Discovered);
    store.upsert_note(&edited).expect("the user edit is stored");

    for _ in 0..2 {
        let plan = Reconciler::plan(&store, &files).expect("plan");
        assert!(plan.conflicts().is_empty(), "the hand-set number ends it");
        assert!(plan.failures().is_empty());
        let report = Reconciler::apply(plan, &mut store).expect("apply");
        assert!(report.failures().is_empty());
        assert_eq!(
            store
                .get_by_id(NoteId::new(31))
                .expect("query")
                .expect("the hand-set number is kept")
                .path(),
            &path("12 - Alpha.pdf")
        );
        assert_eq!(
            store
                .get_by_id(NoteId::new(12))
                .expect("query")
                .expect("the incumbent is untouched")
                .path(),
            &path("12 - Zeta.pdf")
        );
    }
    let _ = std::fs::remove_file(&database);
}

/// Defect 8: a file that cannot produce an indexable record must not reserve
/// its content hash. Otherwise a valid copy of the same bytes is reported as a
/// duplicate of a file that was never indexed, and nothing is indexed at all.
#[test]
fn a_file_that_fails_metadata_does_not_reserve_its_content_hash() {
    let (mut store, database) = open("hash-not-claimed");
    // `.pdf` sorts first, has no title, and is not a record anything can own.
    let files = vec![file(".pdf", 1), file("12 - Good.pdf", 1)];

    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.failures().len(), 1);
    assert_eq!(plan.failures()[0].path(), &path(".pdf"));
    assert!(
        matches!(plan.failures()[0].reason(), FileFailureReason::Metadata(_)),
        "the unnamed file is the failure"
    );
    assert_eq!(
        plan.additions().len(),
        1,
        "the valid copy of those bytes is still indexed"
    );
    assert_eq!(plan.additions()[0].note().path(), &path("12 - Good.pdf"));

    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(report.additions().len(), 1);
    assert_eq!(report.failures().len(), 1);
    assert_eq!(only_note(&store).path(), &path("12 - Good.pdf"));

    // The next rescan sees the same thing and reaches the same state.
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.failures().len(), 1);
    assert!(plan.additions().is_empty());
    assert!(plan.updates().is_empty());
    assert!(plan.moves().is_empty());
    Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(only_note(&store).path(), &path("12 - Good.pdf"));
    let _ = std::fs::remove_file(&database);
}

/// Defect 9: a stored record whose file the catalog could not read, or read but
/// found not to be a PDF, is claiming to be fine when it is not. The condition
/// the catalog observed must become the record's status.
#[test]
fn an_unreadable_or_invalid_file_updates_its_record_status() {
    let (mut store, database) = open("condition-status");
    let mut locked = note("12 - Locked.pdf", 1, Some(NoteId::new(12)), "Locked");
    locked.set_status(IndexStatus::Ready);
    store.upsert_note(&locked).expect("seed");
    let mut corrupt = note("13 - Corrupt.pdf", 2, Some(NoteId::new(13)), "Corrupt");
    corrupt.set_status(IndexStatus::Ready);
    store.upsert_note(&corrupt).expect("seed");

    let files = vec![
        PdfFile::from_path_with_condition(path("12 - Locked.pdf"), FileCondition::Unreadable),
        PdfFile::from_path_with_condition(path("13 - Corrupt.pdf"), FileCondition::InvalidPdf),
    ];
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.failures().len(), 2, "both files are still reported");
    assert_eq!(
        plan.updates().len(),
        2,
        "both records are corrected in the same rescan"
    );
    let statuses: HashMap<&RelativePdfPath, IndexStatus> = plan
        .updates()
        .iter()
        .map(|update| (update.note().path(), update.note().status()))
        .collect();
    assert_eq!(
        statuses[&path("12 - Locked.pdf")],
        IndexStatus::Inaccessible,
        "a file that could not be read is inaccessible"
    );
    assert_eq!(
        statuses[&path("13 - Corrupt.pdf")],
        IndexStatus::InvalidPdf,
        "a file that is not a readable PDF says so"
    );
    assert!(plan.deletions().is_empty(), "the files are still on disk");

    Reconciler::apply(plan, &mut store).expect("apply");
    let mut notes = store.list_all().expect("list");
    notes.sort_by_key(|note| note.path().as_str().to_owned());
    assert_eq!(
        notes[0].status(),
        IndexStatus::Inaccessible,
        "the stored row must not contradict what the catalog saw"
    );
    assert_eq!(
        notes[1].status(),
        IndexStatus::InvalidPdf,
        "the stored row must not contradict what the catalog saw"
    );
    for note in &notes {
        assert!(
            note.id().is_some(),
            "correcting a status never takes the ID away"
        );
    }

    // A second identical rescan writes nothing: the status is already true.
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.failures().len(), 2);
    assert!(
        plan.updates().is_empty(),
        "a repeated observation of the same condition costs no write"
    );
    assert!(
        plan.unchanged().is_empty(),
        "and nothing is claimed untouched"
    );
    Reconciler::apply(plan, &mut store).expect("apply");
    let _ = std::fs::remove_file(&database);
}

/// A record whose file becomes readable again leaves the condition behind and
/// goes back through the pipeline.
#[test]
fn a_record_that_becomes_readable_again_returns_to_discovered() {
    let (mut store, database) = open("condition-recovered");
    let mut blocked = note("12 - Locked.pdf", 1, Some(NoteId::new(12)), "Locked");
    blocked.set_status(IndexStatus::Inaccessible);
    store.upsert_note(&blocked).expect("seed");

    let files = vec![file("12 - Locked.pdf", 1)];
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.updates().len(), 1);
    assert_eq!(plan.updates()[0].note().status(), IndexStatus::Discovered);

    Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(only_note(&store).status(), IndexStatus::Discovered);
    let _ = std::fs::remove_file(&database);
}

/// Defect: a record whose file the catalog could not use is still a record
/// that *holds* its note ID, so excluding it from duplicate-ID resolution
/// hides the collision entirely. The newcomer is then planned as a plain
/// addition carrying an ID the store will refuse, and the collision only
/// surfaces as a write failure — leaving the second PDF unindexed and the
/// conflict unreported.
///
/// The record must stay a candidate for ID resolution even when its status
/// needs no correction, so the conflict is reported and both PDFs stay in the
/// library.
#[test]
fn a_condition_failed_record_still_surfaces_a_duplicate_id_conflict() {
    let (mut store, database) = open("condition-conflict");
    let mut holder = note("12 - Zeta.pdf", 1, Some(NoteId::new(12)), "Zeta");
    holder.set_status(IndexStatus::Inaccessible);
    store.upsert_note(&holder).expect("seed");

    let files = vec![
        PdfFile::from_path_with_condition(path("12 - Zeta.pdf"), FileCondition::Unreadable),
        file("12 - Alpha.pdf", 2),
    ];

    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(
        plan.conflicts().len(),
        1,
        "an unreadable incumbent still claims its ID, so the collision is surfaced"
    );
    assert_eq!(plan.conflicts()[0].id(), NoteId::new(12));
    assert_eq!(
        plan.conflicts()[0].existing(),
        &path("12 - Zeta.pdf"),
        "the record the index already filed under 12 keeps it"
    );
    assert_eq!(
        plan.conflicts()[0].conflicting(),
        &path("12 - Alpha.pdf"),
        "the newcomer is the one written without the contested ID"
    );
    assert_eq!(
        plan.failures().len(),
        1,
        "the unreadable file is still reported per file"
    );
    assert_eq!(
        plan.additions().len(),
        1,
        "the newcomer is indexed rather than dropped"
    );
    assert!(
        plan.additions()[0].note().id().is_none(),
        "at most one record holds a contested ID"
    );
    assert_eq!(
        plan.additions()[0].note().status(),
        IndexStatus::DuplicateIdConflict,
        "a record with no condition of its own shows the conflict"
    );

    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(
        report.failures().len(),
        1,
        "the only failure is the unreadable file, not a refused write"
    );

    let mut notes = store.list_all().expect("list");
    notes.sort_by_key(|note| note.path().as_str().to_owned());
    assert_eq!(notes.len(), 2, "both PDFs stay visible in the library");
    assert_eq!(notes[0].path(), &path("12 - Alpha.pdf"));
    assert_eq!(notes[1].path(), &path("12 - Zeta.pdf"));
    assert_eq!(
        store
            .get_by_id(NoteId::new(12))
            .expect("query")
            .map(|note| note.path().clone()),
        Some(path("12 - Zeta.pdf")),
        "exactly one record holds the contested ID"
    );
    assert!(
        notes[0].id().is_none(),
        "the newcomer holds no ID until the user resolves the conflict"
    );
    assert_eq!(
        notes[1].status(),
        IndexStatus::Inaccessible,
        "the observed condition is kept, not overwritten by a conflict marker"
    );

    // Rescanning the same library reaches the same state: the incumbent still
    // holds the ID, the newcomer is still indexed without it, and neither
    // record is rewritten.
    let holder_written = indexed_at(&database, &path("12 - Zeta.pdf"));
    let newcomer_written = indexed_at(&database, &path("12 - Alpha.pdf"));
    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.conflicts().len(), 1, "the collision is still reported");
    assert_eq!(plan.conflicts()[0].conflicting(), &path("12 - Alpha.pdf"));
    assert!(plan.additions().is_empty());
    assert!(
        plan.updates().is_empty(),
        "a rescan of an unchanged library writes nothing"
    );
    Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(
        indexed_at(&database, &path("12 - Zeta.pdf")),
        holder_written,
        "the incumbent is not rewritten"
    );
    assert_eq!(
        indexed_at(&database, &path("12 - Alpha.pdf")),
        newcomer_written,
        "the newcomer is not rewritten"
    );

    // The collision is still resolvable the ordinary ways: deleting the
    // newcomer leaves the incumbent alone, holding the ID and the condition.
    let resolved = vec![PdfFile::from_path_with_condition(
        path("12 - Zeta.pdf"),
        FileCondition::Unreadable,
    )];
    let plan = Reconciler::plan(&store, &resolved).expect("plan");
    assert!(plan.conflicts().is_empty(), "the collision is over");
    assert_eq!(
        plan.deletions().len(),
        1,
        "the newcomer's file is gone, so its record is"
    );
    assert!(
        plan.updates().is_empty(),
        "the incumbent is untouched: it already says what the catalog saw"
    );
    Reconciler::apply(plan, &mut store).expect("apply");
    let survivor = only_note(&store);
    assert_eq!(survivor.id(), Some(NoteId::new(12)));
    assert_eq!(survivor.status(), IndexStatus::Inaccessible);
    let _ = std::fs::remove_file(&database);
}

/// Defect: marking a contested record replaces whatever status it had. A
/// record whose file became unreadable *while* another file claimed its number
/// therefore stored [`IndexStatus::DuplicateIdConflict`] and lost the fact the
/// catalog observed — and because the stored row then contradicts the catalog,
/// every later rescan planned the same correction again.
///
/// Both facts are kept: the row carries the observed condition, and the
/// conflict is reported alongside it every rescan.
#[test]
fn an_unreadable_record_that_is_contested_keeps_its_condition() {
    let (mut store, database) = open("condition-contested");
    let mut holder = note("12 - Zeta.pdf", 1, Some(NoteId::new(12)), "Zeta");
    holder.set_status(IndexStatus::Ready);
    store.upsert_note(&holder).expect("seed");

    let files = vec![
        PdfFile::from_path_with_condition(path("12 - Zeta.pdf"), FileCondition::Unreadable),
        file("12 - Alpha.pdf", 2),
    ];

    let plan = Reconciler::plan(&store, &files).expect("plan");
    assert_eq!(plan.conflicts().len(), 1, "both facts are reported");
    assert_eq!(plan.conflicts()[0].existing(), &path("12 - Zeta.pdf"));
    assert_eq!(plan.conflicts()[0].conflicting(), &path("12 - Alpha.pdf"));
    assert_eq!(plan.failures().len(), 1, "the condition is still reported");
    let incumbent = plan
        .updates()
        .iter()
        .find(|update| update.note().path() == &path("12 - Zeta.pdf"))
        .expect("the observed condition is corrected");
    assert_eq!(
        incumbent.note().status(),
        IndexStatus::Inaccessible,
        "the condition the catalog observed is not discarded for a conflict marker"
    );

    let report = Reconciler::apply(plan, &mut store).expect("apply");
    assert_eq!(report.failures().len(), 1);

    let holder = store
        .get_by_id(NoteId::new(12))
        .expect("query")
        .expect("the incumbent keeps the contested ID");
    assert_eq!(
        holder.status(),
        IndexStatus::Inaccessible,
        "the stored row must not contradict what the catalog saw"
    );
    let newcomer = store
        .list_all()
        .expect("list")
        .into_iter()
        .find(|note| note.path() == &path("12 - Alpha.pdf"))
        .expect("the newcomer stays visible");
    assert!(newcomer.id().is_none());
    assert_eq!(
        newcomer.status(),
        IndexStatus::DuplicateIdConflict,
        "the readable side of the collision shows the conflict"
    );

    // The same library scanned again changes nothing: the stored rows already
    // say both facts, so no correction is re-planned.
    let holder_written = indexed_at(&database, &path("12 - Zeta.pdf"));
    for round in 1..=2 {
        let plan = Reconciler::plan(&store, &files).expect("plan");
        assert_eq!(plan.conflicts().len(), 1, "round {round}");
        assert_eq!(
            plan.conflicts()[0].existing(),
            &path("12 - Zeta.pdf"),
            "round {round}"
        );
        assert_eq!(plan.failures().len(), 1, "round {round}");
        assert!(
            plan.additions().is_empty() && plan.updates().is_empty(),
            "round {round}: an unchanged library is not rewritten"
        );
        Reconciler::apply(plan, &mut store).expect("apply");
        assert_eq!(
            indexed_at(&database, &path("12 - Zeta.pdf")),
            holder_written,
            "round {round}: the incumbent is left alone"
        );
        assert_eq!(
            store
                .get_by_id(NoteId::new(12))
                .expect("query")
                .expect("held")
                .status(),
            IndexStatus::Inaccessible,
            "round {round}: the condition survives every rescan"
        );
    }
    let _ = std::fs::remove_file(&database);
}
