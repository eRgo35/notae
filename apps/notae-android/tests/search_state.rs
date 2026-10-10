use std::path::PathBuf;

use notae_android::state::{AppController, EventError, Screen, Transition, UiEvent};
use notae_android::ui::library::LibraryViewModel;
use notae_domain::{
    IndexStatus, IndexedNote, MetadataOrigin, NoteId, NoteMetadata, PdfFingerprint,
    RelativePdfPath, Sha256Digest,
};
use notae_index::{IndexStore, OcrText};

fn store() -> IndexStore {
    IndexStore::open(":memory:").expect("open in-memory index")
}

fn note(
    path: &str,
    digest_byte: u8,
    id: Option<u32>,
    title: &str,
    composer: Option<&str>,
    tags: &[&str],
    status: IndexStatus,
) -> IndexedNote {
    let mut metadata = NoteMetadata::new(
        title,
        composer.map(str::to_owned),
        tags.iter().map(|tag| (*tag).to_owned()),
    )
    .expect("valid title");
    metadata.assign_id(id.map(NoteId::new), MetadataOrigin::FilenameDerived);
    let mut note = IndexedNote::new(
        RelativePdfPath::new(path).expect("valid relative path"),
        PdfFingerprint::new(Sha256Digest::from_bytes([digest_byte; 32]), 1_024, None),
        metadata,
    );
    note.set_status(status);
    note
}

fn insert(store: &IndexStore, note: &IndexedNote, ocr: Option<&str>) {
    let ocr = ocr.map(OcrText::new);
    store
        .upsert_note_with_ocr(note, ocr.as_ref())
        .expect("insert indexed note");
}

#[test]
fn empty_library_has_no_search_rows() {
    let store = store();
    let model = LibraryViewModel::new(&store);

    assert!(model.search().expect("search empty index").is_empty());
}

#[test]
fn empty_query_projects_rows_in_numeric_order_with_metadata_status_and_stable_key() {
    let store = store();
    let numbered_ten = note(
        "ten.pdf",
        10,
        Some(10),
        "Sonata",
        Some("Ludwig van Beethoven"),
        &["piano", "classical"],
        IndexStatus::OcrPending,
    );
    let numbered_two = note(
        "two.pdf",
        2,
        Some(2),
        "Prelude",
        None,
        &["keyboard"],
        IndexStatus::OcrIncomplete,
    );
    let unnumbered = note(
        "unnumbered.pdf",
        3,
        None,
        "Etude",
        Some("Artist"),
        &[],
        IndexStatus::DuplicateIdConflict,
    );
    insert(&store, &numbered_ten, None);
    insert(&store, &numbered_two, None);
    insert(&store, &unnumbered, None);

    let model = LibraryViewModel::new(&store);
    let rows = model.search().expect("browse all rows");

    assert_eq!(
        rows.iter().map(|row| row.id.as_deref()).collect::<Vec<_>>(),
        [Some("2"), Some("10"), None]
    );
    assert_eq!(rows[1].title, "Sonata");
    assert_eq!(
        rows[1].composer_or_artist.as_deref(),
        Some("Ludwig van Beethoven")
    );
    assert_eq!(rows[1].tags, ["classical", "piano"]);
    assert_eq!(rows[0].status, "OCR incomplete");
    assert_eq!(rows[1].status, "OCR pending");
    assert_eq!(rows[2].status, "ID conflict");
    assert_eq!(
        rows[1].note_key,
        "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a"
    );
}

#[test]
fn changing_query_filters_rows_and_preserves_exact_match_ranking() {
    let store = store();
    let exact_id = note(
        "id-seven.pdf",
        7,
        Some(7),
        "Etude",
        None,
        &[],
        IndexStatus::Ready,
    );
    let exact_title = note(
        "title-seven.pdf",
        50,
        Some(50),
        "7",
        None,
        &[],
        IndexStatus::Ready,
    );
    let ocr_only = note(
        "ocr-seven.pdf",
        2,
        Some(2),
        "Exercises",
        None,
        &[],
        IndexStatus::Ready,
    );
    insert(&store, &exact_id, None);
    insert(&store, &exact_title, None);
    insert(&store, &ocr_only, Some("7"));

    let mut model = LibraryViewModel::new(&store);
    model.set_query("7".to_owned());
    let rows = model.search().expect("search exact numeric ID");
    assert_eq!(
        rows.iter().map(|row| row.id.as_deref()).collect::<Vec<_>>(),
        [Some("7"), Some("50"), Some("2")]
    );

    model.set_query("not present".to_owned());
    assert!(model.search().expect("search unmatched query").is_empty());
}

#[test]
fn settings_navigation_requires_a_configured_library() {
    let mut controller = AppController::new();

    assert_eq!(
        controller.dispatch(UiEvent::OpenSettings),
        Err(EventError::SettingsUnavailable)
    );
    assert_eq!(
        controller.dispatch(UiEvent::CloseSettings),
        Err(EventError::SettingsUnavailable)
    );
    assert!(matches!(controller.state().screen(), Screen::Setup(_)));

    controller
        .dispatch(UiEvent::PickLibraryRoot {
            root: PathBuf::from("library"),
        })
        .expect("choose library");
    assert_eq!(
        controller.dispatch(UiEvent::OpenSettings),
        Ok(Transition::NoOp)
    );
    assert_eq!(controller.state().screen(), &Screen::Settings);
    assert_eq!(
        controller.dispatch(UiEvent::CloseSettings),
        Ok(Transition::NoOp)
    );
    assert_eq!(controller.state().screen(), &Screen::Library);
}
