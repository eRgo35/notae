//! What the app shows while a saved library is restored at startup.

use std::sync::{Arc, Mutex};

use notae_app_core::operations::events::OperationEvent;
use notae_app_core::storage::{LibraryRootProvider, StorageError, StorageState};
use notae_app_core::ui::{StartupPresentation, UiModel};

#[path = "support/fs_catalog.rs"]
mod fs_catalog;
use fs_catalog::FilesystemLibraryRootProvider;

fn selected_source() -> (
    tempfile::TempDir,
    FilesystemLibraryRootProvider,
    notae_app_core::storage::LibraryRoot,
    notae_app_core::storage::LibrarySource,
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
        notae_app_core::ui::StorageStateKindView::Unconfigured
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
            failure: notae_app_core::text::Failure::WorkerStopped,
        })
        .expect("worker failure is projected");
    assert_eq!(model.startup_presentation(), StartupPresentation::Ready);
    assert!(matches!(
        model.status().operation,
        notae_app_core::state::OperationState::Failed { .. }
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
        notae_app_core::ui::StorageStateKindView::Error(StorageError::Other(detail))
            if detail == "Could not open saved library"
    ));
}
