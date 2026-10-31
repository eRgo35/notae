//! The content URI and the view-intent contract for an external PDF viewer.

use notae_app_core::pdf_intent::{
    ContentUri, LaunchResult, ViewerError, launch_result, pdf_intent_spec,
};

#[test]
fn samsung_notes_wins_only_when_it_resolves_the_pdf() {
    assert_eq!(launch_result(true, 3), Ok(LaunchResult::SamsungNotes));
}

#[test]
fn chooser_fallback_requires_a_compatible_viewer() {
    assert_eq!(launch_result(false, 1), Ok(LaunchResult::SystemChooser));
    assert_eq!(
        launch_result(false, 0),
        Err(ViewerError::NoCompatibleViewer)
    );
}

#[test]
fn only_content_uris_can_be_opened_externally() {
    assert!(ContentUri::new("content://provider/document/123").is_ok());
    assert_eq!(
        ContentUri::new("/data/user/0/org.notae/files/note.pdf"),
        Err(ViewerError::InvalidContentUri)
    );
    assert_eq!(
        ContentUri::new("file:///data/local/tmp/note.pdf"),
        Err(ViewerError::InvalidContentUri)
    );
}

#[test]
fn pdf_handoff_grants_read_access_only() {
    let intent = pdf_intent_spec(
        ContentUri::new("content://provider/document/123").expect("SAF content URI"),
    );
    assert_eq!(intent.mime_type(), "application/pdf");
    assert_eq!(intent.grant_flags() & 0x0000_0001, 0x0000_0001);
    assert_eq!(intent.grant_flags() & 0x0000_0002, 0);
}
