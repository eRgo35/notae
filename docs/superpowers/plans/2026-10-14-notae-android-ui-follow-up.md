# Notae Android UI Follow-up Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve the four post-install usability findings recorded in the One UI redesign plan: persistent operation status, compact search/settings layouts, persistent scan access and details actions, and removal of motion controls/animations.

**Architecture:** Keep Rust `UiModel`/`AppState` authoritative for operation and library data; project one shared status view into `AppWindow` and render it on Library, Search, and Settings. Keep Slint as the sole UI layer and reuse the existing SAF folder picker, scan worker, and host-testable state/projection patterns. Adapt the current screens in place; do not add crates or invent storage metrics.

**Tech Stack:** Rust 2024 workspace, Slint 1.18, `notae-index`, existing Android SAF/JNI bridge, host Rust integration tests.

**Spec:** `docs/superpowers/plans/2026-10-14-notae-android-one-ui-redesign.md`, sections “Post-install findings and follow-up design” and “Global Constraints”; `docs/superpowers/specs/2026-10-09-notae-design.md` for existing product/storage constraints. This follow-up supersedes contradictory motion-preference and animated-progress instructions in Tasks 1, 8, and 11 of the original plan.

## Preconditions

- Execute only in an isolated feature branch/worktree that contains the One UI redesign implementation. The current checkout has extensive user-owned uncommitted work on `main`; preserve it and do not reset, stash, commit, or overwrite it as part of this plan.
- Before implementation, compare the isolated workspace against the current UI interfaces in `apps/notae-android/src/ui.rs`, `src/state.rs`, `src/android_main.rs`, and `ui/*.slint`. Reconcile intentional branch differences before dispatching tasks.
- Emulator/tablet visual checks require the configured Android SDK/NDK and devices. If unavailable, complete host checks and report device checks as unverified; do not claim visual verification.

## Global Constraints

- No new crate dependencies. Java code may use only the Android SDK.
- Fully local: no network at runtime.
- Notae never copies, renames or moves PDFs. Thumbnails are written only to the app’s private `internal_data_path()/notae/thumbnails`.
- `min_sdk_version = 23`, `target_sdk_version = 34` stay as they are.
- `apps/notae-android/src/state.rs` and `src/ui.rs` keep `#![forbid(unsafe_code)]`; new host modules also declare it.
- Touch targets at least 48×48 px; text contrast at least 4.5:1 against its background in both themes.
- The UI has no animated transitions or state effects; remove the Motion preference and all reduced-motion plumbing.
- Pending text is an index state, not active Android OCR. Never describe it as OCR currently running.
- Settings may show only facts Notae can report; do not add guessed storage usage/counts or controls.

## Current Interfaces and Relevant Files

| Path | Current responsibility | Follow-up |
|---|---|---|
| `apps/notae-android/src/state.rs` | Host-testable `AppState`, `OperationState`, transitions | Preserve operation phase/progress/failure and outcome semantics; add state only if a needed fact is not already derivable. |
| `apps/notae-android/src/ui.rs` | `UiModel`, rows, storage view, operation-event projection | Project current status, pending-text count, and selected library display name without leaking storage handles. |
| `apps/notae-android/src/operations/events.rs` | Worker events and scan/import summaries | Reuse existing progress, summary, conflict, and failure data; add an event only for a real missing fact. |
| `apps/notae-android/src/android_main.rs` | Slint synchronization, picker and scan callback wiring | Supply shared chrome/status properties and keep Scan using the existing `rescan_clicked` path. |
| `apps/notae-android/ui/app.slint` | Screen composition and global properties/callbacks | Render shared app chrome/status consistently on Library, Search, Settings. |
| `apps/notae-android/ui/library.slint` | Library navigation and operation banner | Remove screen-specific duplicate status/action rendering after shared integration. |
| `apps/notae-android/ui/search.slint` | Search field, scope chips, result list | Give results the remaining viewport height; keep chips horizontally scrollable and independently usable. |
| `apps/notae-android/ui/setup.slint` | Welcome and Settings screens | Replace generic settings and Motion preference with location, maintenance, privacy, and About sections. |
| `apps/notae-android/ui/details.slint` | Score details panel | Make Edit details icon-only with accessible label. |
| `apps/notae-android/ui/components.slint` | Shared One UI tokens and controls | Remove `reduced-motion` token and all conditional animation support. |
| `apps/notae-android/src/android/theme.rs` | Android theme bridge | Remove motion-preference JNI call; leave fixed/system color handling intact. |
| `apps/notae-android/tests/operation_state.rs`, `tests/search_state.rs`, `tests/storage_state.rs`, `tests/smoke.rs` | Host behavior tests | Extend only for observable state/projection behavior. |
| `docs/android-debugging.md` | Android manual verification | Add follow-up checks and document any unavailable device verification. |

## Tasks

### Task 1: Shared operation status across Library, Search, and Settings

**Files:**
- Modify: `apps/notae-android/src/ui.rs`
- Modify: `apps/notae-android/src/android_main.rs`
- Modify: `apps/notae-android/ui/app.slint`
- Modify: `apps/notae-android/ui/library.slint`
- Modify: `apps/notae-android/ui/search.slint`
- Modify: `apps/notae-android/ui/setup.slint`
- Test: `apps/notae-android/tests/operation_state.rs`, `apps/notae-android/tests/smoke.rs`

**Interfaces:**
- Consume `OperationState`, `OperationEvent`, current `SearchRow.text_pending`, and `StorageStateKindView` already present in the host model.
- Produce `UiStatus` from `UiModel::status()`, with `operation: OperationState`, `latest_rescan: Option<RescanSummary>`, and `pending_text_count: usize`. Derive these from current controller state, the last successful `RescanSummary`, and current rows; retain the last successful scan summary independently of the current operation, including through import completion, cancellation, and failure.
- Keep setup/recovery behavior authoritative to storage state. Status must remain visible across screen changes and must not claim Android is running OCR when rows are only marked pending.

- [ ] **Step 1: Add host tests for status transitions and pending count**

In `tests/operation_state.rs`, assert `UiModel::status()` returns the current `OperationState`, latest successful `RescanSummary`, and count of `SearchRow`s where `text_pending` is true. Cover Idle, RescanInProgress, RescanFinalizing, Completed, and Failed states. Complete a rescan, apply an import, and verify `latest_rescan` remains the completed scan summary. Verify pending rows do not turn an idle operation into an active scan/OCR state.

- [ ] **Step 2: Run the focused host tests and observe the expected failure**

Run: `cargo test -p notae-android --test operation_state`
Expected: the new projection tests fail to compile before `UiStatus` and `UiModel::status()` exist.

- [ ] **Step 3: Implement the smallest status projection**

Add public `UiStatus` in `src/ui.rs` and `UiModel::status() -> UiStatus`. Save the typed `RescanSummary` on `OperationEvent::Completed` and do not clear it for import completion, cancellation, or failure; derive operation from `AppState` and pending count from `search_rows`. Keep human-readable phase/failure text derived in one place in `sync_window`; do not add timers, polling, another state machine, or fabricated progress.

- [ ] **Step 4: Pass and render the same snapshot on all configured screens**

Add shared Slint properties for status text, progress fraction, pending-text count, cancel state, and acknowledge state. Render a compact status surface for Library, Search, and Settings; keep scan status visible while switching screens. Preserve Cancel only during cancellable scanning and Dismiss only when an outcome can be acknowledged. For pending text, show a count and explain that Android does not run OCR; mention desktop OCR/sidecar import only as an available next step, not an active Android operation.

- [ ] **Step 5: Verify status behavior**

Run: `cargo test -p notae-android --test operation_state --test smoke && cargo build -p notae-android`
Expected: status transitions and screen-model behavior pass; Slint compiles on host. On Android, manually start a scan, switch among Library/Search/Settings during progress, complete it, and verify the same status/outcome remains visible until dismissed.

- [ ] **Step 6: Commit the task**

```sh
git add apps/notae-android/src/ui.rs apps/notae-android/src/android_main.rs apps/notae-android/ui/app.slint apps/notae-android/ui/library.slint apps/notae-android/ui/search.slint apps/notae-android/ui/setup.slint apps/notae-android/tests/operation_state.rs apps/notae-android/tests/smoke.rs
git commit -m "fix(android): keep library status visible across screens"
```


### Task 2: Persistent Scan action and accessible Edit details action

**Files:**
- Modify: `apps/notae-android/ui/app.slint`
- Modify: `apps/notae-android/ui/library.slint`
- Modify: `apps/notae-android/ui/search.slint`
- Modify: `apps/notae-android/ui/setup.slint`
- Modify: `apps/notae-android/ui/details.slint`
- Modify: `apps/notae-android/src/android_main.rs`
- Test: `apps/notae-android/tests/smoke.rs`

**Interfaces:**
- Consume existing `rescan_enabled`, `rescan_clicked`, scan worker callbacks, and `IconButton`/`Glyph` controls.
- Produce a persistent Scan icon action in shared app chrome on Library, Search, and Settings. It is disabled exactly when a scan cannot be started; all three entry points invoke the same callback.
- Produce an icon-only details edit action with accessible label `Edit details`; preserve its existing callback and 48×48 px minimum touch target.

- [ ] **Step 1: Add a host test for shared scan eligibility**

Extend `tests/smoke.rs` or the existing operation-state tests to verify the action’s enabled state follows whether the operation is idle and that non-idle operations disable it. Keep UI wiring verification in the Slint build/device scenario, not a tautological callback test.

- [ ] **Step 2: Move Scan into shared screen chrome**

Place a Scan `IconButton` in `app.slint` chrome visible in Library, Search, and Settings. Bind `enabled` to `rescan_enabled`, provide the accessible label `Scan library`, and route the click to `rescan_clicked`. Remove duplicate scan actions from per-screen components so the action appears once per screen. Keep any currently exposed cancel/dismiss actions in the status surface from Task 1.

- [ ] **Step 3: Replace the details text button with the accessible icon action**

In `details.slint`, use the existing edit icon asset and `IconButton` for `Edit details`; retain the `edit()` callback. Set its label to exactly `Edit details`; do not remove the visible accessible name in favor of an unlabeled glyph.

- [ ] **Step 4: Build and exercise actions**

Run: `cargo build -p notae-android && cargo test -p notae-android --test smoke --test operation_state`
Expected: Slint compiles and model tests pass. On the Android UI, verify Scan is present on Library, Search, and Settings; it starts the same scan from each; it is disabled while a scan/import prevents a new scan; Edit details remains accessible and opens the editor.

- [ ] **Step 5: Commit the task**

```sh
git add apps/notae-android/ui/app.slint apps/notae-android/ui/library.slint apps/notae-android/ui/search.slint apps/notae-android/ui/setup.slint apps/notae-android/ui/details.slint apps/notae-android/src/android_main.rs apps/notae-android/tests/smoke.rs
git commit -m "fix(android): keep scan action available across screens"
```

### Task 3: Capability-based Settings and library location

**Files:**
- Modify: `apps/notae-android/ui/setup.slint`
- Modify: `apps/notae-android/ui/app.slint`
- Modify: `apps/notae-android/src/ui.rs`
- Modify: `apps/notae-android/src/android_main.rs`
- Modify: SAF picker callback wiring in `apps/notae-android/ui/app.slint` and `src/android_main.rs`
- Test: `apps/notae-android/tests/storage_state.rs`, `tests/operation_state.rs`

**Interfaces:**
- Consume current storage display name, existing SAF folder selection/recovery flow, `UiStatus.latest_rescan`, pending count, and package version.
- Change `StorageStateKindView::Ready` to carry only `display_name: String`; `UiModel::storage_kind()` must never expose the opaque tree URI.
- Produce Settings sections only for Library location, Library maintenance, Local storage and privacy, and About. Location shows the selected folder display name and invokes the existing SAF picker. Maintenance shows only the latest reported scan outcome, pending-text count, conflicts, and failures. Privacy copy explains PDFs stay in place and index/thumbnail data is local; do not expose file counts or storage usage absent a source of truth.

- [ ] **Step 1: Add tests for selected-folder display name and Settings projection**

Extend `tests/storage_state.rs` to assert a ready local catalog returns its root display name and never its root handle, and permission loss returns the reconnect display name. Extend `tests/operation_state.rs` to assert maintenance reads conflicts/failures from `UiStatus.latest_rescan` and pending count from rows; when no scan summary exists, the UI has no fabricated scan counts. The rescan-then-import retention behavior is tested in Task 1.

- [ ] **Step 2: Run focused tests before implementation**

Run: `cargo test -p notae-android --test storage_state --test operation_state`
Expected: the tests fail to compile before the Ready view and retained scan summary are added.

- [ ] **Step 3: Add Settings data and picker wiring**

Expose `display_name` from `StorageStateKindView::Ready` by reading `source.root().display_name()`. Add a Settings `change-library` callback routed through `AppWindow.setup_chosen` to the existing `window.on_setup_chosen` picker handler. Add the app package version from `env!("CARGO_PKG_VERSION")` as an `AppWindow` property. Do not pass or display the tree URI.

- [ ] **Step 4: Rebuild Settings content and compact layout**

Pass `available-height` from `AppWindow` into `SettingsScreen`. Put the four sections in one vertical scroll region bounded by `available-height`; remove Motion and decorative copy. The Library location section shows folder name and Change/Reconnect; maintenance shows the Scan action and available result/count details; privacy copy says PDFs remain in place and local index/thumbnails are stored locally; About shows Notae, package version, and concise product information. Keep controls at least 48×48 px.

- [ ] **Step 5: Run Settings verification**

Run: `cargo test -p notae-android --test storage_state --test operation_state --test smoke && cargo build -p notae-android && cargo clippy -p notae-android --all-targets -- -D warnings`
Expected: host tests, Slint compilation, and clippy pass. On an emulator at narrow width, verify folder display/change, all four sections, available scan outcome/counts, and vertical scrolling.

- [ ] **Step 6: Commit the task**

```sh
git add apps/notae-android/ui/setup.slint apps/notae-android/ui/app.slint apps/notae-android/src/ui.rs apps/notae-android/src/android_main.rs apps/notae-android/tests/storage_state.rs apps/notae-android/tests/operation_state.rs
git commit -m "fix(android): show actionable library settings"
```

### Task 4: Compact Search layout with keyboard open

**Files:**
- Modify: `apps/notae-android/ui/search.slint`
- Modify: `apps/notae-android/ui/app.slint`

**Interfaces:**
- Consume keyboard-adjusted `AppWindow.available-height` and the existing horizontal `ScrollView`, `ListView`, and `SearchScreen` inputs/callbacks.
- Produce a horizontally scrollable scope row that remains usable at compact heights and a result list occupying the remaining viewport height with independent scrolling.

- [ ] **Step 1: Pass the adjusted viewport height**

Add an `available-height` input property to `SearchScreen` and pass the existing keyboard-adjusted `AppWindow.available-height` from `app.slint`.

- [ ] **Step 2: Allocate the remaining height to results**

Give the result container the remaining vertical stretch. Keep the scope row in a horizontally scrollable viewport. Ensure the search field and scope row remain visible while the keyboard is open; the result list scrolls independently. Empty/no-query states must not consume the result list’s viewport.

- [ ] **Step 3: Build and exercise compact search**

Run: `cargo build -p notae-android`
Expected: Slint compiles on host. On the emulator, open Search, show the on-screen keyboard, scroll the scope chips horizontally, and verify results occupy the remaining height and scroll independently.

- [ ] **Step 4: Commit the task**

```sh
git add apps/notae-android/ui/search.slint apps/notae-android/ui/app.slint
git commit -m "fix(android): keep search usable with keyboard open"
```

### Task 5: Remove motion preference and all UI animation

**Files:**
- Modify: `apps/notae-android/ui/components.slint`
- Modify: `apps/notae-android/ui/library.slint`
- Modify: `apps/notae-android/ui/app.slint`
- Modify: `apps/notae-android/ui/setup.slint`
- Modify: `apps/notae-android/src/android/theme.rs`
- Modify: `apps/notae-android/android/src/main/java/org/notae/android/NotaeActivity.java`
- Modify: `docs/android-debugging.md`

**Interfaces:**
- Consume the product requirement “no animations”; preserve modal open/close behavior, palette behavior, and all non-motion Android theme behavior.
- Remove `Theme.reduced-motion`, `AppWindow.reduced_motion`, Settings bindings, `isNotaeMotionReduced()` Java/JNI bridge, animation-tick progress effects, and every Slint `animate` block. Use only static progress indicators driven by worker-reported progress.

- [ ] **Step 1: Remove reduced-motion UI/API references and animations**

Delete the `reduced-motion` theme property and all Rust/Slint bindings. Remove the Motion section from Settings. Remove all `animate` blocks and `animation-tick()` usage. Change modal open/dismiss and progress updates immediately; retain static pressed/focus affordances needed for input accessibility, with no animation or transition.

- [ ] **Step 2: Remove the Android motion JNI bridge**

Delete `isNotaeMotionReduced()` from `NotaeActivity.java` and its call from `src/android/theme.rs`. Do not alter system light/dark palette behavior or unrelated SAF JNI helpers.

- [ ] **Step 3: Run host and Android checks**

Run: `cargo build -p notae-android && cargo test -p notae-android -p notae-index && cargo clippy -p notae-android -p notae-index --all-targets -- -D warnings && cargo apk2 check -p notae-android --target aarch64-linux-android`
Expected: checks pass and Android compilation has no stale generated property references. On device, confirm no UI transition animates; progress changes only when worker events arrive.

- [ ] **Step 4: Update the Android verification runbook**

Add follow-up smoke checks to `docs/android-debugging.md`: status visibility while switching screens; pending-text meaning; Scan action on all configured screens; narrow Settings; keyboard-open Search; accessible Edit details; immediate/no-motion behavior. Mark emulator/tablet checks unverified if no device is available.

- [ ] **Step 5: Commit the task**

```sh
git add apps/notae-android/ui/components.slint apps/notae-android/ui/library.slint apps/notae-android/ui/app.slint apps/notae-android/ui/setup.slint apps/notae-android/src/android/theme.rs apps/notae-android/android/src/main/java/org/notae/android/NotaeActivity.java docs/android-debugging.md
git commit -m "fix(android): remove motion controls and animations"
```

## Final Verification

Run the repository-pinned Rust 1.98.1 quality gates from `docs/engineering-standards.md` after all five tasks are complete and committed:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings -A clippy::unwrap_used -A clippy::expect_used
cargo clippy --workspace --lib --bins -- -D warnings -D clippy::unwrap_used -D clippy::expect_used
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo deny check
cargo llvm-cov --workspace --summary-only --fail-under-lines 84
cargo run -p notae-index-cli -- --help
cargo apk2 check -p notae-android --target aarch64-linux-android
```

Build/install and visually exercise on the emulator/tablet when available. Report device checks separately from host verification; do not claim a visual check that was not performed. If any final gate discovers a fix, rerun the affected gate after the fix before treating verification as complete.
