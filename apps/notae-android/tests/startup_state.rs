use std::sync::{Arc, Mutex};

use notae_android::operations::events::OperationEvent;
use notae_android::storage::{
    FilesystemLibraryRootProvider, LibraryRootProvider, StorageError, StorageState,
};
use notae_android::ui::{StartupPresentation, UiModel};

fn selected_source() -> (
    tempfile::TempDir,
    FilesystemLibraryRootProvider,
    notae_android::storage::LibraryRoot,
    notae_android::storage::LibrarySource,
) {
    let directory = tempfile::TempDir::new().expect("temporary library");
    let provider = FilesystemLibraryRootProvider::new();
    provider.queue_directory(directory.path().to_path_buf());
    let root = pollster::block_on(provider.select_root()).expect("select library");
    let source = provider.reopen(&root).expect("open library");
    (directory, provider, root, source)
}

#[test]
fn saved_root_stays_neutral_until_first_rows_arrive() {
    let (_directory, _provider, _root, source) = selected_source();
    let mut model = UiModel::new();
    assert_eq!(model.startup_presentation(), StartupPresentation::Restoring);

    model.set_storage(StorageState::Ready(Arc::new(Mutex::new(source))));
    assert_eq!(
        model.startup_presentation(),
        StartupPresentation::LoadingRows
    );

    model
        .apply_operation_event(OperationEvent::SearchRows(Vec::new()))
        .expect("empty initial rows are valid");
    assert_eq!(model.startup_presentation(), StartupPresentation::Ready);
}

#[test]
fn no_saved_root_shows_setup_after_restore_finishes() {
    let mut model = UiModel::new();
    model.finish_startup_without_library();
    assert_eq!(model.startup_presentation(), StartupPresentation::Ready);
    assert!(matches!(
        model.storage_kind(),
        notae_android::ui::StorageStateKindView::Unconfigured
    ));
}

#[test]
fn revoked_saved_grant_shows_recovery_instead_of_loading_forever() {
    let (_directory, provider, root, _source) = selected_source();
    provider.revoke_persisted(&root);
    let mut model = UiModel::new();
    model.set_storage(StorageState::PermissionLost(root));
    assert_eq!(model.startup_presentation(), StartupPresentation::Ready);
}

#[test]
fn failed_initial_query_shows_an_actionable_failure() {
    let (_directory, _provider, _root, source) = selected_source();
    let mut model = UiModel::new();
    model.set_storage(StorageState::Ready(Arc::new(Mutex::new(source))));
    model
        .apply_operation_event(OperationEvent::Failed {
            reason: "Could not load library rows".to_owned(),
        })
        .expect("worker failure is projected");
    assert_eq!(model.startup_presentation(), StartupPresentation::Ready);
    assert!(matches!(
        model.status().operation,
        notae_android::state::OperationState::Failed { .. }
    ));
}

#[test]
fn storage_error_during_restore_shows_setup_error() {
    let mut model = UiModel::new();
    model.set_storage(StorageState::Error(StorageError::Other(
        "Could not open saved library".to_owned(),
    )));
    assert_eq!(model.startup_presentation(), StartupPresentation::Ready);
    assert!(matches!(
        model.storage_kind(),
        notae_android::ui::StorageStateKindView::Error(message)
            if message.contains("Could not open saved library")
    ));
}
