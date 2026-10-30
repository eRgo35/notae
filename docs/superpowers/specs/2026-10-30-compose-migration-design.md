# Compose migration: design

Status: agreed with the owner in a design interview, 2026-10-30. Target release: 0.5.0.
Decision record: [ADR 0002](../../adr/0002-compose-ui-over-ffi.md).

## 1. Why

Notae's Android UI is Slint inside a `NativeActivity`. Accessibility, the IME and the One UI look are all hand-built. The `cargo-apk2` and NDK r26d pairing caps the target SDK at 34. Before a Google Play release the owner wants:

- native accessibility
- native text input
- predictive back
- a current SDK
- a look as close to Samsung One UI as possible

The Rust core is kept. Only the UI layer and the Android platform glue change.

Success: 0.5.0 installs as `com.c2yz.notae`. It looks like 0.3.0 screen for screen, and every 0.3.0 action works. The sets features that 0.3.0 left out also work: reorder, text notes, "Just opened" and import. TalkBack reads every control. Favorites and sets exported from 0.3.0 import on first launch.

## 2. Goals, non-goals

Goals:

- Jetpack Compose UI over a UniFFI Rust core, with parity to 0.3.0 plus the deferred sets features.
- Android only: remove the desktop indexer, sidecar import and desktop OCR.
- A Gradle build: AAB and APK, target SDK 36, min SDK 26, 16 KB-aligned native libraries.
- Tests: Rust logic tests, Robolectric Compose tests, Roborazzi screenshots, and one emulator smoke test.

Non-goals:

- The Play Store submission itself, a separate step.
- On-device OCR. It is deferred, and its code is kept on `archive/notae-ocr`.
- Phone layouts.
- New features beyond the deferred sets work.
- Changing the release signing key.

Anti-goals, failures even if the app works:

- A Compose app that looks generic Material instead of matching 0.3.0.
- Business logic moving into Kotlin.
- Any network, telemetry or Google Play Services dependency.

## 3. Owner decisions (2026-10-30)

| # | Decision |
| --- | --- |
| 1 | Reasons: accessibility and native input first; the look is a bonus. |
| 2, 16 | Favorites/sets: 0.3.0 (Slint) ships Tasks 1–7 of the favorites plan, re-linking of replaced PDFs and an "Export sets" button. Reorder, text notes, "Just opened" and import are built in Compose only. |
| 3 | Gradle with Kotlin DSL and AGP 9, plus `cargo-ndk`. `cargo-apk2` is removed. |
| 4 | One switch on `feat/compose-migration`. Slint is deleted only after parity. |
| 5 | UniFFI generates the Kotlin bindings. |
| 6, 15 | 0.3.0 is a GitHub-only Slint release. The Compose app is 0.5.0, with no 0.4.0. versionCode keeps its current formula. Play is a separate step. |
| 7 | Rust stops calling Android. Kotlin owns every platform API and passes plain data in. |
| 8 | Rust owns the screen state and pushes snapshots. Kotlin renders them and sends actions. |
| 9 | Strings go to `strings.xml` plus `values-pl`. Rust returns data and codes, never text. Per-app language stays. |
| 10, 21 | Material 3 themed to replicate the current Slint tokens: colours, radii, sizes, spacing. System font, light and dark, dynamic colour off, tablet only. |
| 11 | Tests: Rust logic, Robolectric Compose, Roborazzi screenshots, one emulator smoke test in CI. |
| 12 | The application id is `com.c2yz.notae`. |
| 13 | Target SDK 36, min SDK 26. |
| 14 | Record the change in ADR 0002 and update the core design spec. |
| 17 | 0.3.0 exports; 0.5.0's first launch offers "Import from a previous Notae". |
| 18 | Keep the existing release key. |
| 19 | Slint-free Rust moves to `crates/notae-app-core`. |
| 20 | Navigation 3, back stack derived from Rust state; Back calls the core. |
| 22 | Reference screenshots of every Slint screen, plus a parity checklist, made before the switch. |
| 23, 27 | Remove `apps/notae-index`, sidecar import, the Tesseract CI setup and the indexer docs inside this plan. 0.3.0 still has import. |
| 24 | Dependencies approved: Gradle/AGP 9, Kotlin, `cargo-ndk`, Compose BOM, Material 3, Navigation 3, AndroidX Activity, UniFFI with JNA, Robolectric, Compose UI test, Roborazzi. |
| 25 | These docs live on `feat/compose-migration`, cut from `main`. |
| 26, 28 | `crates/notae-ocr` moves to `archive/notae-ocr` and is removed from the workspace. OCR is later work. |

## 4. Architecture

```
apps/notae-android-compose/          Gradle project (renamed to apps/notae-android at the switch)
  app/src/main/kotlin/com/c2yz/notae/
    MainActivity.kt                  single ComponentActivity, edge-to-edge, predictive back
    core/NotaeViewModel.kt           owns the NotaeCore handle, exposes StateFlow<Snapshot>
    platform/                        SafLibrary (PlatformLibrary impl), Pickers, PdfOpener,
                                     VoiceSearch, Language, Motion
    ui/theme/                        NotaeTheme: tokens copied from the Slint Theme
    ui/<screen>/                     one package per screen
  app/src/main/res/values{,-pl}/strings.xml
crates/notae-app-core/               Rust: UiModel, state machine, stores, browse/search,
                                     operations, backup, the UniFFI API (src/api.rs)
crates/notae-domain, crates/notae-index   unchanged, except that sidecar import is removed
```

### 4.1 The core API (UniFFI)

One object, `NotaeCore`, built with:

- the app's data directory, which holds `recent.txt`, `ui-prefs.txt`, `favorites.json`, `sets.json`, the index database and thumbnails
- a `PlatformLibrary` callback interface
- a `CoreListener` callback interface

Calls from Kotlin:

- `dispatch(action: Action)`. `Action` is one enum for every user action, replacing today's Slint callbacks and `UiEvent`.
- `back() -> bool`. It returns false when nothing is left to close, and Kotlin then lets the system finish.
- `snapshot() -> Snapshot`, used for the initial render.

Calls into Kotlin:

- `CoreListener.on_snapshot(Snapshot)` after every state change.
- `CoreListener.on_effect(Effect)`. Effects are the platform work Rust asks Kotlin to do: open a PDF, launch the folder picker, start voice search, write an export file, pick an import file.

Kotlin implements `PlatformLibrary`. Rust calls it on the worker thread:

- `enumerate(handle) -> Vec<DocumentEntry>`, where each entry has the relative path, display name, size, and either a SHA-256 hex or a condition of unreadable / invalid PDF.
- `extract_text_layer(handle, path) -> Option<String>`
- `render_thumbnail(handle, path, out_png) -> bool`

These replace today's `LibraryCatalog`, `PdfCatalog` and `PdfPageBackend` implementations in `src/android/`.

### 4.2 State and threads

- `UiModel` moves into the core unchanged in behaviour. Its `Rc<Mutex<StorageState>>` becomes `Arc`, so `NotaeCore` (`Mutex<UiModel>` plus the `OperationController`) is `Send + Sync`, as UniFFI requires.
- One core thread drains worker events. Today that is the 16 ms `slint::Timer`; it becomes a blocking `recv_event_timeout` loop. The thread applies each event under the lock and calls `on_snapshot`.
- `dispatch` applies the action under the lock, then calls `on_snapshot`.
- Kotlin writes each snapshot into a `MutableStateFlow`, which is thread-safe, and Compose collects it.
- `Snapshot` is a UniFFI record containing:
  - the screen
  - the sidebar sections with their counts
  - the visible rows and set entries
  - the selection and neighbours
  - search state and results
  - the operation banner
  - settings
  - open dialogs and sheets
  - the add picker
  - transient status codes
- Rows carry a thumbnail file path. Kotlin decodes it with `BitmapFactory` into an in-memory LRU cache. No image library is added.

### 4.3 Strings

Every text that Rust builds with `pick()` today becomes a typed field, and Kotlin formats it from `strings.xml`. The typed forms include:

- section titles
- the set subtitle: date, pieces count, notes count
- "Edited …": today, yesterday, n days ago, or on a date
- "Missing from library"
- operation and status messages
- "X of Y"

Plurals use `<plurals>`. Language selection uses the framework `LocaleManager` on API 33 and newer, which matches Android's per-app language settings. On API 26–32 the app follows the system language and Settings hides the in-app switch. `androidx.appcompat` is not on the approved list. The language preference leaves `ui-prefs.txt`.

### 4.4 Navigation and back

The back stack is a `List<NavKey>` computed from `Snapshot.screen`:

| Screen | Back stack |
| --- | --- |
| Setup | [Setup] |
| Library section | [Library(section)] |
| Set | [Library(sets), Set(id)] |
| Details open | […, Details(selection)] |
| Search | […, Search] |
| Settings | […, Settings] |

On a tablet, Library and Details render as a list-detail scene, as in today's details pane. `NavigationBackHandler` calls `core.back()`. Navigation 3 animates the change when the next snapshot arrives. Dialogs and sheets are snapshot fields, not navigation entries.

### 4.5 Look

`NotaeTheme` copies the Slint `Theme` tokens one for one: colours for light and dark, the accent, corner radii, row and card sizes, spacing and type sizes. It uses `FontFamily.Default`, which is SamsungOne on Samsung devices. Components keep their current geometry: cards, rows, sidebar, pill buttons, icon buttons, sheets and dialogs. The Material icons are the same SVGs, converted to vector drawables.

### 4.6 Data migration

On first launch, when favorites and sets are both empty, the setup screen offers "Import from a previous Notae". It runs the same import as Settings, through `ACTION_OPEN_DOCUMENT` and `backup::apply_import`. The library folder is picked again as normal.

## 5. Build, CI, release

- Gradle wrapper, Kotlin DSL, AGP 9. `compileSdk` and `targetSdk` are 36; `minSdk` is 26.
- `cargo-ndk` builds `notae-app-core` for `arm64-v8a` with NDK r28 or newer, linked with `-Wl,-z,max-page-size=16384`.
- UniFFI bindgen generates the Kotlin bindings at build time.
- A host build of the core for `x86_64-unknown-linux-gnu` is loaded through JNA in Robolectric tests.
- versionCode uses today's formula from the workspace version, ported to Gradle. 0.5.0 must give a higher value than 0.3.0.
- Signing reads the existing `CARGO_APK_RELEASE_KEYSTORE*` secrets, renamed for Gradle in the workflow.
- CI runs the Rust gates (fmt, clippy ×2, tests, docs, deny, llvm-cov ≥ 84%) plus Gradle: lint, unit and Robolectric tests, Roborazzi verify, and `assembleRelease`. An emulator smoke test runs on the tablet AVD. The Tesseract apt setup and the ubuntu-24.04 pin go away.
- `release.yml` builds the AAB and APK, verifies them and attaches the APK to the GitHub Release.

## 6. Removal

| Removed | Notes |
| --- | --- |
| `apps/notae-index` | The host tests' filesystem catalog moves to a test helper in `notae-app-core`. |
| `crates/notae-ocr` | Pushed to `archive/notae-ocr` first. |
| Sidecar import | `SidecarImporter`, manifest discovery, the import preview flow and screen. |
| Slint UI | `ui/`, `android_main.rs`, `src/android/`, `NotaeActivity.java`, `NotaeAccessibility.java`, the accessibility bridge, `voice_search.rs`, `language.rs` (`pick`), the `.po` catalog and the Slint UI tests. |
| Tooling | The `cargo-apk2` metadata and build script, the `unsafe_code = "deny"` exception, and the Tesseract CI and docs. |

## 7. Risks

- Snapshot size on large libraries. The full snapshot is sent on every change, so measure it at 2,000 rows. If it shows, split the rows out behind a revision number.
- UniFFI callbacks running on the worker thread while Kotlin does blocking SAF I/O. That is acceptable because it is the worker thread, but `dispatch` must never block on platform calls.
- Parity drift. The reference screenshots and checklist are the gate, not memory.

## 8. Phases

1. Baseline: reference screenshots and the parity checklist from 0.3.0.
2. Android-only cleanup: archive OCR, remove the indexer and sidecar import.
3. Core crate and UniFFI API, with Slint still running on top of the core.
4. Gradle app skeleton, theme and the platform layer.
5. Screens.
6. The deferred sets features and data import.
7. Parity check, the switch, CI and release workflows, docs, 0.5.0.
