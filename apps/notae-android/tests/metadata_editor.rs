use notae_android::ui::metadata_editor::{EditableMetadata, MetadataEditor};
use notae_android::ui::search::UiError;
use notae_domain::{
    EpochTimestamp, IndexStatus, IndexedNote, MetadataField, MetadataOrigin, NoteId, NoteMetadata,
    PdfFingerprint, RelativePdfPath, Sha256Digest,
};
use notae_index::{IndexError, IndexStore, OcrText, PdfFile, Reconciler};
use sha2::{Digest, Sha256};

fn note(path: &str, bytes: &[u8], id: u32, title: &str) -> IndexedNote {
    let mut metadata =
        NoteMetadata::new(title, Some("Composer".to_owned()), ["original".to_owned()])
            .expect("valid initial metadata");
    metadata.assign_id(Some(NoteId::new(id)), MetadataOrigin::FilenameDerived);
    let mut note = IndexedNote::new(
        RelativePdfPath::new(path).expect("valid path"),
        PdfFingerprint::new(
            Sha256Digest::from_bytes(Sha256::digest(bytes).into()),
            u64::try_from(bytes.len()).expect("fixture fits in u64"),
            Some(EpochTimestamp::new(10, 0).expect("valid timestamp")),
        ),
        metadata,
    );
    note.set_status(IndexStatus::Ready);
    note
}

fn editable(
    note_key: String,
    id: Option<u32>,
    title: &str,
    composer: Option<&str>,
    tags: &[&str],
) -> EditableMetadata {
    EditableMetadata {
        note_key,
        id: id.map_or_else(String::new, |value| value.to_string()),
        title: title.to_owned(),
        composer_or_artist: composer.unwrap_or_default().to_owned(),
        tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
    }
}

#[test]
fn save_normalizes_fields_and_preserves_file_identity_and_pipeline_state() {
    let store = IndexStore::open(":memory:").expect("open test index");
    let original = note("12 - Autumn Leaves.pdf", b"pdf bytes", 12, "Autumn Leaves");
    store
        .reindex_note_with_ocr(&original, Some(&OcrText::new("searchable score")))
        .expect("insert original record");

    MetadataEditor::new(&store)
        .save(editable(
            original.fingerprint().sha256().to_string(),
            Some(42),
            "  New Title  ",
            Some("  New Artist  "),
            &["  Jazz  ", "Jazz", ""],
        ))
        .expect("save valid edits");

    let edited = store.list_all().expect("read edited record").remove(0);
    assert_eq!(edited.path().as_str(), "12 - Autumn Leaves.pdf");
    assert_eq!(edited.fingerprint(), original.fingerprint());
    assert_eq!(edited.status(), IndexStatus::Ready);
    assert_eq!(edited.id(), Some(NoteId::new(42)));
    assert_eq!(edited.title(), "New Title");
    assert_eq!(edited.metadata().composer_or_artist(), Some("New Artist"));
    assert_eq!(
        edited
            .metadata()
            .tags()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["Jazz"]
    );
    for field in MetadataField::ALL {
        assert!(edited.metadata().provenance().is_manual(field));
    }
    assert_eq!(
        store
            .search("searchable score", 10)
            .expect("search retained OCR")
            .len(),
        1
    );
}

#[test]
fn invalid_empty_title_does_not_change_the_index() {
    let store = IndexStore::open(":memory:").expect("open test index");
    let original = note("12 - Autumn Leaves.pdf", b"pdf bytes", 12, "Autumn Leaves");
    store
        .reindex_note(&original)
        .expect("insert original record");

    assert!(
        MetadataEditor::new(&store)
            .save(editable(
                original.fingerprint().sha256().to_string(),
                Some(12),
                "  ",
                Some("Artist"),
                &["jazz"],
            ))
            .is_err()
    );
    assert_eq!(store.list_all().expect("read unchanged index"), [original]);
}

#[test]
fn duplicate_manual_id_is_rejected_without_changing_either_record() {
    let store = IndexStore::open(":memory:").expect("open test index");
    let first = note("01 - First.pdf", b"first", 1, "First");
    let second = note("02 - Second.pdf", b"second", 2, "Second");
    store.reindex_note(&first).expect("insert first record");
    store.reindex_note(&second).expect("insert second record");

    let result = MetadataEditor::new(&store).save(editable(
        second.fingerprint().sha256().to_string(),
        Some(1),
        "Changed",
        Some("Artist"),
        &["new"],
    ));
    assert!(matches!(
        result,
        Err(UiError::Index(IndexError::DuplicateNoteId { .. }))
    ));
    let records = store.list_all().expect("read unchanged records");
    assert_eq!(records.len(), 2);
    assert_eq!(
        records
            .iter()
            .find(|record| record.path().as_str() == "01 - First.pdf")
            .unwrap(),
        &first
    );
    assert_eq!(
        records
            .iter()
            .find(|record| record.path().as_str() == "02 - Second.pdf")
            .unwrap(),
        &second
    );
}

#[test]
fn manual_edits_survive_rescan() {
    let mut store = IndexStore::open(":memory:").expect("open test index");
    let path = "12 - Autumn Leaves.pdf";
    let bytes = b"stable pdf bytes";
    let original = note(path, bytes, 12, "Autumn Leaves");
    store
        .reindex_note(&original)
        .expect("insert initial record");
    let note_key = original.fingerprint().sha256().to_string();
    MetadataEditor::new(&store)
        .save(editable(
            note_key,
            Some(42),
            "Manual title",
            Some("Manual composer"),
            &["manual tag"],
        ))
        .expect("save manual edits");

    let pdf = PdfFile::new(
        original.path().clone(),
        original.fingerprint().clone(),
        path.to_owned(),
    );
    let plan = Reconciler::plan(&store, std::slice::from_ref(&pdf)).expect("plan unchanged rescan");
    Reconciler::apply(plan, &mut store).expect("apply unchanged rescan");
    let rescanned = store.list_all().expect("read rescanned record").remove(0);
    assert_eq!(rescanned.id(), Some(NoteId::new(42)));
    assert_eq!(rescanned.title(), "Manual title");
    assert_eq!(
        rescanned.metadata().composer_or_artist(),
        Some("Manual composer")
    );
    assert_eq!(
        rescanned
            .metadata()
            .tags()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["manual tag"]
    );
}
