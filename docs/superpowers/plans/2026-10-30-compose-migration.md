# Compose Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Slint UI with a Jetpack Compose app (`com.c2yz.notae`, 0.5.0) over a UniFFI Rust core. Drop the desktop indexer and sidecar import, and build the sets features that 0.3.0 deferred.

**Architecture:** The Slint-free Rust (`UiModel`, the state machine, stores, browse/search, operations, backup) moves to `crates/notae-app-core`. It exposes one UniFFI object, `NotaeCore`, which owns the state, pushes `Snapshot`s and asks Kotlin for platform work through `Effect`s. A Gradle app (`apps/notae-android-compose`, renamed at the switch) renders the snapshots in Compose and implements the platform callback interface in Kotlin. Slint keeps running on top of the core until the parity gate passes, then it is deleted.

**Tech Stack:** Rust 1.98.1, UniFFI, Kotlin, Jetpack Compose (BOM) with Material 3 and Navigation 3, AndroidX Activity, Gradle Kotlin DSL with AGP 9, `cargo-ndk` with NDK r28 or newer, Robolectric, Compose UI test, Roborazzi.

**Spec:** `docs/superpowers/specs/2026-10-30-compose-migration-design.md`. Decision record: `docs/adr/0002-compose-ui-over-ffi.md`. Favorites/sets behaviour: `docs/superpowers/specs/2026-10-26-favorites-and-sets-design.md`.

## Global Constraints

- **Start point.** Cut the branch from `main` **after** favorites/sets 0.3.0 merges. That release includes Tasks 1–7, re-linking, and `backup.rs` with the Export button. Before starting, rebase this docs commit onto that `main`.
- **Approved dependencies only:**
  - Build: Gradle/AGP 9, Kotlin, `cargo-ndk`.
  - UI: Compose BOM, Material 3, Navigation 3, AndroidX Activity, AndroidX AppCompat (per-app language).
  - FFI: UniFFI (Rust) with JNA (Kotlin).
  - Tests: Robolectric, Compose UI test, Roborazzi.
  - Not allowed: Coil, Hilt, Play Services, networking, telemetry. Anything else needs the owner's approval.
- **Android ids and SDKs:** `applicationId` and namespace `com.c2yz.notae`. `compileSdk`/`targetSdk` 36, `minSdk` 26. Native libraries are 16 KB-aligned (`-Wl,-z,max-page-size=16384`, NDK r28+). ABI `arm64-v8a` only.
- **Versioning:** version 0.5.0 from the workspace `Cargo.toml`. The versionCode formula is identical to today's (0.2.2 → 16777730).
- **Signing:** use the existing release key, through the existing release secrets.
- **Rust boundaries:** Rust never calls Android APIs and never builds display text. `#![forbid(unsafe_code)]` in every crate. No `unwrap`/`expect` in production code (tests may use them).
- **Strings:** every visible string lives in `res/values/strings.xml` and `res/values-pl/strings.xml`, with `<plurals>` for counts.
- **Look:** One UI as the current Slint app draws it. Copy the tokens from `apps/notae-android/ui/components.slint` (`Theme`) exactly. Use `FontFamily.Default`, light and dark, dynamic colour off, tablet layouts only.
- **Accessibility:** every clickable node has a label, and touch targets are at least 48 dp.
- **Commits:** Conventional Commits ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Never push; the owner pushes.
- **Quality gates** before a phase is done:
  - Rust: `cargo fmt --check`, both clippy runs from `docs/engineering-standards.md`, `cargo test --workspace`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`, `cargo deny check`, `cargo llvm-cov --workspace --summary-only --fail-under-lines 84`.
  - Gradle (once it exists): `./gradlew lint testDebugUnitTest verifyRoborazziDebug assembleRelease`.
  - Slint (until the switch): the arm64 `cargo apk2 check`.

## Review Focus

1. **Android kills Notae while the PDF app is in front.** Coming back shows the same section, set and selection, rebuilt from the data directory, with no blank screen. Test: Task 11 `process_recreation_restores_open_set`; Task 21 checks it on the emulator.
2. **The library folder is revoked, moved or deleted between launches.** The app shows the "permission lost" setup screen, with no crash and no empty library. Tests: Task 7 `revoked_root_snapshot_is_permission_lost`; Task 12 `permission_lost_offers_pick_again`.
3. **Back with a sheet, dialog or reorder mode open.** Back closes the innermost overlay first. On the bare library it returns false and the app goes to the background. Test: Task 7 `back_closes_innermost_first`.
4. **A large library (2,000 PDFs).** Scrolling stays smooth and a snapshot after an action arrives within one frame budget on the tablet. Tests: Task 7 `snapshot_of_2000_rows_is_built`, which records the time and size in the report; Task 13 uses stable `LazyGrid` keys (the selection id); Task 21 measures on the emulator.
5. **A Kotlin exception inside a `PlatformLibrary` call.** An unreadable document becomes a per-file failure in the scan summary, not a crash or a stuck banner. Test: Task 7 `platform_error_becomes_file_failure`.

---

## Phase 1: Baseline

### Task 1: Reference screenshots and parity checklist

**Files:**
- Create: `docs/compose-parity/README.md` (the checklist), `docs/compose-parity/slint/*.png`

- [ ] **Step 1:** Install the 0.3.0 APK on the tablet AVD described in `docs/android-debugging.md`. Seed a library from `fixtures` that has repeats in a set, a missing piece, favorites and a long title.
- [ ] **Step 2:** Capture every screen and state as `<screen>-<state>-<portrait|landscape>-<light|dark>.png`:
  - setup: unconfigured, permission lost, error
  - library: all, recent, favorites, composer, grid and list, sidebar collapsed, empty states
  - the ⋮ sheet
  - details, plus the metadata editor
  - search: empty, results, no results
  - settings
  - the operation banner: running, done, failed
  - Sets: grid and list
  - the set dialog, with its error
  - delete confirmation
  - inside a set, with repeats and a missing piece
  - the add-to-set sheet
  - the add picker
- [ ] **Step 3:** Write the checklist. List one line per user action reachable in 0.3.0, grouped by screen, as `- [ ] <action> → <expected result>`. Add the deferred features from the favorites spec: reorder, text notes, "Just opened", import, and first-launch import.
- [ ] **Step 4: Commit** `docs(compose): capture Slint reference screenshots and the parity checklist`.

## Phase 2: Android-only cleanup

### Task 2: Archive OCR, remove the desktop indexer

**Files:**
- Delete: `apps/notae-index/`, `crates/notae-ocr/`
- Modify: root `Cargo.toml` (members), `apps/notae-android/Cargo.toml`, `apps/notae-android/src/storage.rs` (host module), `.github/workflows/ci.yml`, `deny.toml` if it names OCR crates, `README.md`, `CONTRIBUTING.md`, `docs/engineering-standards.md`
- Create: `apps/notae-android/tests/support/fs_catalog.rs` (a host `FilesystemCatalog` and `ValidatedRoot`, the minimum the tests use)

- [ ] **Step 1:** `git branch archive/notae-ocr HEAD`. Record the SHA in the report; the owner pushes it.
- [ ] **Step 2:** Move the parts of `notae-index-cli`'s `FilesystemCatalog` that `storage.rs` and `tests/storage_state.rs` use into the test helper, then drop the `cfg(not(android))` dependency.
- [ ] **Step 3:** Delete both crates. Remove from CI: the Tesseract, Leptonica and Fontconfig apt steps, `TESSDATA_PREFIX`, the ubuntu-24.04 pin and its comment, and the `notae-index-cli --help` smoke. Rewrite the README, CONTRIBUTING and the engineering-standards sections that describe the indexer and Tesseract so they describe an Android-only app. Mark the "desktop indexer" decision in `2026-10-09-notae-design.md` as superseded by ADR 0002.
- [ ] **Step 4: Verify.** `cargo metadata --format-version 1 | grep -c '"notae-ocr"\|"notae-index-cli"'` prints `0`. The Rust gates pass, including llvm-cov at ≥ 84%; report the new figure. The arm64 Slint check passes.
- [ ] **Step 5: Commit** `refactor!: drop the desktop indexer and archive desktop OCR`.

### Task 3: Remove sidecar import

**Files:**
- Modify:
  - `crates/notae-index/src/{lib.rs,import.rs,sidecar.rs}`: delete the importer, manifest and sidecar reader. Keep only what `store.rs` and `schema.rs` still use, such as provenance values already stored in user databases.
  - `apps/notae-android/src/{state.rs,ui.rs,storage.rs,saf_tree.rs,operations/*,android/saf.rs,android_main.rs}`
  - `ui/*.slint` (import review)
  - the `.po` catalog
- Delete: `src/operations/import.rs`, `tests/import_review_ui.rs`, and the sidecar tests in `crates/notae-index/tests/`

- [ ] **Step 1:** Remove `UiEvent::{ConfirmImport, CancelImport}`, `OperationEvent`'s import variants, `DesktopImportCheck`, `ImportPreview*`, `LibraryCatalog::sidecar_manifest`, `discover_sidecar_manifest`, and `OperationController::{preview_import, confirm_import, discard_import_preview}`.
- [ ] **Step 2:** Add a test in `crates/notae-index/tests/` named `opens_database_written_with_sidecar_provenance`. It opens a database fixture that contains imported-sidecar rows and checks that search still returns those rows. Existing users' indexes must keep opening.
- [ ] **Step 3: Verify.** `git grep -il 'sidecar' -- '*.rs' '*.slint'` lists only the provenance kept in Step 1. The Rust gates and the arm64 check pass.
- [ ] **Step 4: Commit** `refactor!: remove desktop sidecar import`.

## Phase 3: Core crate and API

### Task 4: Extract `crates/notae-app-core`

**Files:**
- Create: `crates/notae-app-core/{Cargo.toml,src/lib.rs}`
- Move these modules from `apps/notae-android/src/` with `git mv`, keeping their history:
  - `state.rs`, `ui.rs` and `ui/{browse,library,metadata_editor}.rs`
  - `ui/search.rs`, the Slint-free part (`SearchRow`, `split_snippet`, `UiError`)
  - `favorites.rs`, `sets.rs`, `recent.rs`, `ui_prefs.rs`, `thumbnails.rs`, `backup.rs`
  - `storage.rs` without its Android module
  - `operations/`, `pdf_intent.rs` (the `ContentUri` types), `pdf_pages.rs` (`classify_text_layer`)
- Move with them: their logic tests in `tests/`, `tests/support/fs_catalog.rs`
- Modify: `apps/notae-android` (depends on the core; `library_row` and anything that touches Slint stays in the app)

**Interfaces:**
- Produces: `notae_app_core::{UiModel, AppController, AppState, Screen, Setup, SearchRow, LibrarySection, SetStore, Favorites, backup, …}` with today's names and signatures, except that `UiModel` holds `Arc<Mutex<StorageState>>`. `UiModel: Send`.

- [ ] **Step 1: Write the failing test** `crates/notae-app-core/tests/send.rs`: `fn ui_model_is_send() { fn assert_send<T: Send>() {} assert_send::<notae_app_core::UiModel>(); }`.
- [ ] **Step 2: Move the modules.** Swap `Rc` for `Arc` and use `#![forbid(unsafe_code)]`. The moved tests pass under `cargo test -p notae-app-core`. The Slint tests that remain pass under `cargo test -p notae-android`.
- [ ] **Step 3: Verify.** `rg -n 'slint' crates/notae-app-core` is empty. The Rust gates and the arm64 check pass.
- [ ] **Step 4: Commit** `refactor(core): move the Slint-free app logic into notae-app-core`.

### Task 5: Typed text instead of `pick()`

**Files:**
- Create: `crates/notae-app-core/src/text.rs`, `apps/notae-android/src/texts.rs` (the Slint mapping)
- Modify: every core module that calls `pick`/`polish_plural`, including `browse.rs`, `ui.rs`, `search.rs` and `operations/*`. The Slint app's `sync_window`.

**Interfaces:**
- Produces, in `text.rs`:
  - `pub enum SectionTitle { All, Recent, Favorites, Sets, Composer(String), Set(String) }`
  - `pub struct SetSubtitle { pub date: Option<String> /* ISO */, pub pieces: usize, pub notes: usize }`
  - `pub enum Edited { Today, Yesterday, DaysAgo(u32), On(String /* ISO */) }`
  - `pub enum RowStatus { Ready, TextPending, MissingFromLibrary, Unreadable, InvalidPdf }`
  - `pub enum StatusMessage { … }`, one variant per message the core builds today, with its numbers as fields
  - `pub struct PositionOf { pub position: usize, pub total: usize }`
- The fields that hold text today (`section_title()`, `SearchRow.status`, the banner text, the subtitle) carry these types instead.
- `apps/notae-android/src/texts.rs` maps each type to today's exact English and Polish strings with `pick`, so 0.3.x screens are unchanged.

- [ ] **Step 1: Failing tests** in the core: `set_subtitle_counts_pieces_and_notes`, `edited_buckets_by_day` (today, yesterday, 3 days, 10 days → `On`), `missing_piece_has_missing_status`.
- [ ] **Step 2: Implement.** Remove `language.rs` from the core. `rg -n 'pick\(|polish_plural' crates/notae-app-core` is empty.
- [ ] **Step 3: Verify.** The existing Slint tests `tests/language_texts.rs`, `tests/translations.rs` and the browse tests pass unchanged (the visible strings are identical).
- [ ] **Step 4: Commit** `refactor(core): return typed values instead of display text`.

### Task 6: `PlatformLibrary` boundary

**Files:**
- Create: `crates/notae-app-core/src/platform.rs`
- Modify: `crates/notae-app-core/src/{storage.rs,operations/rescan.rs}`, `apps/notae-android/src/android/{saf.rs,pdf_pages.rs}` (Slint implements the trait)

**Interfaces:**
- Produces:
  - `pub trait PlatformLibrary: Send + Sync { fn enumerate(&self, handle: String) -> Result<Vec<DocumentEntry>, PlatformError>; fn extract_text_layer(&self, handle: String, path: String) -> Result<Option<String>, PlatformError>; fn render_thumbnail(&self, handle: String, path: String, out_png: String) -> Result<bool, PlatformError>; }`
  - `pub struct DocumentEntry { pub path: String, pub display_name: String, pub size: u64, pub content: DocumentContent }`
  - `pub enum DocumentContent { Sha256(String), Unreadable, InvalidPdf }`
  - `pub enum PlatformError { Unavailable { message: String }, PermissionLost }`
  - `pub struct PlatformCatalog`, which implements `LibraryCatalog` and `PdfCatalog` over a `PlatformLibrary`
- The trait's shape is UniFFI-ready: owned `String`s, no lifetimes.

- [ ] **Step 1: Failing tests** with a `FakePlatform` in `tests/support`:
  - `catalog_maps_entries_to_pdf_files`: an invalid path is dropped and reported
  - `unreadable_entry_becomes_inaccessible`
  - `permission_lost_maps_to_storage_permission_lost`
- [ ] **Step 2: Implement.** The Slint SAF code moves its hashing, text and thumbnail work behind the trait. Its behaviour is unchanged.
- [ ] **Step 3: Verify.** The Rust gates and the arm64 check pass.
- [ ] **Step 4: Commit** `refactor(core): put platform access behind PlatformLibrary`.

### Task 7: The UniFFI API

**Files:**
- Create: `crates/notae-app-core/src/api.rs`, `crates/notae-app-core/src/api/{action.rs,snapshot.rs,effect.rs}`, `crates/notae-app-core/uniffi.toml`, `crates/notae-app-core/tests/api.rs`
- Modify: `crates/notae-app-core/Cargo.toml`: add `uniffi` (approved), `crate-type = ["lib", "cdylib"]`

**Interfaces:**
- Produces, all as UniFFI exports:
  - `#[derive(uniffi::Object)] pub struct NotaeCore`
  - `#[uniffi::constructor] fn new(data_dir: String, platform: Arc<dyn PlatformLibrary>, listener: Arc<dyn CoreListener>) -> Arc<Self>`
  - `fn dispatch(&self, action: Action)`
  - `fn back(&self) -> bool`
  - `fn snapshot(&self) -> Snapshot`
  - `fn library_root_picked(&self, display_name: String, handle: String)`
  - `fn file_written(&self, ok: bool)`
  - `fn import_file_read(&self, text: String)`
- `#[uniffi::export(with_foreign)] pub trait CoreListener: Send + Sync { fn on_snapshot(&self, snapshot: Snapshot); fn on_effect(&self, effect: Effect); }`. `PlatformLibrary` from Task 6 is exported `with_foreign` as well.
- `Action`: one variant per callback the Slint app wires today, in `android_main.rs` and `wire_sets`, plus the deferred-feature actions that Tasks 18–20 add. List every variant in the report.
- `Effect`:
  - `PickLibraryRoot`
  - `OpenPdf { uri: String, note_key: String, item_id: Option<String> }`
  - `StartVoiceSearch`
  - `WriteExport { suggested_name: String, json: String }`
  - `PickImportFile`
- `Snapshot`: the fields listed in spec §4.2, as UniFFI records built from the Task 5 types.
- Behaviour:
  - `dispatch` applies the action under the lock and then calls `on_snapshot`. It never calls `PlatformLibrary` on the caller's thread.
  - A core thread drains `OperationController::recv_event_timeout`, applies each event and pushes a snapshot.

- [ ] **Step 1: Failing tests** in `tests/api.rs`, using `FakePlatform` and a `RecordingListener`:
  - `fresh_core_starts_unconfigured`
  - `picking_root_and_rescan_produces_library_rows`
  - `back_closes_innermost_first`. The table: sheet → dialog → reorder mode → details → search or settings → set → sets → the library returns `false`.
  - `revoked_root_snapshot_is_permission_lost`
  - `platform_error_becomes_file_failure`
  - `opening_a_set_piece_emits_open_pdf_with_item_id`
  - `snapshot_of_2000_rows_is_built`. Log the elapsed time and the serialized size, and assert only that it completes.
- [ ] **Step 2: Implement.**
- [ ] **Step 3: Verify.** `cargo run -p notae-app-core --bin uniffi-bindgen generate --library target/debug/libnotae_app_core.so --language kotlin --out-dir target/uniffi-kotlin` writes `notae_app_core.kt`. The Rust gates pass.
- [ ] **Step 4: Commit** `feat(core): expose NotaeCore over UniFFI`.

## Phase 4: The Gradle app

### Task 8: Gradle skeleton and the native build

**Files:**
- Create under `apps/notae-android-compose/`:
  - `settings.gradle.kts`, `build.gradle.kts`, `gradle/libs.versions.toml`, the Gradle wrapper
  - `app/build.gradle.kts`
  - `app/src/main/AndroidManifest.xml`
  - `app/src/main/kotlin/com/c2yz/notae/MainActivity.kt`
  - `app/src/test/kotlin/com/c2yz/notae/{CoreSmokeTest.kt,VersionCodeTest.kt}`
- Create: `apps/notae-android-compose/version.gradle.kts`, which holds `fun versionCodeFor(version: String): Int`, ported from the current derivation (find it with `rg -n versionCode .github apps/notae-android`).

**Interfaces:**
- Produces:
  - A Gradle task `cargoNdkBuild` (arm64-v8a, release profile, `-Wl,-z,max-page-size=16384`) feeding `jniLibs`.
  - `generateUniffiBindings` feeding `src/main/kotlin`.
  - `cargoHostBuild`, which builds the host `.so` for unit tests, found by JNA through `jna.library.path`.
- Manifest:
  - `<supports-screens android:smallScreens="false" android:normalScreens="false" android:largeScreens="true" android:xlargeScreens="true"/>`
  - `android:enableOnBackInvokedCallback="true"`
  - the PDF and speech `<queries>` copied from today's Cargo metadata
  - `configChanges` for locale and layoutDirection

- [ ] **Step 1: Failing tests:**
  - `VersionCodeTest`: `versionCodeFor("0.2.2") == 16777730`, `versionCodeFor("0.5.0") > versionCodeFor("0.3.0")`.
  - `CoreSmokeTest` (Robolectric): construct `NotaeCore` with a Kotlin fake `PlatformLibrary` and a recording listener. Assert that `snapshot().screen` is the unconfigured setup.
- [ ] **Step 2: Implement.** `MainActivity` sets edge-to-edge and shows the placeholder text "Notae".
- [ ] **Step 3: Verify:**
  - `./gradlew testDebugUnitTest assembleDebug assembleRelease` passes; release signing comes from the env vars documented in `docs/android-release.md`.
  - `unzip -p app-release.apk lib/arm64-v8a/libnotae_app_core.so > /tmp/x.so && llvm-readelf -l /tmp/x.so | grep -E 'LOAD.*0x4000'` shows 16 KB alignment.
  - The APK installs on the tablet AVD and launches.
- [ ] **Step 4: Commit** `build(android): add the Gradle Compose app and the cargo-ndk core build`.

### Task 9: Theme, icons and base components

**Files:**
- Create:
  - `app/src/main/kotlin/com/c2yz/notae/ui/theme/{Tokens.kt,NotaeTheme.kt}`
  - `ui/components/{NotaeCard.kt,PillButton.kt,IconButton.kt,SidebarItem.kt,ActionSheet.kt,NotaeDialog.kt}`
  - `res/drawable/ic_*.xml`, converted from `apps/notae-android/ui/material-icons/*.svg`
  - `app/src/test/kotlin/.../ui/ComponentsScreenshotTest.kt`
- Modify: `app/build.gradle.kts` to add Roborazzi

**Interfaces:**
- Produces:
  - `object Tokens`, which mirrors every value in the Slint `Theme`, light and dark. Copy the names, converting kebab-case to camelCase, and keep the values exact.
  - `@Composable fun NotaeTheme(dark: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit)`, with `FontFamily.Default` and no dynamic colour.
  - `fun SemanticsNodeInteractionsProvider.assertAllClickablesLabelled()` in `app/src/test/.../ui/A11y.kt`, used by every screen task.

- [ ] **Step 1: Failing test** `tokens_match_slint_theme`. It parses `apps/notae-android/ui/components.slint`'s `Theme` colour and length values from the source file and asserts that each one equals the matching `Tokens` value.
- [ ] **Step 2: Implement** the components with the Slint component's geometry: padding, radius, height and the 48 dp minimum.
- [ ] **Step 3:** Run Roborazzi screenshots of each component in light and dark, and record the baselines.
- [ ] **Step 4: Commit** `feat(android): add the One UI theme and base components in Compose`.

### Task 10: Platform layer

**Files:**
- Create in `app/src/main/kotlin/com/c2yz/notae/platform/`:
  - `SafLibrary.kt`: `PlatformLibrary` over `DocumentsContract`; streaming SHA-256 with `MessageDigest`; `PdfRenderer` text on API 35+ (`Page.getTextContents`); a first-page PNG thumbnail.
  - `FolderPicker.kt`: `OpenDocumentTree`, plus `takePersistableUriPermission` with read only.
  - `PdfOpener.kt`: `ACTION_VIEW` with `FLAG_GRANT_READ_URI_PERMISSION`, keeping today's viewer behaviour (port it from `src/android/pdf_intent.rs`).
  - `VoiceSearch.kt`: `RecognizerIntent`.
  - `Motion.kt`: `ANIMATOR_DURATION_SCALE`.
  - `Language.kt`: `AppCompatDelegate.setApplicationLocales` / `getApplicationLocales` on all APIs; `res/xml/locales_config.xml` (en, pl) and `AppLocalesMetadataHolderService` with `autoStoreLocales` in the manifest. `MainActivity` extends `AppCompatActivity`.
  - `Dates.kt`: `fun formatIsoDate(iso: String, locale: Locale): String` using `DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM)`.
- Test: `app/src/test/kotlin/.../platform/{HashingTest.kt,PdfOpenerTest.kt,SafLibraryTest.kt}`

- [ ] **Step 1: Failing tests:**
  - `sha256_of_fixture_matches_rust`: a fixture PDF's hex equals the content key the Rust tests use for it.
  - `pdf_intent_grants_read_and_targets_pdf_mime`.
  - `enumerate_reports_unreadable_document`: Robolectric `ShadowContentResolver` with a document that throws on open.
  - `revoked_tree_maps_to_permission_lost`.
  - `iso_date_formats_per_locale`: `2026-11-01` formats with day, month and year for `Locale.ENGLISH` and `Locale("pl")`, and differs between them.
  - `language_round_trips_through_appcompat`: set `pl`, read back `pl`, under Robolectric `@Config(sdk = [30])`.
- [ ] **Step 2: Implement.** Every exception inside a `PlatformLibrary` method becomes a `PlatformError`. None escape into UniFFI as a panic.
- [ ] **Step 3: Commit** `feat(android): implement the platform layer in Kotlin`.

### Task 11: App shell and navigation

**Files:**
- Create in `app/src/main/kotlin/com/c2yz/notae/`:
  - `core/NotaeViewModel.kt`
  - `core/EffectHandler.kt`
  - `nav/BackStack.kt`
  - `nav/NotaeNavDisplay.kt`
- Test: `app/src/test/.../nav/BackStackTest.kt`, `app/src/test/.../core/RecreationTest.kt`

**Interfaces:**
- Produces:
  - `class NotaeViewModel(app: Application) : AndroidViewModel`. It holds `val snapshot: StateFlow<Snapshot>`, plus `fun dispatch(action: Action)` and `fun back(): Boolean`. It creates `NotaeCore` from `filesDir` with `SafLibrary`.
  - `fun backStackFor(snapshot: Snapshot): List<NavKey>`, following the table in spec §4.4.
  - `EffectHandler` launches the activity-result contracts and calls the matching core callback (`library_root_picked`, `file_written`, `import_file_read`).
- Library and Details render as a Navigation 3 list-detail scene.
- `NavigationBackHandler` calls `back()` and finishes the activity when it returns false.

- [ ] **Step 1: Failing tests:**
  - `back_stack_for_each_screen`: one assertion per row of the spec table.
  - `process_recreation_restores_open_set`: Robolectric `ActivityScenario.recreate()` after opening a set; the new ViewModel's first snapshot shows the same set and selection.
- [ ] **Step 2: Implement**, using placeholder screens that show only the screen name.
- [ ] **Step 3: Commit** `feat(android): add the view model, effects and Navigation 3 shell`.

## Phase 5: Screens

Every screen task has the same steps:

- [ ] **Step 1: Failing Robolectric tests.** Write them for the task's listed actions: click or type, assert the `Action` the fake core received, and render from a crafted `Snapshot`. Each task's test file also calls `assertAllClickablesLabelled()`.
- [ ] **Step 2: Implement** the screen against `docs/compose-parity/slint/` and the `Tokens`. Add its strings to both `strings.xml` files, copying today's Polish from the `.po` catalog and `apps/notae-android/src/texts.rs`.
- [ ] **Step 3: Record Roborazzi screenshots** for portrait and landscape at 1280×800 and 800×1280, in light and dark. Put the matching `slint/` reference beside each in the report.
- [ ] **Step 4: Run** `./gradlew testDebugUnitTest verifyRoborazziDebug`. It passes.
- [ ] **Step 5: Commit** `feat(android): add the <screen> screen in Compose`.

### Task 12: Setup screens
Covers unconfigured, permission lost and error. Actions: pick a folder (emits the `PickLibraryRoot` effect), pick again, and "Import from a previous Notae", shown only when favorites and sets are empty and wired in Task 20. Tests: `unconfigured_pick_folder_emits_effect`, `permission_lost_offers_pick_again`.

### Task 13: Library screen
Covers:
- the sidebar: sections with counts, collapse
- the header: title, grid/list toggle, search icon, rescan
- the score grid and list: thumbnail, title, composer, status, favorite badge, set position
- the ⋮ sheet: open, details, favorite, add to set, remove from set
- empty states and the operation banner, with cancel and dismiss

`LazyVerticalGrid` and `LazyColumn` use the selection id as `key`. Tests:
- `grid_and_list_follow_library_view`
- `overflow_sheet_offers_favorite_and_add_to_set`
- `favorite_badge_shows`
- `banner_cancel_dispatches_cancel`

### Task 14: Details pane and metadata editor
Covers previous/next by selection id, "X of Y" from `PositionOf`, open, the favorite star, and edit metadata, save and cancel. Tests:
- `second_copy_keeps_position`, with the set A, B, A
- `next_disabled_on_last`
- `metadata_save_dispatches_fields`

### Task 15: Search screen
Covers the text field with the IME focused on open, scope chips, results with highlighted snippet parts (`split_snippet`), voice search (emits the `StartVoiceSearch` effect), Ctrl K and Escape. Tests:
- `typing_dispatches_query`
- `scope_chip_dispatches_scope`
- `ctrl_k_opens_search`

### Task 16: Settings screen
Covers the library folder (change), rescan, language (English, Polish, system default), "Export sets" (`WriteExport`), "Import sets" (Task 20) and about with the version. Tests:
- `language_switch_dispatches_appcompat_locale` (Robolectric `@Config(sdk = [30])` and `[34]`)
- `export_emits_write_export`
- `polish_plurals_1_2_5_22_25`, a plural check on the item-count strings

### Task 17: Sets list, set view, dialogs, add-to-set and the picker
Covers:
- folder cards and set rows with "Edited …" (from `Edited`)
- the set dialog: name, date as a locale-formatted field opening the M3 `DatePickerDialog` with clear (ISO to the core), notes as multi-line; field errors
- set dates shown with `formatIsoDate`
- delete confirmation naming the set
- the set header: back, title, `SetSubtitle`, notes line, Add, Reorder, search
- the add-to-set sheet, with checks and New set
- the add picker, with its search, add/added toggles and the "n added" footer

Tests:
- `new_set_requires_name`
- `date_picker_returns_iso_and_shows_locale_format`
- `delete_names_the_set`
- `add_to_set_sheet_checks_member_sets`
- `picker_untoggle_removes_only_session_item`

## Phase 6: Deferred sets features and data import

### Task 18: Reorder mode
Core: `Action::{ReorderStarted, ReorderFinished, MoveSetItem { item_id, delta }, MoveSetItemTo { item_id, index }}`. Add `SetStore::move_item_to(set_id, item_id, index, now) -> bool` with core tests: `move_item_to_clamps`, `move_item_to_bumps_modified`. UI: list rows with a drag handle (a Compose drag gesture that calls `MoveSetItemTo` on drop), up and down (disabled at the edges), remove, and the accessibility labels from favorites spec §5.4. Done restores the previous layout. Tests:
- `up_disabled_on_first_row`
- `move_down_then_up_restores_order`
- `done_restores_grid_layout`
- `drag_to_index_dispatches_move_to`

### Task 19: Text notes
Core: `SetStore::edit_note(set_id, item_id, text, now) -> bool`, with the test `edit_note_bumps_modified`, plus `Action::{AddNote { text }, EditNote { item_id, text }}`. UI:
- the add picker's "Text note" tab: multi-line text, "Added at the end of the set.", Add disabled while empty
- tinted cards and rows using `Tokens.note`/`Tokens.onNote` (light `#FFF4E0`/`#6B3A00`, dark `#3A2A12`/`#FFD9A0`)
- the note's ⋮ offers Edit note and Remove from set
- previous/next skip notes

Tests:
- `add_note_appends_tinted_card`
- `empty_note_cannot_be_added`
- `details_previous_next_skip_notes`

### Task 20: "Just opened" and import
Core:
- The `OpenPdf` effect with an `item_id` calls `SetStore::mark_opened` and saves. `modified` is unchanged.
- The snapshot marks `just_opened` on that one item.
- `Action::{ImportRequested, ImportConfirmed, ImportCancelled}` use `backup::{parse_export, plan_import, apply_import}`.
- The confirmation lists the replaced set names.
- The first-launch offer from Task 12 uses the same flow.

Tests:
- core: `opening_item_marks_just_opened_without_bumping_modified`, `import_cancel_changes_nothing`
- UI: `import_confirmation_names_replaced_sets`, `just_opened_chip_is_announced`

## Phase 7: Parity and the switch

### Task 21: Parity check on the emulator
- [ ] Build the release APK and install it beside 0.3.0 on the tablet AVD. Go through every line of `docs/compose-parity/README.md` and tick it, or open a fix inside this task.
- [ ] Capture the same screenshot set as Task 1 into `docs/compose-parity/compose/` and add a side-by-side table to the README.
- [ ] Check the Review Focus items by hand:
  - kill the app with `adb shell am kill` while the PDF app is in front, then return
  - revoke the folder permission
  - a 2,000-PDF library: record the snapshot time and check for jank with `adb shell dumpsys gfxinfo`
  - TalkBack through every screen
- [ ] Commit `test(android): pass the Compose parity check`.

### Task 22: Remove Slint and switch the build
- Delete:
  - `apps/notae-android`: the Slint crate, `ui/`, `src/android/`, the Java sources, `texts.rs`, `language.rs`, the `.po` catalog, the Slint tests
  - the cargo-apk2 metadata, `.github/scripts/build-apk.sh` (replaced by a Gradle equivalent) and the `unsafe_code = "deny"` override
- Rename `apps/notae-android-compose` to `apps/notae-android` with `git mv`.
- Rewrite `ci.yml`:
  - the Rust gates on `ubuntu-latest`
  - a Gradle job: lint, unit tests, Roborazzi verify, `assembleRelease`, the 16 KB check
  - an emulator smoke job: install, launch, pick a fixture folder, open Sets
- Rewrite `release.yml`: build the AAB and APK, verify, attach the APK.
- Update `docs/engineering-standards.md` (gates, RUST-002 resolved, COND-005 closed), `docs/android-release.md`, `docs/android-debugging.md`, `README.md` and `CONTRIBUTING.md`.
- Set the workspace version to `0.5.0`.
- Add a `CHANGELOG.md` `[Unreleased]` entry that covers:
  - the new app id
  - export from 0.3.0 and import on first launch
  - that scanned PDFs are found by file name only
  - the removed desktop indexer
- Before deleting `components.slint`, convert Task 9's `tokens_match_slint_theme` test to assert the values it currently reads, written out as literals.
- [ ] Verify all gates. `rg -n -i 'slint|cargo-apk2|notae-index-cli|tesseract' --glob '!docs/superpowers/**' --glob '!CHANGELOG.md' --glob '!docs/adr/**'` is empty.
- [ ] Commit `feat(android)!: switch Notae to the Compose app`.

## Self-review

- Spec coverage, mapping each spec section to its tasks:
  - §3 decisions: tasks 2, 3, 4, 7, 8, 11 and 22
  - §4.1 API: tasks 6–7
  - §4.2 threads and snapshot: task 7
  - §4.3 strings: tasks 5 and 12–17
  - §4.4 navigation: task 11
  - §4.5 look: tasks 1, 9 and 21
  - §4.6 migration: tasks 12 and 20
  - §5 build, CI and release: tasks 8 and 22
  - §6 removal: tasks 2, 3 and 22
  - §7 risks: Review Focus 4 and task 21
  - §8 phases: phases 1–7
- Owner rulings 2026-10-30: AppCompat added for per-app language on every API; dates are locale-formatted with a date picker; Gradle stays in `apps/notae-android-compose` until the switch; drag-to-reorder ships.
