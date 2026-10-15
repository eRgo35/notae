# Notae Android One UI Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the Notae Android app (`apps/notae-android`) from its wallpaper-tinted Material look to the Samsung One UI–style design approved on the canvas, and add the features that design shows: a section side panel (All, Recent, Needs details, Text pending, Composers), first-page thumbnails, a list-detail score pane, and a single search screen with scopes, lyrics snippets and Ctrl K.

**Architecture:** Slint stays the only UI layer; the single `ui/app.slint` is split into a token/controls file and one file per screen, with `AppWindow` in `app.slint` composing them. All new behavior that can run on Linux (sections, composer counts, recent list, thumbnail cache, search scope and snippets, screen navigation) lives in host-testable Rust modules with integration tests under `apps/notae-android/tests` and `crates/notae-index/tests`. Android-only pieces (thumbnail rendering through `PdfRenderer`, Slint glue in `android_main.rs`) are thin and verified on the emulator and tablet.

**Tech Stack:** Rust 2024 workspace, Slint 1.18 (`slint-build`), `android-activity` 0.6, `jni` 0.22, Java (`NotaeActivity`), SQLite FTS5 via `notae-index`, `cargo-apk2`.

**Spec:** Approved design canvas https://claude.ai/artifact/3bQ38pa1r5L3hVXF1M8N6Z (boards 1 · Library, 2 · Score details, 3 · Search (also Ctrl K), 4 · First run) plus the decisions below. The product spec `docs/superpowers/specs/2026-10-09-notae-design.md` is updated in Task 12 where these decisions change it.

## Progress

_Last updated: 2026-10-14. Branch `feat/one-ui-redesign`. Emulator/tablet visual checks were not performed; tablet installation remains deferred pending user go-ahead._

| Task | Status | Commits |
|---|---|---|
| 1 · One UI palette with system light/dark | ✅ Done, reviewed | `dc02b67` |
| 2 · One UI controls | ✅ Done, reviewed | `7e09e31` |
| 3 · Library sections and composer counts | ✅ Done, reviewed | `5800fb8` |
| 4 · Recent scores | ✅ Done, reviewed | `e52c0f2` |
| 5 · Search scopes and lyrics snippets | ✅ Done, reviewed | `a2509ab` |
| 6 · Search screen navigation | ✅ Done, reviewed | `2dffa6e` |
| 7 · First-page thumbnails | ✅ Done, reviewed | `57bcc65` |
| 8 · Library screen | ✅ Done, reviewed (device check deferred) | `1c79d4a`, `b3b9691` |
| 9 · Score details pane | ✅ Done, reviewed (device check deferred) | `acd269b`, `1513f44` |
| 10 · Search screen and Ctrl K | ✅ Done, reviewed (device check deferred) | `4632cec` |
| 11 · First run, settings and dialogs | ✅ Done, reviewed (device check deferred) | `8d3a470`, `4fe85be` |
| 12 · Spec, runbook and device verification | ✅ Docs/host verification reviewed (device check deferred) | `0b7842b` |

Deviations so far: Slint reserves `row`, so `PagePreview`/`ScoreCard` take `score:`; the details slot is an always-present element with `visible: details-open` (Slint rejects `@children` inside `if`); Settings keeps an interim back button until Task 11; thumbnails are decoded once and cached on the Android side (Slint's own image cache holds only ~5 thumbnails).

## Design decisions (2026-10-14 review with the user)

- Scope: style **and** the new features shown in the canvas.
- Desktop sidecar import stays as a feature; only its dialog is restyled.
- Colors: fixed One UI–style palette (blue accent, no teal), light/dark follows the Android system setting. The wallpaper-derived colors are removed.
- Font: the system font (`sans-serif`), which is Samsung's UI font on the Galaxy Tab. No bundled font.
- One search entry point: a search icon in the Library header and Ctrl K both open the Search screen. There is no always-visible search bar.
- Tapping a score opens its details pane; the PDF opens from **Open score**.

## Post-install findings and follow-up design (2026-10-14)

These findings came from manual review of the installed build. Record them for the next UI fix pass; this note does not claim the fixes are implemented.

- **Text-pending status:** A “Text pending” badge alone does not explain progress. Add an app-wide status surface, visible on Library, Search, and Settings, that explains the current scan/index phase, progress, pending-text count, and any failure or next action. Keep the status visible as screens change; do not present pending text as OCR actively running when no such operation exists.
- **Search layout:** The scope chips and result area must remain visible and usable at compact heights, including with the on-screen keyboard open. Chips may scroll horizontally; results must receive the remaining viewport height and scroll independently rather than being pushed below the screen or hidden.
- **Settings layout:** Fix the broken vertical spacing and make the screen fit narrow viewports. Keep only controls and status tied to capabilities Notae actually has:
  - **Library location:** show the selected folder’s display name and offer change/reconnect through the Storage Access Framework.
  - **Library maintenance:** provide an explicit scan action and show scan/index outcome, pending-text count, conflicts, and failures when available.
  - **Local storage and privacy:** explain that PDFs stay in place and thumbnails/index data are local. Do not invent storage controls or counts the app cannot report.
  - **About:** retain only app name/version and concise product information if useful.
  - Remove the generic “Motion” preference and reduce-motion plumbing: the product decision is **no animations**, not an optional reduced-motion mode. Remove decorative or irrelevant settings copy.
- **Persistent actions:** Keep the Scan icon visible in the app chrome on Library, Search, and Settings; disable it only when scanning cannot be started. In score details, show **Edit details** as an icon-only action with an accessible label (“Edit details”).
- **Motion:** No animated transitions or state effects anywhere in the redesigned UI.

## Global Constraints

- No new crate dependencies (user rule). Java code may use only the Android SDK.
- Fully local: no network at runtime.
- Notae never copies, renames or moves PDFs. Thumbnails are written only to the app's private `internal_data_path()/notae/thumbnails`.
- `min_sdk_version = 23`, `target_sdk_version = 34` stay as they are.
- `state.rs` and `ui.rs` keep `#![forbid(unsafe_code)]`; new host modules also declare it.
- Touch targets at least 48×48 px; text contrast at least 4.5:1 against its background in both themes (the palette below meets this).
- Palette (light / dark): background `#F3F3F5` / `#000000`; card `#FFFFFF` / `#17171A`; field `#EBEBEF` / `#26262B`; foreground `#111114` / `#F2F2F4`; muted `#6E6E73` / `#A1A1A8`; divider `#E6E6EA` / `#2A2A2E`; accent `#2F6FEB` / `#5B9BFF`; on-accent `#FFFFFF` / `#06142E`; accent-soft `#E6EEFE` / `#1C2C4A`; on-accent-soft `#1A4FB8` / `#B9D2FF`; warn-soft `#FFF0E0` / `#3A2610`; on-warn-soft `#A14D00` / `#FFB870`; error `#C62828` / `#FF8A80`; paper `#FFFFFF` / `#E4E4E8`; paper-edge `#E3E3E8` / `#2A2A2E`.
- Shapes: cards 26 px radius, list/nav rows 18 px, pill buttons height 52 px, chips height 40 px, search field height 52 px.
- Copy: "All scores", "Recent", "Needs details", "Text pending", "Composers", "Open score", "Edit details", "Text pending" badge, "Press Ctrl K to search from anywhere".
- Out of scope (needs a sidecar/schema change, separate plan): page numbers on lyrics matches. Snippets show the matching text without a page number.

## Review Focus

1. Switching the system between light and dark while Notae is open → Notae's colors switch too without a restart (Task 1 emulator check; fallback reads the mode at launch).
2. A score with no thumbnail (first launch before a scan, an image-less or encrypted PDF, a failed render, a corrupt cache file) → the card shows the document placeholder and nothing crashes; a failed render leaves no partial file in the cache (Task 7 test `failed_render_leaves_no_partial_file`).
3. A damaged `recent.txt` (garbage lines, duplicates, deleted scores) → ignored lines are dropped and Recent shows only scores that still exist (Task 4 tests `from_text_keeps_only_valid_unique_keys`, `recent_section_skips_unknown_keys`).
4. A search made only of punctuation, or a scope with no matches → an empty "No scores found" state, never an error banner (Task 5 test `punctuation_only_query_in_any_scope_returns_nothing`).
5. Split screen or a narrow window (< 840 px) and the on-screen keyboard → the side panel becomes a chip row and the details pane takes the full width with a back button (Task 8/9 emulator checks).

---

## File Structure

| Path | Responsibility | Change |
|---|---|---|
| `apps/notae-android/ui/components.slint` | `Theme` tokens and shared controls (`ActionInput`, `Glyph`, `PillButton`, `IconButton`, `SearchField`, `Card`, `NavRow`, `Chip`, `StatusBadge`) | Renamed from `material.slint` (Task 2) |
| `apps/notae-android/ui/library.slint` | `LibraryRow`/`SectionItem` structs, side panel, score grid, operation banner, empty state, `LibraryScreen` | Create (Task 8) |
| `apps/notae-android/ui/details.slint` | `ScoreList`, `ScoreDetails` pane | Create (Task 9) |
| `apps/notae-android/ui/search.slint` | `SearchScreen` | Create (Task 10) |
| `apps/notae-android/ui/setup.slint` | `WelcomeScreen`, `SettingsScreen`, `DialogSheet` contents | Create (Task 11) |
| `apps/notae-android/ui/app.slint` | `AppWindow` properties/callbacks, screen switching, Ctrl K, modal sheets | Modify throughout |
| `apps/notae-android/ui/material-icons/*.svg` | Icons; adds `schedule.svg`, `subject.svg` | Modify (Task 2) |
| `apps/notae-android/build.rs` | Slint style `material` (follows color scheme) | Modify (Task 1) |
| `apps/notae-android/src/android/theme.rs` | Reduced-motion preference only (wallpaper colors removed) | Modify (Task 1) |
| `apps/notae-android/android/src/main/java/org/notae/android/NotaeActivity.java` | Remove `getNotaeColor`; add `renderNotaeThumbnail` | Modify (Tasks 1, 7) |
| `apps/notae-android/src/ui/browse.rs` | `LibrarySection`, `ComposerCount`, `SectionView`, filtering | Create (Task 3) |
| `apps/notae-android/src/ui/search.rs` | `SearchRow` gains `text_pending`, origins, `text_snippet`; `split_snippet`; Android `library_row` | Modify (Tasks 3, 5, 8) |
| `apps/notae-android/src/recent.rs` | `RecentScores` persisted list | Create (Task 4) |
| `apps/notae-android/src/state.rs` | `Screen::Search`, `OpenSearch`/`CloseSearch`/`UpdateSearchScope`, `SearchState.scope` | Modify (Task 6) |
| `apps/notae-android/src/ui.rs` | `UiModel` section/selection/recent/thumbnail state | Modify (Tasks 3, 4, 7) |
| `crates/notae-index/src/search.rs`, `store.rs`, `lib.rs` | `SearchScope`, `search_in`, `SearchHit::text_snippet` | Modify (Task 5) |
| `apps/notae-android/src/ui/library.rs`, `src/operations/rescan.rs` | Scope-aware projection; thumbnail generation and pruning | Modify (Tasks 5, 7) |
| `apps/notae-android/src/thumbnails.rs` | `ThumbnailCache` | Create (Task 7) |
| `apps/notae-android/src/storage.rs`, `src/android/saf.rs` | `LibraryCatalog::render_thumbnail` | Modify (Task 7) |
| `apps/notae-android/src/android_main.rs` | Wiring for every new property/callback | Modify (Tasks 4, 7–11) |
| `apps/notae-android/tests/browse.rs`, `recent.rs`, `thumbnails.rs`; `tests/search_state.rs`, `tests/operation_state.rs`; `crates/notae-index/tests/store_and_search.rs` | Tests | Create/Modify |
| `docs/superpowers/specs/2026-10-09-notae-design.md`, `docs/android-debugging.md`, `docs/superpowers/IMPLEMENTATION-STATE.md` | Spec and runbook updates | Modify (Task 12) |

## Commands used throughout

```sh
# Host build (also compiles every .slint file through build.rs)
cargo build -p notae-android
# Host tests
cargo test -p notae-android
cargo test -p notae-index
# Lint (workspace rule)
cargo clippy -p notae-android -p notae-index --all-targets -- -D warnings
# Emulator APK (see docs/android-debugging.md for the environment variables)
ANDROID_NDK_ROOT=/home/mike/Android/Sdk/ndk/android-ndk-r26d \
JAVA_HOME=/usr/lib/jvm/java-25-openjdk \
JDK_JAVAC_OPTIONS="-source 17 -target 17" JAVA_SOURCE_VERSION=17 JAVA_TARGET_VERSION=17 \
cargo apk2 build --release -p notae-android --target x86_64-linux-android
adb -s emulator-5554 install -r target/release/apk/notae-android.apk
adb -s emulator-5554 shell am start -n org.notae.android/.NotaeActivity
```

Note: Slint UI types (`AppWindow`, `LibraryRow`) are generated only for Android (`src/lib.rs` includes them under `cfg(target_os = "android")`). Host `cargo build` still runs `build.rs`, so a broken `.slint` file fails the host build. Android-only Rust (`android_main.rs`, `src/android/*`) is checked with `cargo apk2 check -p notae-android --target aarch64-linux-android`.

---

### Task 1: One UI palette with system light/dark

**Files:**
- Modify: `apps/notae-android/ui/material.slint:1-18` (Theme global)
- Modify: `apps/notae-android/build.rs`
- Modify: `apps/notae-android/src/android/theme.rs`
- Modify: `apps/notae-android/android/src/main/java/org/notae/android/NotaeActivity.java`

**Interfaces:**
- Produces: `Theme` tokens `dark, background, card, field, foreground, muted, divider, accent, on-accent, accent-soft, on-accent-soft, warn-soft, on-warn-soft, error, paper, paper-edge, reduced-motion, modal-open`. Legacy names `surface, container, high, primary, on-primary, primary-container, on-container, outline, success` stay as aliases until Task 11 removes them.

- [x] **Step 1: Replace the `Theme` global**

Replace lines 1–18 of `ui/material.slint` with:

```slint
// One UI–style palette. `dark` follows the Android system setting through Slint's Palette.
import { Palette } from "std-widgets.slint";

export global Theme {
    in-out property <bool> dark: Palette.color-scheme == ColorScheme.dark;
    out property <color> background: root.dark ? #000000 : #F3F3F5;
    out property <color> card: root.dark ? #17171A : #FFFFFF;
    out property <color> field: root.dark ? #26262B : #EBEBEF;
    out property <color> foreground: root.dark ? #F2F2F4 : #111114;
    out property <color> muted: root.dark ? #A1A1A8 : #6E6E73;
    out property <color> divider: root.dark ? #2A2A2E : #E6E6EA;
    out property <color> accent: root.dark ? #5B9BFF : #2F6FEB;
    out property <color> on-accent: root.dark ? #06142E : #FFFFFF;
    out property <color> accent-soft: root.dark ? #1C2C4A : #E6EEFE;
    out property <color> on-accent-soft: root.dark ? #B9D2FF : #1A4FB8;
    out property <color> warn-soft: root.dark ? #3A2610 : #FFF0E0;
    out property <color> on-warn-soft: root.dark ? #FFB870 : #A14D00;
    out property <color> error: root.dark ? #FF8A80 : #C62828;
    out property <color> paper: root.dark ? #E4E4E8 : #FFFFFF;
    out property <color> paper-edge: root.dark ? #2A2A2E : #E3E3E8;
    // Legacy aliases, removed in Task 11 once every screen uses the names above.
    out property <color> surface: root.card;
    out property <color> container: root.card;
    out property <color> high: root.field;
    out property <color> primary: root.accent;
    out property <color> on-primary: root.on-accent;
    out property <color> primary-container: root.accent-soft;
    out property <color> on-container: root.on-accent-soft;
    out property <color> outline: root.dark ? #3A3A40 : #C7C7CC;
    out property <color> success: root.dark ? #8FD19E : #2E7D32;
    in-out property <bool> reduced-motion: false;
    in-out property <bool> modal-open: false;
}
```

- [x] **Step 2: Let Slint's own widgets follow the color scheme**

In `build.rs`, change the style so `ScrollView`, `ListView` and `CheckBox` switch with the system:

```rust
    let config = slint_build::CompilerConfiguration::new()
        .with_style("material".into());
```

- [x] **Step 3: Remove wallpaper colors from Rust**

Replace `src/android/theme.rs` with:

```rust
//! System motion preference. Colors come from the fixed One UI palette in `components.slint`.
use android_activity::AndroidApp;
use jni::{jni_sig, jni_str};

use crate::AppWindow;

pub fn configure(app: &AndroidApp, window: &AppWindow) {
    let result = super::saf::with_env(app, |env| {
        let activity = super::saf::activity_as_jobject(env, app.activity_as_ptr())?;
        env.call_method(&activity, jni_str!("isNotaeMotionReduced"), jni_sig!("()Z"), &[])?
            .z()
    });
    match result {
        Ok(reduced) => window.set_reduced_motion(reduced),
        Err(error) => eprintln!("notae: cannot read Android motion preference: {error}"),
    }
}
```

- [x] **Step 4: Remove the Java color bridge**

In `NotaeActivity.java` delete the `getNotaeColor` method, its comment, and the `mix` helper. Remove the now-unused `import android.os.Build;`.

- [x] **Step 5: Build on the host**

Run: `cargo build -p notae-android`
Expected: builds. A Slint error naming `Palette` or `ColorScheme` means the import line is wrong; fix it before going on.

- [x] **Step 6: Check the Android build**

Run: `cargo apk2 check -p notae-android --target aarch64-linux-android` (with the environment from "Commands used throughout").
Expected: no errors and no unused-import warnings in `theme.rs`.

- [ ] *(deferred to device check)* **Step 7: Emulator check — Review Focus 1**

Build and install the x86_64 APK, launch it, then run:

```sh
adb -s emulator-5554 shell cmd uimode night yes
adb -s emulator-5554 shell cmd uimode night no
```

Expected: the background goes black and back to light grey while the app stays open.

If it does **not** switch (Slint's Android backend did not update `Palette.color-scheme`), apply this fallback, which reads the mode at launch:

Add to `NotaeActivity.java`:

```java
    public boolean isNotaeNightMode() {
        int mode = getResources().getConfiguration().uiMode
            & android.content.res.Configuration.UI_MODE_NIGHT_MASK;
        return mode == android.content.res.Configuration.UI_MODE_NIGHT_YES;
    }
```

And in `theme.rs` `configure`, after the motion preference:

```rust
    let night = super::saf::with_env(app, |env| {
        let activity = super::saf::activity_as_jobject(env, app.activity_as_ptr())?;
        env.call_method(&activity, jni_str!("isNotaeNightMode"), jni_sig!("()Z"), &[])?.z()
    });
    if let Ok(night) = night {
        window.global::<crate::Theme>().set_dark(night);
    }
```

(add `use slint::ComponentHandle;`). Record in `docs/android-debugging.md` (Task 12) that the theme then switches on the next launch.

- [x] **Step 8: Commit**

```bash
git add apps/notae-android/ui/material.slint apps/notae-android/build.rs apps/notae-android/src/android/theme.rs apps/notae-android/android/src/main/java/org/notae/android/NotaeActivity.java
git commit -m "feat(android): adopt One UI palette with system light/dark"
```

---

### Task 2: One UI controls

**Files:**
- Rename: `apps/notae-android/ui/material.slint` → `apps/notae-android/ui/components.slint`
- Modify: `apps/notae-android/ui/app.slint` (imports and renamed components)
- Create: `apps/notae-android/ui/material-icons/schedule.svg`, `apps/notae-android/ui/material-icons/subject.svg`

**Interfaces:**
- Consumes: `Theme` from Task 1.
- Produces (all exported from `components.slint`): `ActionInput`, `Glyph { kind, ink }` with kinds `search folder library settings scan edit arrow-back close document chevron-right check warning schedule subject`; `PillButton { text, icon, primary, tonal, enabled, in-dialog; callback clicked() }`; `IconButton { label, icon, selected, enabled, in-dialog; callback clicked() }`; `SearchField { text, placeholder, enabled, in-dialog; callback edited(string) }` (always a search field) and `TextField { text, placeholder, enabled, in-dialog; callback edited(string) }`; `Card`; `NavRow { label, kind, count, selected; callback clicked() }`; `Chip { label, selected; callback clicked() }`; `StatusBadge { text }`.

- [x] **Step 1: Rename the file**

```bash
git mv apps/notae-android/ui/material.slint apps/notae-android/ui/components.slint
```

- [x] **Step 2: Add two icons**

`ui/material-icons/schedule.svg`:

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm0 18a8 8 0 1 1 0-16 8 8 0 0 1 0 16Zm.5-13H11v6l5.2 3.2.8-1.3-4.5-2.7Z"/></svg>
```

`ui/material-icons/subject.svg`:

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4 6h16v2H4zm0 5h16v2H4zm0 5h10v2H4z"/></svg>
```

- [x] **Step 3: Export `ActionInput` and extend `Glyph`**

In `components.slint` change `component ActionInput inherits TouchArea {` to `export component ActionInput inherits TouchArea {`. In `Glyph`, add `in property <color> ink: Theme.muted;`, change `colorize: Theme.muted;` to `colorize: root.ink;`, and add these two branches before the final fallback:

```slint
        : root.kind == "schedule" ? @image-url("material-icons/schedule.svg")
        : root.kind == "subject" ? @image-url("material-icons/subject.svg")
```

- [x] **Step 4: Restyle `MaterialButton` as `PillButton`**

Rename `export component MaterialButton` to `export component PillButton` and change only these properties (keep focus, accessibility and `ActionInput` as they are):

```slint
    private property <color> ink: !root.enabled ? Theme.muted
        : root.primary ? Theme.on-accent : Theme.foreground;

    min-width: max(52px, content.min-width);
    min-height: 52px;
    preferred-height: 52px;
    border-radius: 26px;
    background: !root.enabled ? Theme.field
        : root.primary ? Theme.accent
        : root.tonal ? Theme.field : transparent;
    border-width: 0px;
```

and in `content := HorizontalLayout` use `padding-left: 28px; padding-right: 28px; padding-top: 14px; padding-bottom: 14px;`, and in its `Text` use `font-size: 16px; font-weight: 600;`. Change the focus ring's `border-color` to `root.primary ? Theme.on-accent : Theme.accent`.

- [x] **Step 5: Restyle `IconButton`**

```slint
    private property <color> ink: root.selected ? Theme.on-accent-soft : Theme.foreground;
    background: root.selected && root.enabled ? Theme.accent-soft : transparent;
    border-color: Theme.accent;
```

- [x] **Step 6: Split `MaterialField` into `SearchField` and `TextField`**

Rename `MaterialField` to `SearchField`, delete its `in property <bool> search` and replace every `root.search` condition with the search branch. Then set:

```slint
    min-height: 52px;
    preferred-height: 52px;
    border-radius: 26px;
    background: Theme.field;
    border-width: editor.has-focus ? 2px : 0px;
    border-color: Theme.accent;
```

and in the inner `TextInput` use `color: Theme.foreground; selection-background-color: Theme.accent; selection-foreground-color: Theme.on-accent;`.

Copy the whole component as `TextField`, delete the search glyph and the clear button, set `viewport.x` to `16px`, its width to `max(1px, parent.width - 32px)`, and `border-radius: 16px;`.

- [x] **Step 7: Add the new shared components**

Append to `components.slint`:

```slint
export component Card inherits Rectangle {
    background: Theme.card;
    border-radius: 26px;
}

export component StatusBadge inherits Rectangle {
    in property <string> text;
    height: 28px;
    width: label.preferred-width + 24px;
    border-radius: 14px;
    background: Theme.warn-soft;
    label := Text {
        text: root.text;
        color: Theme.on-warn-soft;
        font-size: 13px;
        font-weight: 600;
        vertical-alignment: center;
        horizontal-alignment: center;
    }
}

export component NavRow inherits Rectangle {
    in property <string> label;
    in property <string> kind;
    in property <int> count: -1;
    in property <bool> selected;
    callback clicked();
    height: 52px;
    border-radius: 18px;
    background: root.selected ? Theme.accent-soft
        : input.pressed ? Theme.field : transparent;
    border-width: input.has-focus ? 2px : 0px;
    border-color: Theme.accent;
    accessible-role: button;
    accessible-label: root.label;
    accessible-description: root.selected ? "Current section" : "";
    accessible-action-default => { if !Theme.modal-open { root.clicked(); } }
    forward-focus: input;
    HorizontalLayout {
        padding-left: 14px;
        padding-right: 14px;
        spacing: 14px;
        if root.kind != "" : VerticalLayout {
            alignment: center;
            Glyph {
                kind: root.kind;
                width: 22px;
                height: 22px;
                ink: root.selected ? Theme.on-accent-soft : Theme.foreground;
            }
        }
        Text {
            text: root.label;
            horizontal-stretch: 1;
            vertical-alignment: center;
            overflow: elide;
            font-size: 16px;
            font-weight: root.selected ? 600 : 400;
            color: root.selected ? Theme.on-accent-soft : Theme.foreground;
        }
        if root.count >= 0 : Text {
            text: root.count;
            vertical-alignment: center;
            font-size: 14px;
            color: root.selected ? Theme.on-accent-soft : Theme.muted;
        }
    }
    input := ActionInput {
        enabled: !Theme.modal-open;
        clicked => { self.focus(); root.clicked(); }
    }
}

export component Chip inherits Rectangle {
    in property <string> label;
    in property <bool> selected;
    callback clicked();
    height: 40px;
    width: text.preferred-width + 36px;
    border-radius: 20px;
    background: root.selected ? Theme.accent : Theme.card;
    border-width: input.has-focus ? 2px : 0px;
    border-color: root.selected ? Theme.on-accent : Theme.accent;
    accessible-role: button;
    accessible-label: root.label;
    accessible-description: root.selected ? "Selected" : "";
    accessible-action-default => { if !Theme.modal-open { root.clicked(); } }
    forward-focus: input;
    text := Text {
        text: root.label;
        font-size: 15px;
        font-weight: root.selected ? 600 : 500;
        color: root.selected ? Theme.on-accent : Theme.foreground;
        vertical-alignment: center;
        horizontal-alignment: center;
    }
    input := ActionInput {
        enabled: !Theme.modal-open;
        clicked => { self.focus(); root.clicked(); }
    }
}
```

- [x] **Step 8: Update `app.slint` to the new names**

In `app.slint`, change the import to:

```slint
import { Theme, PillButton, SearchField, TextField, IconButton, Glyph } from "components.slint";
```

Replace every `MaterialButton` with `PillButton`. Replace the library search `MaterialField { … search: true; … }` with `SearchField { … }` (drop `search: true;`). Replace the four metadata-editor `MaterialField { in-dialog: true; … }` with `TextField { in-dialog: true; … }`.

- [x] **Step 9: Build and lint**

Run: `cargo build -p notae-android && cargo clippy -p notae-android --all-targets -- -D warnings`
Expected: both succeed.

- [x] **Step 10: Commit**

```bash
git add -A apps/notae-android/ui
git commit -m "feat(android): add One UI controls"
```

---

### Task 3: Library sections and composer counts (host logic)

**Files:**
- Modify: `apps/notae-android/src/ui/search.rs` (new `SearchRow` fields)
- Create: `apps/notae-android/src/ui/browse.rs`
- Modify: `apps/notae-android/src/ui.rs` (`pub mod browse;`)
- Modify: `apps/notae-android/tests/operation_state.rs:261,458` (struct literals)
- Test: `apps/notae-android/tests/browse.rs`

**Interfaces:**
- Produces: `SearchRow { …, text_pending: bool, title_origin: String, composer_origin: String, text_snippet: Option<String> }` (`text_snippet` is filled in Task 5; it is `None` until then).
- Produces in `notae_android::ui::browse`:
  - `enum LibrarySection { All, Recent, NeedsDetails, TextPending, Composer(String) }` with `key(&self) -> String`, `from_key(&str) -> Self`, `title(&self) -> String`, `Default = All`.
  - `struct ComposerCount { name: String, count: usize }`; `fn composers(rows: &[SearchRow]) -> Vec<ComposerCount>`.
  - `fn section_rows(rows: &[SearchRow], section: &LibrarySection, recent: &[String]) -> Vec<SearchRow>`.
  - `struct SectionView { key: String, label: String, count: usize, kind: &'static str }`; `fn sections(rows: &[SearchRow], recent: &[String]) -> Vec<SectionView>` (the four fixed sections).

- [x] **Step 1: Write the failing tests**

Create `tests/browse.rs`:

```rust
use notae_android::ui::browse::{
    ComposerCount, LibrarySection, composers, section_rows, sections,
};
use notae_android::ui::search::SearchRow;

fn row(key: char, id: Option<&str>, title: &str, composer: Option<&str>, pending: bool) -> SearchRow {
    SearchRow {
        id: id.map(str::to_owned),
        title: title.to_owned(),
        composer_or_artist: composer.map(str::to_owned),
        tags: Vec::new(),
        status: if pending { "OCR pending" } else { "Ready" }.to_owned(),
        relative_path: format!("{title}.pdf"),
        note_key: key.to_string().repeat(64),
        text_pending: pending,
        title_origin: "From filename".to_owned(),
        composer_origin: String::new(),
        text_snippet: None,
    }
}

fn library() -> Vec<SearchRow> {
    vec![
        row('a', Some("12"), "Clair de lune", Some("Claude Debussy"), false),
        row('b', Some("14"), "Cantique", Some("Gabriel Fauré"), true),
        row('c', None, "Scarborough Fair", Some("traditional"), false),
        row('d', Some("17"), "Pavane", Some("gabriel fauré"), false),
        row('e', Some("20"), "Untitled sketch", None, false),
    ]
}

#[test]
fn section_keys_round_trip() {
    for section in [
        LibrarySection::All,
        LibrarySection::Recent,
        LibrarySection::NeedsDetails,
        LibrarySection::TextPending,
        LibrarySection::Composer("Gabriel Fauré".to_owned()),
    ] {
        assert_eq!(LibrarySection::from_key(&section.key()), section);
    }
}

#[test]
fn unknown_or_empty_keys_fall_back_to_all() {
    assert_eq!(LibrarySection::from_key("nonsense"), LibrarySection::All);
    assert_eq!(LibrarySection::from_key("composer:"), LibrarySection::All);
}

#[test]
fn composers_merge_case_variants_and_sort_by_name() {
    assert_eq!(
        composers(&library()),
        vec![
            ComposerCount { name: "Claude Debussy".to_owned(), count: 1 },
            ComposerCount { name: "Gabriel Fauré".to_owned(), count: 2 },
            ComposerCount { name: "traditional".to_owned(), count: 1 },
        ]
    );
}

#[test]
fn needs_details_means_missing_number_or_composer() {
    let titles: Vec<String> = section_rows(&library(), &LibrarySection::NeedsDetails, &[])
        .into_iter()
        .map(|row| row.title)
        .collect();
    assert_eq!(titles, ["Scarborough Fair", "Untitled sketch"]);
}

#[test]
fn text_pending_and_composer_sections_filter_rows() {
    let pending = section_rows(&library(), &LibrarySection::TextPending, &[]);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].title, "Cantique");

    let faure = section_rows(
        &library(),
        &LibrarySection::Composer("GABRIEL FAURÉ".to_owned()),
        &[],
    );
    assert_eq!(faure.len(), 2);
}

#[test]
fn recent_section_follows_recent_order_and_skips_unknown_keys() {
    let recent = vec!["d".repeat(64), "z".repeat(64), "a".repeat(64)];
    let titles: Vec<String> = section_rows(&library(), &LibrarySection::Recent, &recent)
        .into_iter()
        .map(|row| row.title)
        .collect();
    assert_eq!(titles, ["Pavane", "Clair de lune"]);
}

#[test]
fn fixed_sections_report_counts() {
    let recent = vec!["a".repeat(64)];
    let views = sections(&library(), &recent);
    let summary: Vec<(String, usize)> =
        views.into_iter().map(|view| (view.key, view.count)).collect();
    assert_eq!(
        summary,
        [
            ("all".to_owned(), 5),
            ("recent".to_owned(), 1),
            ("needs-details".to_owned(), 2),
            ("text-pending".to_owned(), 1),
        ]
    );
}
```

- [x] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p notae-android --test browse`
Expected: FAIL to compile — `notae_android::ui::browse` does not exist and `SearchRow` has no `text_pending`.

- [x] **Step 3: Extend `SearchRow`**

In `src/ui/search.rs` add the fields after `note_key`:

```rust
    /// Whether searchable text is still missing (OCR pending or incomplete).
    pub text_pending: bool,
    /// Where the title came from, as a short label ("From filename", "From desktop", "Edited").
    pub title_origin: String,
    /// Where the composer came from; empty when there is no composer.
    pub composer_origin: String,
    /// FTS5 snippet of matching lyrics/page text, with `\u{2}`/`\u{3}` around the match.
    pub text_snippet: Option<String>,
```

Update `from_note` (add `use notae_domain::{MetadataField, MetadataOrigin};`):

```rust
    pub(crate) fn from_note(note: IndexedNote) -> Self {
        let metadata = note.metadata();
        let composer = metadata.composer_or_artist().map(str::to_owned);
        Self {
            id: note.id().map(|id| id.to_string()),
            title: note.title().to_owned(),
            composer_origin: if composer.is_some() {
                origin_label(metadata.origin_of(MetadataField::ComposerOrArtist)).to_owned()
            } else {
                String::new()
            },
            composer_or_artist: composer,
            tags: metadata.tags().iter().cloned().collect(),
            status: status_label(note.status()).to_owned(),
            text_pending: matches!(
                note.status(),
                IndexStatus::OcrPending | IndexStatus::OcrIncomplete
            ),
            title_origin: origin_label(metadata.origin_of(MetadataField::Title)).to_owned(),
            note_key: note.fingerprint().sha256().to_string(),
            relative_path: note.path().as_str().to_owned(),
            text_snippet: None,
        }
    }
```

and add:

```rust
fn origin_label(origin: MetadataOrigin) -> &'static str {
    match origin {
        MetadataOrigin::FilenameDerived => "From filename",
        MetadataOrigin::PcImported => "From desktop",
        MetadataOrigin::Manual => "Edited",
        _ => "",
    }
}
```

In the Android `From<SearchRow> for crate::LibraryRow` impl nothing changes yet (the new fields are unused until Task 8).

- [x] **Step 4: Update the two test literals**

In `tests/operation_state.rs` at both `SearchRow { … }` literals (lines ~261 and ~458) add:

```rust
        text_pending: false,
        title_origin: String::new(),
        composer_origin: String::new(),
        text_snippet: None,
```

- [x] **Step 5: Write `browse.rs`**

Create `src/ui/browse.rs`:

```rust
//! Library sections, composer navigation and section filtering over projected rows.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use crate::ui::search::SearchRow;

/// One entry of the library side panel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum LibrarySection {
    /// Every indexed score.
    #[default]
    All,
    /// Scores opened recently, newest first.
    Recent,
    /// Scores missing a number or a composer.
    NeedsDetails,
    /// Scores whose searchable text is not available yet.
    TextPending,
    /// Scores by one composer, matched case-insensitively.
    Composer(String),
}

impl LibrarySection {
    /// Stable key passed through the Slint layer.
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Self::All => "all".to_owned(),
            Self::Recent => "recent".to_owned(),
            Self::NeedsDetails => "needs-details".to_owned(),
            Self::TextPending => "text-pending".to_owned(),
            Self::Composer(name) => format!("composer:{name}"),
        }
    }

    /// Parses a key; anything unknown is the All section.
    #[must_use]
    pub fn from_key(key: &str) -> Self {
        match key {
            "recent" => Self::Recent,
            "needs-details" => Self::NeedsDetails,
            "text-pending" => Self::TextPending,
            _ => key
                .strip_prefix("composer:")
                .filter(|name| !name.trim().is_empty())
                .map_or(Self::All, |name| Self::Composer(name.to_owned())),
        }
    }

    /// Large title shown above the score grid.
    #[must_use]
    pub fn title(&self) -> String {
        match self {
            Self::All => "All scores".to_owned(),
            Self::Recent => "Recent".to_owned(),
            Self::NeedsDetails => "Needs details".to_owned(),
            Self::TextPending => "Text pending".to_owned(),
            Self::Composer(name) => name.clone(),
        }
    }
}

/// A composer and how many scores carry that name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerCount {
    /// Spelling from the first score in library order.
    pub name: String,
    /// Number of scores.
    pub count: usize,
}

/// A fixed side-panel entry with its count and icon kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionView {
    /// Value of [`LibrarySection::key`].
    pub key: String,
    /// Visible label.
    pub label: String,
    /// Number of scores in the section.
    pub count: usize,
    /// `Glyph` kind in `components.slint`.
    pub kind: &'static str,
}

/// Composers sorted case-insensitively; case variants of one name are merged.
#[must_use]
pub fn composers(rows: &[SearchRow]) -> Vec<ComposerCount> {
    let mut by_name: BTreeMap<String, ComposerCount> = BTreeMap::new();
    for name in rows.iter().filter_map(|row| row.composer_or_artist.as_deref()) {
        by_name
            .entry(name.to_lowercase())
            .or_insert_with(|| ComposerCount { name: name.to_owned(), count: 0 })
            .count += 1;
    }
    by_name.into_values().collect()
}

/// Rows that belong to `section`; `recent` holds content keys, newest first.
#[must_use]
pub fn section_rows(rows: &[SearchRow], section: &LibrarySection, recent: &[String]) -> Vec<SearchRow> {
    match section {
        LibrarySection::All => rows.to_vec(),
        LibrarySection::Recent => recent
            .iter()
            .filter_map(|key| rows.iter().find(|row| &row.note_key == key))
            .cloned()
            .collect(),
        LibrarySection::NeedsDetails => rows
            .iter()
            .filter(|row| row.id.is_none() || row.composer_or_artist.is_none())
            .cloned()
            .collect(),
        LibrarySection::TextPending => rows.iter().filter(|row| row.text_pending).cloned().collect(),
        LibrarySection::Composer(name) => {
            let name = name.to_lowercase();
            rows.iter()
                .filter(|row| {
                    row.composer_or_artist
                        .as_deref()
                        .is_some_and(|composer| composer.to_lowercase() == name)
                })
                .cloned()
                .collect()
        }
    }
}

/// The four fixed side-panel sections with their counts.
#[must_use]
pub fn sections(rows: &[SearchRow], recent: &[String]) -> Vec<SectionView> {
    [
        (LibrarySection::All, "library"),
        (LibrarySection::Recent, "schedule"),
        (LibrarySection::NeedsDetails, "edit"),
        (LibrarySection::TextPending, "subject"),
    ]
    .into_iter()
    .map(|(section, kind)| SectionView {
        key: section.key(),
        label: section.title(),
        count: section_rows(rows, &section, recent).len(),
        kind,
    })
    .collect()
}
```

Add `pub mod browse;` next to `pub mod library;` in `src/ui.rs`.

- [x] **Step 6: Run the tests**

Run: `cargo test -p notae-android`
Expected: all tests pass, including the seven in `tests/browse.rs`.

- [x] **Step 7: Lint and commit**

```bash
cargo clippy -p notae-android --all-targets -- -D warnings
git add apps/notae-android/src/ui/browse.rs apps/notae-android/src/ui/search.rs apps/notae-android/src/ui.rs apps/notae-android/tests/browse.rs apps/notae-android/tests/operation_state.rs
git commit -m "feat(android): add library sections and composer counts"
```

---

### Task 4: Recent scores

**Files:**
- Create: `apps/notae-android/src/recent.rs`
- Modify: `apps/notae-android/src/lib.rs` (`pub mod recent;`)
- Modify: `apps/notae-android/src/ui.rs` (UiModel fields and methods)
- Test: `apps/notae-android/tests/recent.rs`

**Interfaces:**
- Produces `notae_android::recent::RecentScores` with `RECENT_LIMIT = 30`, `from_text(&str) -> Self`, `to_text(&self) -> String`, `load(&Path) -> Self`, `save(&self, &Path) -> io::Result<()>`, `record(&mut self, key: &str)`, `keys(&self) -> &[String]`.
- Produces on `UiModel`: `set_recent_path(PathBuf)`, `record_opened(&mut self, note_key: &str) -> io::Result<()>`, `recent(&self) -> &[String]`, `section(&self) -> &LibrarySection`, `select_section(&mut self, LibrarySection)`, `visible_rows(&self) -> Vec<SearchRow>`, `select_note(&mut self, Option<String>)`, `selected_row(&self) -> Option<SearchRow>`.

- [x] **Step 1: Write the failing tests**

Create `tests/recent.rs`:

```rust
use notae_android::recent::{RECENT_LIMIT, RecentScores};
use notae_android::ui::UiModel;
use notae_android::ui::browse::LibrarySection;
use notae_android::ui::search::SearchRow;

fn key(c: char) -> String {
    c.to_string().repeat(64)
}

fn row(c: char, title: &str) -> SearchRow {
    SearchRow {
        id: None,
        title: title.to_owned(),
        composer_or_artist: None,
        tags: Vec::new(),
        status: "Ready".to_owned(),
        relative_path: format!("{title}.pdf"),
        note_key: key(c),
        text_pending: false,
        title_origin: String::new(),
        composer_origin: String::new(),
        text_snippet: None,
    }
}

#[test]
fn record_moves_key_to_front_without_duplicates() {
    let mut recent = RecentScores::default();
    recent.record(&key('a'));
    recent.record(&key('b'));
    recent.record(&key('a'));
    assert_eq!(recent.keys(), [key('a'), key('b')]);
}

#[test]
fn record_keeps_at_most_the_limit() {
    let mut recent = RecentScores::default();
    for index in 0..(RECENT_LIMIT + 5) {
        recent.record(&format!("{index:064x}"));
    }
    assert_eq!(recent.keys().len(), RECENT_LIMIT);
    assert_eq!(recent.keys()[0], format!("{:064x}", RECENT_LIMIT + 4));
}

#[test]
fn from_text_keeps_only_valid_unique_keys() {
    let text = format!("{}\nnot-a-key\n\n{}\n{}\nABC\n", key('a'), key('b'), key('a'));
    assert_eq!(RecentScores::from_text(&text).keys(), [key('a'), key('b')]);
}

#[test]
fn save_then_load_round_trips_and_missing_file_is_empty() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("recent.txt");
    assert!(RecentScores::load(&path).keys().is_empty());

    let mut recent = RecentScores::default();
    recent.record(&key('c'));
    recent.save(&path).expect("save recent");
    assert_eq!(RecentScores::load(&path).keys(), [key('c')]);
    assert!(!path.with_extension("tmp").exists());
}

#[test]
fn ui_model_records_opened_scores_and_filters_recent_section() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut model = UiModel::new();
    model.set_recent_path(dir.path().join("recent.txt"));
    model.set_search_rows(vec![row('a', "One"), row('b', "Two")]);
    model.record_opened(&key('b')).expect("record");
    model.select_section(LibrarySection::Recent);

    let titles: Vec<String> = model.visible_rows().into_iter().map(|row| row.title).collect();
    assert_eq!(titles, ["Two"]);

    let reloaded = notae_android::recent::RecentScores::load(&dir.path().join("recent.txt"));
    assert_eq!(reloaded.keys(), [key('b')]);
}

#[test]
fn selection_is_dropped_when_the_row_disappears() {
    let mut model = UiModel::new();
    model.set_search_rows(vec![row('a', "One")]);
    model.select_note(Some(key('a')));
    assert_eq!(model.selected_row().map(|row| row.title), Some("One".to_owned()));

    model.set_search_rows(vec![row('b', "Two")]);
    assert!(model.selected_row().is_none());
}
```

- [x] **Step 2: Run to verify they fail**

Run: `cargo test -p notae-android --test recent`
Expected: FAIL to compile — `notae_android::recent` is missing.

- [x] **Step 3: Write `recent.rs`**

```rust
//! The most recently opened scores, stored as content keys in a private text file.

#![forbid(unsafe_code)]

use std::fs;
use std::io;
use std::path::Path;

/// How many scores the Recent section remembers.
pub const RECENT_LIMIT: usize = 30;

/// Content keys (lowercase SHA-256 hex) of recently opened scores, newest first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecentScores {
    keys: Vec<String>,
}

fn is_content_key(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

impl RecentScores {
    /// Parses one key per line; invalid lines and repeats are dropped.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let mut recent = Self::default();
        for line in text.lines().map(str::trim).filter(|line| is_content_key(line)) {
            if !recent.keys.iter().any(|key| key == line) && recent.keys.len() < RECENT_LIMIT {
                recent.keys.push(line.to_owned());
            }
        }
        recent
    }

    /// One key per line, newest first.
    #[must_use]
    pub fn to_text(&self) -> String {
        self.keys.iter().map(|key| format!("{key}\n")).collect()
    }

    /// Reads `path`; a missing or unreadable file is an empty list.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path).map_or_else(|_| Self::default(), |text| Self::from_text(&text))
    }

    /// Writes through a temporary file so a crash never leaves half a list.
    ///
    /// # Errors
    ///
    /// Returns the I/O error from writing or renaming the file.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, self.to_text())?;
        fs::rename(temporary, path)
    }

    /// Moves `key` to the front. Keys that are not content keys are ignored.
    pub fn record(&mut self, key: &str) {
        if !is_content_key(key) {
            return;
        }
        self.keys.retain(|existing| existing != key);
        self.keys.insert(0, key.to_owned());
        self.keys.truncate(RECENT_LIMIT);
    }

    /// Keys, newest first.
    #[must_use]
    pub fn keys(&self) -> &[String] {
        &self.keys
    }
}
```

Note the test `record_keeps_at_most_the_limit` uses `{index:064x}`, which is valid lowercase hex.

Add `pub mod recent;` to `src/lib.rs` next to `pub mod state;`.

- [x] **Step 4: Extend `UiModel`**

In `src/ui.rs` add imports:

```rust
use std::path::PathBuf;

use crate::recent::RecentScores;
use crate::ui::browse::{LibrarySection, section_rows};
```

Add fields to `UiModel`:

```rust
    section: LibrarySection,
    recent: RecentScores,
    recent_path: Option<PathBuf>,
    selected_note: Option<String>,
```

initialize them in `from_parts` with `LibrarySection::All`, `RecentScores::default()`, `None`, `None`, and add:

```rust
    /// Loads the recent list from, and later saves it to, `path`.
    pub fn set_recent_path(&mut self, path: PathBuf) {
        self.recent = RecentScores::load(&path);
        self.recent_path = Some(path);
    }

    /// Records that the score with `note_key` was opened.
    ///
    /// # Errors
    ///
    /// Returns the I/O error when the list cannot be saved; the in-memory list is still updated.
    pub fn record_opened(&mut self, note_key: &str) -> std::io::Result<()> {
        self.recent.record(note_key);
        match &self.recent_path {
            Some(path) => self.recent.save(path),
            None => Ok(()),
        }
    }

    /// Content keys of recently opened scores, newest first.
    #[must_use]
    pub fn recent(&self) -> &[String] {
        self.recent.keys()
    }

    /// The side-panel section on display.
    #[must_use]
    pub fn section(&self) -> &LibrarySection {
        &self.section
    }

    /// Switches section and closes the details pane.
    pub fn select_section(&mut self, section: LibrarySection) {
        self.section = section;
        self.selected_note = None;
    }

    /// Rows of the current section. Task 6 makes this screen-aware.
    #[must_use]
    pub fn visible_rows(&self) -> Vec<SearchRow> {
        section_rows(&self.search_rows, &self.section, self.recent.keys())
    }

    /// Opens (`Some`) or closes (`None`) the details pane.
    pub fn select_note(&mut self, note_key: Option<String>) {
        self.selected_note = note_key;
    }

    /// The row shown in the details pane, if it is still in the library.
    #[must_use]
    pub fn selected_row(&self) -> Option<SearchRow> {
        let key = self.selected_note.as_deref()?;
        self.search_rows.iter().find(|row| row.note_key == key).cloned()
    }
```

- [x] **Step 5: Run the tests**

Run: `cargo test -p notae-android`
Expected: all pass, including the six in `tests/recent.rs`.

- [x] **Step 6: Lint and commit**

```bash
cargo clippy -p notae-android --all-targets -- -D warnings
git add apps/notae-android/src/recent.rs apps/notae-android/src/lib.rs apps/notae-android/src/ui.rs apps/notae-android/tests/recent.rs
git commit -m "feat(android): remember recently opened scores"
```

---

### Task 5: Search scopes and lyrics snippets

**Files:**
- Modify: `crates/notae-index/src/search.rs`, `crates/notae-index/src/store.rs`, `crates/notae-index/src/lib.rs`
- Modify: `apps/notae-android/src/ui/search.rs` (`split_snippet`), `apps/notae-android/src/ui/library.rs`, `apps/notae-android/src/operations/rescan.rs`
- Test: `crates/notae-index/tests/store_and_search.rs`, `apps/notae-android/tests/search_state.rs`

**Interfaces:**
- Produces in `notae_index`: `enum SearchScope { Everything, Titles, Composers, Tags, Text }` (`Default = Everything`, `Copy`, `key(self) -> &'static str`, `from_key(&str) -> Self`); `IndexStore::search_in(&self, query: &str, scope: SearchScope, limit: usize) -> Result<Vec<SearchHit>, IndexError>`; `SearchHit::text_snippet: Option<String>` (match wrapped in `\u{2}` … `\u{3}`).
- Produces in `notae_android::ui::search`: `fn split_snippet(snippet: &str) -> (String, String, String)` returning (before, match, after), with `before` trimmed to its last 48 characters.
- Produces: `LibraryViewModel::set_scope(SearchScope)`; `OperationController::set_search(query: String, scope: SearchScope)`.

- [x] **Step 1: Write the failing index tests**

Append to `crates/notae-index/tests/store_and_search.rs` (add `SearchScope` to the `notae_index` import):

```rust
fn scoped_store() -> IndexStore {
    let store = IndexStore::open(":memory:").expect("open");
    let schubert = note("ave.pdf", 1, Some(NoteId::new(13)), "Ave Maria", Some("Schubert"), &["solo"]);
    let antiphons = note("antiphons.pdf", 2, Some(NoteId::new(77)), "Marian antiphons", Some("Traditional"), &["choir"]);
    store.upsert_note(&schubert).expect("insert");
    store
        .upsert_note_with_ocr(
            &antiphons,
            Some(&OcrText::new("Regina caeli laetare alleluia. Ave Maria gratia plena Dominus tecum")),
        )
        .expect("insert");
    store
}

#[test]
fn text_scope_matches_only_page_text_and_returns_a_marked_snippet() {
    let hits = scoped_store().search_in("ave maria", SearchScope::Text, 10).expect("search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].note.title(), "Marian antiphons");
    let snippet = hits[0].text_snippet.as_deref().expect("snippet");
    assert!(snippet.contains("\u{2}Ave\u{3}"), "{snippet:?}");
}

#[test]
fn titles_scope_ignores_page_text() {
    let hits = scoped_store().search_in("ave", SearchScope::Titles, 10).expect("search");
    let titles: Vec<&str> = hits.iter().map(|hit| hit.note.title()).collect();
    assert_eq!(titles, ["Ave Maria"]);
    assert!(hits[0].text_snippet.is_none());
}

#[test]
fn composers_and_tags_scopes_use_their_columns() {
    let store = scoped_store();
    assert_eq!(store.search_in("traditional", SearchScope::Composers, 10).expect("search").len(), 1);
    assert_eq!(store.search_in("choir", SearchScope::Tags, 10).expect("search").len(), 1);
    assert!(store.search_in("choir", SearchScope::Composers, 10).expect("search").is_empty());
}

#[test]
fn everything_scope_matches_search() {
    let store = scoped_store();
    let scoped = store.search_in("ave", SearchScope::Everything, 10).expect("search");
    let plain = store.search("ave", 10).expect("search");
    let titles = |hits: &[notae_index::SearchHit]| hits.iter().map(|hit| hit.note.title().to_owned()).collect::<Vec<_>>();
    assert_eq!(titles(&scoped), titles(&plain));
}

#[test]
fn punctuation_only_query_in_any_scope_returns_nothing() {
    let store = scoped_store();
    for scope in [SearchScope::Everything, SearchScope::Titles, SearchScope::Composers, SearchScope::Tags, SearchScope::Text] {
        assert!(store.search_in("?!…", scope, 10).expect("search").is_empty());
    }
}

#[test]
fn scope_keys_round_trip() {
    for scope in [SearchScope::Everything, SearchScope::Titles, SearchScope::Composers, SearchScope::Tags, SearchScope::Text] {
        assert_eq!(SearchScope::from_key(scope.key()), scope);
    }
    assert_eq!(SearchScope::from_key("bogus"), SearchScope::Everything);
}
```

- [x] **Step 2: Run to verify they fail**

Run: `cargo test -p notae-index --test store_and_search`
Expected: FAIL to compile — `SearchScope` not found.

- [x] **Step 3: Implement scopes and snippets in `notae-index`**

In `crates/notae-index/src/search.rs` add:

```rust
/// Which `notes_fts` columns a search looks at.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SearchScope {
    /// Every indexed column.
    #[default]
    Everything,
    /// Note number, title and filename.
    Titles,
    /// Composer or artist.
    Composers,
    /// Tags.
    Tags,
    /// Lyrics and other page text.
    Text,
}

impl SearchScope {
    /// Stable key used by the Android UI.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Everything => "everything",
            Self::Titles => "titles",
            Self::Composers => "composers",
            Self::Tags => "tags",
            Self::Text => "text",
        }
    }

    /// Parses a key; unknown keys search everything.
    #[must_use]
    pub fn from_key(key: &str) -> Self {
        match key {
            "titles" => Self::Titles,
            "composers" => Self::Composers,
            "tags" => Self::Tags,
            "text" => Self::Text,
            _ => Self::Everything,
        }
    }

    /// FTS5 column filter, or `None` for every column.
    const fn columns(self) -> Option<&'static str> {
        match self {
            Self::Everything => None,
            Self::Titles => Some("note_id title filename"),
            Self::Composers => Some("composer"),
            Self::Tags => Some("tags"),
            Self::Text => Some("ocr_text"),
        }
    }
}
```

Add the field to `SearchHit`:

```rust
    /// Page-text excerpt around the match, with the matched terms between
    /// `\u{2}` and `\u{3}`; `None` when the page text did not match.
    pub text_snippet: Option<String>,
```

Change `SEARCH_SQL`'s first line to append the snippet after `notes.*` (columns are read by name, `relevance` stays at index 0):

```rust
SELECT bm25(notes_fts, 8.0, 10.0, 6.0, 5.0, 4.0, 1.0) AS relevance, notes.*,
       snippet(notes_fts, 5, char(2), char(3), '…', 12) AS text_snippet
```

Change the signature to `pub(crate) fn search(conn: &Connection, query: &str, scope: SearchScope, limit: usize)`, call `match_expression(query, scope)`, read the snippet in the row closure:

```rust
        |row| {
            let relevance: f64 = row.get(0)?;
            let snippet: Option<String> = row.get("text_snippet")?;
            let note = crate::store::note_from_row(row)?;
            Ok((relevance, snippet, note))
        },
```

and build hits with:

```rust
        let (relevance, snippet, note) = row?;
        …
        hits.push(SearchHit {
            rank: exact_id_match + exact_title_match - relevance,
            text_snippet: snippet.filter(|text| text.contains('\u{2}')),
            note,
        });
```

Change `match_expression`:

```rust
fn match_expression(query: &str, scope: SearchScope) -> String {
    // (term splitting unchanged)
    let body = terms
        .into_iter()
        .map(|term| format!("\"{term}\"*"))
        .collect::<Vec<_>>()
        .join(" ");
    match (body.is_empty(), scope.columns()) {
        (true, _) => body,
        (false, None) => body,
        (false, Some(columns)) => format!("{{{columns}}} : ({body})"),
    }
}
```

In `store.rs`:

```rust
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, IndexError> {
        self.search_in(query, SearchScope::Everything, limit)
    }

    /// Like [`IndexStore::search`], restricted to the columns of `scope`.
    ///
    /// # Errors
    ///
    /// Fails when the database cannot be read or holds a value that no longer parses.
    pub fn search_in(
        &self,
        query: &str,
        scope: SearchScope,
        limit: usize,
    ) -> Result<Vec<SearchHit>, IndexError> {
        crate::search::search(&self.conn, query, scope, limit)
    }
```

with `use crate::search::{SearchHit, SearchScope};`. In `lib.rs` change the re-export to `pub use crate::search::{SearchHit, SearchScope};`.

- [x] **Step 4: Run the index tests**

Run: `cargo test -p notae-index`
Expected: all pass, including the six new tests and every existing search test.

- [x] **Step 5: Write the failing Android tests**

Append to `apps/notae-android/tests/search_state.rs` (add `use notae_android::ui::search::split_snippet;`):

```rust
#[test]
fn split_snippet_returns_text_around_the_first_match() {
    assert_eq!(
        split_snippet("…alleluia. \u{2}Ave\u{3} \u{2}Maria\u{3} gratia…"),
        ("…alleluia. ".to_owned(), "Ave".to_owned(), " Maria gratia…".to_owned())
    );
}

#[test]
fn split_snippet_keeps_only_the_last_48_characters_before_the_match() {
    let long = format!("{}\u{2}hit\u{3}", "x".repeat(100));
    let (before, hit, after) = split_snippet(&long);
    assert_eq!(before, format!("…{}", "x".repeat(47)));
    assert_eq!((hit.as_str(), after.as_str()), ("hit", ""));
}

#[test]
fn library_view_model_applies_scope_and_fills_snippets() {
    let store = store();
    insert(&store, &note("a.pdf", 1, Some(1), "Antiphons", None, &[], IndexStatus::Ready), Some("Ave Maria gratia plena"));
    insert(&store, &note("b.pdf", 2, Some(2), "Ave verum", None, &[], IndexStatus::Ready), None);
    let mut model = LibraryViewModel::new(&store);
    model.set_query("ave".to_owned());
    model.set_scope(notae_index::SearchScope::Text);

    let rows = model.search().expect("search");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Antiphons");
    assert!(rows[0].text_snippet.is_some());
}
```

- [x] **Step 6: Implement the Android side**

In `src/ui/search.rs`:

```rust
/// Splits an FTS5 snippet into (before, match, after) around its first marked match.
#[must_use]
pub fn split_snippet(snippet: &str) -> (String, String, String) {
    let clean = |text: &str| text.replace(['\u{2}', '\u{3}'], "");
    let Some((before, rest)) = snippet.split_once('\u{2}') else {
        return (clean(snippet), String::new(), String::new());
    };
    let (hit, after) = rest.split_once('\u{3}').unwrap_or((rest, ""));
    let before = clean(before);
    let count = before.chars().count();
    let before = if count > 48 {
        format!("…{}", before.chars().skip(count - 47).collect::<String>())
    } else {
        before
    };
    (before, hit.to_owned(), clean(after))
}
```

In `src/ui/library.rs` add a `scope: SearchScope` field (initialized to `SearchScope::default()`), a setter:

```rust
    /// Restricts the next search to `scope`.
    pub fn set_scope(&mut self, scope: SearchScope) {
        self.scope = scope;
    }
```

and change the non-empty branch of `search` to keep snippets:

```rust
        if self.query.trim().is_empty() {
            return Ok(self.store.list_all()?.into_iter().map(SearchRow::from_note).collect());
        }
        Ok(self
            .store
            .search_in(&self.query, self.scope, usize::MAX)?
            .into_iter()
            .map(|hit| {
                let mut row = SearchRow::from_note(hit.note);
                row.text_snippet = hit.text_snippet;
                row
            })
            .collect())
```

In `src/operations/rescan.rs`: change `WorkerCommand::Search(String)` to `Search(String, SearchScope)`; keep `let mut scope = SearchScope::default();` beside `query` in `run_worker`; in the `Search` arm set both; change `project_rows(local, query)` to `project_rows(local, query, scope)` everywhere (pass `scope` into `run_rescan` next to `&query`), and:

```rust
fn project_rows(local: &IndexStore, query: &str, scope: SearchScope) -> Result<Vec<SearchRow>, UiError> {
    let mut view_model = LibraryViewModel::new(local);
    view_model.set_query(query.to_owned());
    view_model.set_scope(scope);
    view_model.search()
}
```

On `OperationController`:

```rust
    /// Re-runs the search with `scope` on the worker-owned index.
    pub fn set_search(&self, query: String, scope: SearchScope) -> Result<(), UiError> {
        self.commands
            .send(WorkerCommand::Search(query, scope))
            .map_err(|_| UiError::WorkerStopped)
    }

    /// Re-runs the search across every field.
    pub fn set_query(&self, query: String) -> Result<(), UiError> {
        self.set_search(query, SearchScope::Everything)
    }
```

- [x] **Step 7: Run all tests and lint**

Run: `cargo test -p notae-android -p notae-index && cargo clippy -p notae-android -p notae-index --all-targets -- -D warnings`
Expected: all pass, no warnings.

- [x] **Step 8: Commit**

```bash
git add crates/notae-index apps/notae-android/src apps/notae-android/tests
git commit -m "feat(search): add search scopes and page-text snippets"
```

---

### Task 6: Search screen navigation in the state machine

**Files:**
- Modify: `apps/notae-android/src/state.rs`
- Modify: `apps/notae-android/src/ui.rs` (`visible_rows`)
- Test: `apps/notae-android/tests/search_state.rs`

**Interfaces:**
- Consumes: `notae_index::SearchScope` and `SearchScope::default() == Everything` from Task 5.
- Produces: `Screen::Search`; `UiEvent::OpenSearch`, `UiEvent::CloseSearch`, `UiEvent::UpdateSearchScope { scope: SearchScope }`; `SearchState { query: String, scope: SearchScope }`; `EventError::SearchUnavailable`.

- [x] **Step 1: Write the failing tests**

Append to `tests/search_state.rs` (add `use notae_index::SearchScope;`):

```rust
fn library_controller() -> AppController {
    let mut controller = AppController::new();
    controller
        .dispatch(UiEvent::PickLibraryRoot { root: PathBuf::from("/library") })
        .expect("pick root");
    controller
}

#[test]
fn search_opens_from_library_and_closing_clears_query_and_scope() {
    let mut controller = library_controller();
    controller.dispatch(UiEvent::OpenSearch).expect("open search");
    assert_eq!(controller.state().screen(), &Screen::Search);

    controller
        .dispatch(UiEvent::UpdateSearchQuery { query: "ave".to_owned() })
        .expect("type");
    controller
        .dispatch(UiEvent::UpdateSearchScope { scope: SearchScope::Text })
        .expect("scope");
    controller.dispatch(UiEvent::CloseSearch).expect("close search");

    assert_eq!(controller.state().screen(), &Screen::Library);
    assert_eq!(controller.state().search().query, "");
    assert_eq!(controller.state().search().scope, SearchScope::Everything);
}

#[test]
fn search_is_unavailable_before_a_library_is_open() {
    let mut controller = AppController::new();
    assert_eq!(
        controller.dispatch(UiEvent::OpenSearch),
        Err(EventError::SearchUnavailable)
    );
}

#[test]
fn closing_search_when_not_searching_is_rejected() {
    let mut controller = library_controller();
    assert_eq!(
        controller.dispatch(UiEvent::CloseSearch),
        Err(EventError::SearchUnavailable)
    );
}
```

- [x] **Step 2: Run to verify they fail**

Run: `cargo test -p notae-android --test search_state`
Expected: FAIL to compile — no `UiEvent::OpenSearch`.

- [x] **Step 3: Implement the state changes**

In `src/state.rs`:

```rust
use notae_index::SearchScope;
```

Add to `UiEvent`:

```rust
    /// The user opened the search screen (search icon or Ctrl K).
    OpenSearch,
    /// The user left the search screen; the query and scope reset.
    CloseSearch,
    /// The user picked a search scope chip.
    UpdateSearchScope {
        /// The chosen scope.
        scope: SearchScope,
    },
```

Add to `EventError`:

```rust
    /// Search opens only from the library and closes only from search.
    #[error("search is unavailable from the current screen")]
    SearchUnavailable,
```

Add to `Screen`:

```rust
    /// The search screen over the configured library.
    Search,
```

Change `SearchState`:

```rust
pub struct SearchState {
    /// The current text in the search box. Empty by default.
    pub query: String,
    /// Which fields the query searches.
    pub scope: SearchScope,
}
```

(`SearchScope: Default` is `Everything`, so `#[derive(Default)]` still works.)

Add the match arms in `dispatch`:

```rust
            UiEvent::OpenSearch => {
                if !matches!(&self.state.screen, Screen::Library) {
                    return Err(EventError::SearchUnavailable);
                }
                self.state.screen = Screen::Search;
                Ok(Transition::NoOp)
            }
            UiEvent::CloseSearch => {
                if !matches!(&self.state.screen, Screen::Search) {
                    return Err(EventError::SearchUnavailable);
                }
                self.state.screen = Screen::Library;
                self.state.search = SearchState::default();
                Ok(Transition::NoOp)
            }
            UiEvent::UpdateSearchScope { scope } => {
                self.state.search.scope = scope;
                Ok(Transition::NoOp)
            }
```

Any other exhaustive `match` on `Screen` in the crate (`cargo build` lists them) gets a `Screen::Search` arm that behaves like `Screen::Library`.

- [x] **Step 4: Make `visible_rows` screen-aware**

In `src/ui.rs` replace `visible_rows` with:

```rust
    /// Rows for the current screen: search hits on the Search screen, the section otherwise.
    #[must_use]
    pub fn visible_rows(&self) -> Vec<SearchRow> {
        if matches!(self.state().screen(), crate::state::Screen::Search) {
            self.search_rows.clone()
        } else {
            section_rows(&self.search_rows, &self.section, self.recent.keys())
        }
    }
```

- [x] **Step 5: Run the tests**

Run: `cargo test -p notae-android`
Expected: all pass.

- [x] **Step 6: Lint and commit**

```bash
cargo clippy -p notae-android --all-targets -- -D warnings
git add apps/notae-android/src/state.rs apps/notae-android/src/ui.rs apps/notae-android/tests/search_state.rs
git commit -m "feat(android): add search screen navigation"
```

---

### Task 7: First-page thumbnails

**Files:**
- Create: `apps/notae-android/src/thumbnails.rs`
- Modify: `apps/notae-android/src/lib.rs`, `src/storage.rs` (trait + forwards), `src/operations/rescan.rs`, `src/ui.rs`, `src/android/saf.rs`, `android/src/main/java/org/notae/android/NotaeActivity.java`
- Test: `apps/notae-android/tests/thumbnails.rs`

**Interfaces:**
- Produces `notae_android::thumbnails::ThumbnailCache` with `new(dir: PathBuf) -> io::Result<Self>`, `path_for(&self, note_key: &str) -> Option<PathBuf>`, `existing(&self, note_key: &str) -> Option<PathBuf>`, `ensure(&self, note_key: &str, render: impl FnOnce(&Path) -> Result<bool, PdfError>) -> Result<Option<PathBuf>, PdfError>`, `prune(&self, keep: &HashSet<String>) -> io::Result<usize>`.
- Produces `LibraryCatalog::render_thumbnail(&self, path: &RelativePdfPath, out: &Path) -> Result<bool, PdfError>` (default `Ok(false)`).
- Produces `OperationController::with_thumbnails(local, catalog, cache: ThumbnailCache) -> Self`.
- Produces `UiModel::set_thumbnail_cache(ThumbnailCache)` and `UiModel::thumbnail_cache(&self) -> Option<&ThumbnailCache>`.

- [x] **Step 1: Write the failing tests**

Create `tests/thumbnails.rs`:

```rust
use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use notae_android::operations::events::OperationEvent;
use notae_android::operations::rescan::OperationController;
use notae_android::pdf_pages::PdfError;
use notae_android::storage::LibraryCatalog;
use notae_android::thumbnails::ThumbnailCache;
use notae_domain::{PdfFingerprint, RelativePdfPath, Sha256Digest};
use notae_index::{CatalogError, IndexStore, PdfCatalog, PdfFile};

const KEY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn cache() -> (tempfile::TempDir, ThumbnailCache) {
    let dir = tempfile::tempdir().expect("temp dir");
    let cache = ThumbnailCache::new(dir.path().join("thumbnails")).expect("cache");
    (dir, cache)
}

#[test]
fn only_content_keys_get_a_path() {
    let (_dir, cache) = cache();
    assert!(cache.path_for(KEY).is_some());
    assert!(cache.path_for("../escape").is_none());
}

#[test]
fn ensure_renders_once_and_reuses_the_file() {
    let (_dir, cache) = cache();
    let first = cache
        .ensure(KEY, |out: &Path| { std::fs::write(out, b"png").unwrap(); Ok(true) })
        .expect("render");
    assert!(first.as_deref().is_some_and(Path::is_file));
    let second = cache
        .ensure(KEY, |_out: &Path| panic!("must not render twice"))
        .expect("reuse");
    assert_eq!(first, second);
}

#[test]
fn failed_render_leaves_no_partial_file() {
    let (_dir, cache) = cache();
    let result = cache.ensure(KEY, |out: &Path| {
        std::fs::write(out, b"half").unwrap();
        Err(PdfError::Platform("boom".to_owned()))
    });
    assert!(result.is_err());
    assert!(cache.existing(KEY).is_none());
    assert_eq!(std::fs::read_dir(cache.path_for(KEY).unwrap().parent().unwrap()).unwrap().count(), 0);
}

#[test]
fn declined_render_is_not_cached() {
    let (_dir, cache) = cache();
    assert_eq!(cache.ensure(KEY, |_out: &Path| Ok(false)).expect("declined"), None);
    assert!(cache.existing(KEY).is_none());
}

#[test]
fn prune_removes_thumbnails_of_deleted_scores() {
    let (_dir, cache) = cache();
    let other = "b".repeat(64);
    for key in [KEY, other.as_str()] {
        cache.ensure(key, |out: &Path| { std::fs::write(out, b"png").unwrap(); Ok(true) }).unwrap();
    }
    let keep: HashSet<String> = [KEY.to_owned()].into();
    assert_eq!(cache.prune(&keep).expect("prune"), 1);
    assert!(cache.existing(KEY).is_some());
    assert!(cache.existing(&other).is_none());
}

struct ThumbnailCatalog {
    file: PdfFile,
}

impl PdfCatalog for ThumbnailCatalog {
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError> {
        Ok(vec![self.file.clone()])
    }
}

impl LibraryCatalog for ThumbnailCatalog {
    fn render_thumbnail(&self, _path: &RelativePdfPath, out: &Path) -> Result<bool, PdfError> {
        std::fs::write(out, b"png").expect("write thumbnail");
        Ok(true)
    }
}

#[test]
fn rescan_writes_a_thumbnail_for_each_indexed_score() {
    let (_dir, cache) = cache();
    let digest = Sha256Digest::from_bytes([0xaa; 32]);
    let file = PdfFile::new(
        RelativePdfPath::new("0012 - Clair de lune.pdf").expect("path"),
        PdfFingerprint::new(digest, 1_024, None),
        "0012 - Clair de lune.pdf".to_owned(),
    );
    let controller = OperationController::with_thumbnails(
        IndexStore::open(":memory:").expect("index"),
        Box::new(ThumbnailCatalog { file }),
        cache.clone(),
    );
    controller.start_rescan().expect("rescan");
    loop {
        match controller.recv_event_timeout(Duration::from_secs(5)).expect("event") {
            OperationEvent::Completed { .. } => break,
            OperationEvent::Failed { reason } => panic!("{reason}"),
            _ => {}
        }
    }
    assert!(cache.existing(&digest.to_string()).is_some());
}
```

- [x] **Step 2: Run to verify they fail**

Run: `cargo test -p notae-android --test thumbnails`
Expected: FAIL to compile — `notae_android::thumbnails` is missing.

- [x] **Step 3: Write `thumbnails.rs`**

```rust
//! Private cache of first-page thumbnails, one PNG per content key.

#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::pdf_pages::PdfError;

/// Width in pixels thumbnails are rendered at.
pub const THUMBNAIL_WIDTH_PX: i32 = 400;

/// Directory of `<sha256>.png` files under the app's private storage.
#[derive(Debug, Clone)]
pub struct ThumbnailCache {
    dir: PathBuf,
}

fn is_content_key(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

impl ThumbnailCache {
    /// Creates the directory if needed.
    ///
    /// # Errors
    ///
    /// Returns the error from creating the directory.
    pub fn new(dir: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// Where the thumbnail for `note_key` lives; `None` for anything but a content key.
    #[must_use]
    pub fn path_for(&self, note_key: &str) -> Option<PathBuf> {
        is_content_key(note_key).then(|| self.dir.join(format!("{note_key}.png")))
    }

    /// The thumbnail file, when it has been rendered.
    #[must_use]
    pub fn existing(&self, note_key: &str) -> Option<PathBuf> {
        self.path_for(note_key).filter(|path| path.is_file())
    }

    /// Returns the cached thumbnail, rendering it with `render` when missing.
    ///
    /// `render` writes a PNG to the path it is given and returns `Ok(true)`, or
    /// `Ok(false)` when the document has nothing to show. Anything but `Ok(true)`
    /// removes the partial file.
    ///
    /// # Errors
    ///
    /// Returns the render error, or a platform error when the file cannot be moved into place.
    pub fn ensure(
        &self,
        note_key: &str,
        render: impl FnOnce(&Path) -> Result<bool, PdfError>,
    ) -> Result<Option<PathBuf>, PdfError> {
        let Some(path) = self.path_for(note_key) else {
            return Ok(None);
        };
        if path.is_file() {
            return Ok(Some(path));
        }
        let partial = path.with_extension("png.part");
        let outcome = render(&partial);
        if !matches!(outcome, Ok(true)) {
            let _ = fs::remove_file(&partial);
            return outcome.map(|_| None);
        }
        fs::rename(&partial, &path).map_err(|error| {
            let _ = fs::remove_file(&partial);
            PdfError::Platform(format!("cannot store thumbnail: {error}"))
        })?;
        Ok(Some(path))
    }

    /// Deletes thumbnails whose key is not in `keep`, and stray partial files.
    ///
    /// # Errors
    ///
    /// Returns the error from reading the directory.
    pub fn prune(&self, keep: &HashSet<String>) -> io::Result<usize> {
        let mut removed = 0;
        for entry in fs::read_dir(&self.dir)? {
            let path = entry?.path();
            let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
            let stale = match name.strip_suffix(".png") {
                Some(key) => !keep.contains(key),
                None => name.ends_with(".png.part"),
            };
            if stale && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }
}
```

Add `pub mod thumbnails;` to `src/lib.rs`.

- [x] **Step 4: Add `render_thumbnail` to the catalog trait**

In `src/storage.rs`, inside `trait LibraryCatalog`:

```rust
    /// Renders the first page as a PNG at `out`; `Ok(false)` when the platform cannot.
    fn render_thumbnail(&self, _path: &RelativePdfPath, _out: &Path) -> Result<bool, PdfError> {
        Ok(false)
    }
```

Forward it in `impl LibraryCatalog for LibrarySource`:

```rust
    fn render_thumbnail(&self, path: &RelativePdfPath, out: &Path) -> Result<bool, PdfError> {
        self.catalog.render_thumbnail(path, out)
    }
```

and in `impl LibraryCatalog for SharedLibraryCatalog`:

```rust
    fn render_thumbnail(&self, path: &RelativePdfPath, out: &Path) -> Result<bool, PdfError> {
        self.0
            .lock()
            .map_err(|_| PdfError::Platform("library source lock poisoned".to_owned()))?
            .render_thumbnail(path, out)
    }
```

(add `use std::path::Path;` if missing).

- [x] **Step 5: Generate thumbnails during rescan**

In `src/operations/rescan.rs`:
- Add a `thumbnails: Option<ThumbnailCache>` parameter to `run_worker` and a `thumbnails: Option<&ThumbnailCache>` parameter to `run_rescan`; `run_worker` passes `thumbnails.as_ref()`. Import `crate::thumbnails::ThumbnailCache`.
- Rename the body of `OperationController::new` to a private `fn start(local, catalog, thumbnails: Option<ThumbnailCache>) -> Self` that moves `thumbnails` into the worker closure, then:

```rust
    #[must_use]
    pub fn new(local: IndexStore, catalog: Box<dyn LibraryCatalog + Send>) -> Self {
        Self::start(local, catalog, None)
    }

    /// Like [`OperationController::new`], also rendering first-page thumbnails into `cache`.
    #[must_use]
    pub fn with_thumbnails(
        local: IndexStore,
        catalog: Box<dyn LibraryCatalog + Send>,
        cache: ThumbnailCache,
    ) -> Self {
        Self::start(local, catalog, Some(cache))
    }
```

- In `run_rescan`'s finalization loop, after the text-layer `failure` is computed and before sending `FileSucceeded`/`FileFailed`, add:

```rust
                if let (Some(cache), Some(fingerprint)) = (thumbnails, file.fingerprint()) {
                    let key = fingerprint.sha256().to_string();
                    if let Err(error) = cache.ensure(&key, |out| catalog.render_thumbnail(file.path(), out)) {
                        eprintln!("notae: no thumbnail for {path}: {error}");
                    }
                }
```

  A missing thumbnail is never a file failure.
- After the loop, before `project_rows`:

```rust
            if let Some(cache) = thumbnails {
                if let Ok(notes) = local.list_all() {
                    let keep = notes
                        .iter()
                        .map(|note| note.fingerprint().sha256().to_string())
                        .collect();
                    if let Err(error) = cache.prune(&keep) {
                        eprintln!("notae: cannot prune thumbnails: {error}");
                    }
                }
            }
```

- [x] **Step 6: Keep the cache on `UiModel`**

In `src/ui.rs` add a field `thumbnails: Option<ThumbnailCache>` (initialized `None`) and:

```rust
    /// Uses `cache` to look up row thumbnails.
    pub fn set_thumbnail_cache(&mut self, cache: ThumbnailCache) {
        self.thumbnails = Some(cache);
    }

    /// The thumbnail cache, once the Android entry point has created it.
    #[must_use]
    pub fn thumbnail_cache(&self) -> Option<&ThumbnailCache> {
        self.thumbnails.as_ref()
    }
```

- [x] **Step 7: Run the host tests**

Run: `cargo test -p notae-android`
Expected: all pass, including the six in `tests/thumbnails.rs`.

- [x] **Step 8: Render on Android**

Add to `NotaeActivity.java` (imports `android.graphics.Bitmap`, `android.graphics.Color`, `android.graphics.pdf.PdfRenderer`, `android.net.Uri`, `android.os.ParcelFileDescriptor`, `java.io.FileOutputStream`):

```java
    // Renders page 1 of a SAF document to a PNG in app-private storage for the library grid.
    public boolean renderNotaeThumbnail(String uri, String outPath, int widthPx) {
        try (ParcelFileDescriptor descriptor =
                 getContentResolver().openFileDescriptor(Uri.parse(uri), "r")) {
            if (descriptor == null) return false;
            try (PdfRenderer renderer = new PdfRenderer(descriptor)) {
                if (renderer.getPageCount() == 0) return false;
                try (PdfRenderer.Page page = renderer.openPage(0)) {
                    int height = Math.max(1, Math.round(
                        widthPx * (float) page.getHeight() / Math.max(1, page.getWidth())));
                    Bitmap bitmap = Bitmap.createBitmap(widthPx, height, Bitmap.Config.ARGB_8888);
                    bitmap.eraseColor(Color.WHITE);
                    page.render(bitmap, null, null, PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY);
                    try (FileOutputStream out = new FileOutputStream(outPath)) {
                        boolean written = bitmap.compress(Bitmap.CompressFormat.PNG, 100, out);
                        bitmap.recycle();
                        return written;
                    }
                }
            }
        } catch (Exception error) {
            return false;
        }
    }
```

In `src/android/saf.rs`, inside `impl LibraryCatalog for SafPdfCatalog`:

```rust
    fn render_thumbnail(
        &self,
        path: &notae_domain::RelativePdfPath,
        out: &std::path::Path,
    ) -> Result<bool, crate::pdf_pages::PdfError> {
        let uri = document_content_uri(self.app.clone(), &self.tree_uri, path.as_str())
            .map_err(|error| crate::pdf_pages::PdfError::Platform(error.to_string()))?;
        let uri = uri.as_str().to_owned();
        let out = out.to_string_lossy().into_owned();
        let activity_ptr = self.app.activity_as_ptr();
        with_env(&self.app, move |env| {
            let activity = activity_as_jobject(env, activity_ptr)?;
            let uri = env.new_string(&uri)?;
            let out = env.new_string(&out)?;
            env.call_method(
                &activity,
                jni_str!("renderNotaeThumbnail"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;I)Z"),
                &[
                    JValue::Object(&uri),
                    JValue::Object(&out),
                    JValue::Int(crate::thumbnails::THUMBNAIL_WIDTH_PX),
                ],
            )?
            .z()
        })
        .map_err(|error| crate::pdf_pages::PdfError::Platform(error.to_string()))
    }
```

(Use the same `JValue`/`jni_str!`/`jni_sig!` imports `saf.rs` already has.)

In `src/android_main.rs`, right after `index_path` is computed:

```rust
    let thumbnails = crate::thumbnails::ThumbnailCache::new(index_dir.join("thumbnails"))
        .expect("create private thumbnail directory");
    model.borrow_mut().set_thumbnail_cache(thumbnails.clone());
    model.borrow_mut().set_recent_path(index_dir.join("recent.txt"));
```

(If `model` is created after this point, move these three lines to just after `model` exists.) Pass `thumbnails` into `apply_picker_result` (new parameter `thumbnails: &ThumbnailCache`) and replace `OperationController::new(` there with:

```rust
                    let controller = OperationController::with_thumbnails(
                        local,
                        Box::new(SharedLibraryCatalog(Arc::clone(&source))),
                        thumbnails.clone(),
                    );
```

- [x] **Step 9: Check the Android build and lint**

Run: `cargo apk2 check -p notae-android --target aarch64-linux-android && cargo clippy -p notae-android --all-targets -- -D warnings`
Expected: no errors.

- [x] **Step 10: Commit**

```bash
git add apps/notae-android
git commit -m "feat(android): render first-page thumbnails during rescan"
```

---

### Task 8: Library screen

**Files:**
- Create: `apps/notae-android/ui/library.slint`
- Modify: `apps/notae-android/ui/app.slint`, `apps/notae-android/src/ui/search.rs` (Android `library_row`), `apps/notae-android/src/android_main.rs`

**Interfaces:**
- Consumes: Task 2 components; `UiModel::{visible_rows, section, select_section, recent, thumbnail_cache}`; `browse::{sections, composers, LibrarySection}`; `split_snippet`.
- Produces Slint structs `LibraryRow { id, title, composer_or_artist, tags, status, note_key, relative_path, text_pending: bool, has_thumbnail: bool, thumbnail: image, title_origin, composer_origin, snippet_before, snippet_match, snippet_after }` and `SectionItem { key, label, count: int, kind }`; component `LibraryScreen`.
- Produces `AppWindow` properties `sections: [SectionItem]`, `composers: [SectionItem]`, `selected_section: string`, `library_title: string`, `library_subtitle: string`; callbacks `section_selected(string)`, `search_opened()`, `score_selected(string)`.

- [ ] **Step 1: Create `ui/library.slint`**

```slint
// One UI library: side panel, large title, thumbnail grid and operation banner.
import { Theme, Glyph, IconButton, PillButton, Card, NavRow, Chip, StatusBadge, ActionInput } from "components.slint";

export struct LibraryRow {
    id: string,
    title: string,
    composer_or_artist: string,
    tags: string,
    status: string,
    note_key: string,
    relative_path: string,
    text_pending: bool,
    has_thumbnail: bool,
    thumbnail: image,
    title_origin: string,
    composer_origin: string,
    snippet_before: string,
    snippet_match: string,
    snippet_after: string,
}

export struct SectionItem {
    key: string,
    label: string,
    count: int,
    kind: string,
}

export component PagePreview inherits Rectangle {
    in property <LibraryRow> row;
    border-radius: 14px;
    background: Theme.paper;
    border-width: 1px;
    border-color: Theme.paper-edge;
    clip: true;
    if root.row.has_thumbnail : Image {
        width: parent.width;
        height: parent.height;
        source: root.row.thumbnail;
        image-fit: contain;
        accessible-role: none;
    }
    if !root.row.has_thumbnail : Glyph {
        kind: "document";
        width: min(40px, parent.width / 2);
        height: self.width;
        x: (parent.width - self.width) / 2;
        y: (parent.height - self.height) / 2;
    }
}

component ScoreCard inherits Rectangle {
    in property <LibraryRow> row;
    in property <bool> selected;
    callback clicked();
    border-radius: 20px;
    background: root.selected ? Theme.accent-soft : transparent;
    border-width: input.has-focus ? 2px : 0px;
    border-color: Theme.accent;
    accessible-role: button;
    accessible-label: root.row.title;
    accessible-description: root.row.composer_or_artist + (root.row.text_pending ? ". Text pending" : "");
    accessible-action-default => { if !Theme.modal-open { root.clicked(); } }
    forward-focus: input;
    VerticalLayout {
        padding: 6px;
        spacing: 10px;
        Rectangle {
            vertical-stretch: 1;
            PagePreview { row: root.row; width: parent.width; height: parent.height; }
            if root.row.text_pending : StatusBadge { text: "Text pending"; x: 8px; y: parent.height - self.height - 8px; }
        }
        VerticalLayout {
            spacing: 2px;
            padding-left: 4px;
            padding-right: 4px;
            Text { text: root.row.title; color: Theme.foreground; font-size: 16px; font-weight: 600; overflow: elide; }
            Text {
                text: (root.row.id != "" ? root.row.id + " · " : "") + root.row.composer_or_artist;
                color: Theme.muted;
                font-size: 14px;
                overflow: elide;
            }
        }
    }
    input := ActionInput {
        enabled: !Theme.modal-open;
        clicked => { self.focus(); root.clicked(); }
    }
}

component ScoreGrid inherits Flickable {
    in property <[LibraryRow]> rows;
    in property <string> selected-key;
    callback select(string);
    private property <int> columns: max(1, floor((self.width + 16px) / 192px));
    private property <length> cell-width: (self.width - (self.columns - 1) * 16px) / self.columns;
    private property <length> cell-height: self.cell-width * 1.3 + 64px;
    viewport-height: ceil(root.rows.length / root.columns) * (self.cell-height + 20px);
    for row[index] in root.rows : ScoreCard {
        x: mod(index, root.columns) * (root.cell-width + 16px);
        y: floor(index / root.columns) * (root.cell-height + 20px);
        width: root.cell-width;
        height: root.cell-height;
        row: row;
        selected: row.note_key == root.selected-key;
        clicked => { root.select(row.note_key); }
    }
}

component SidePanel inherits VerticalLayout {
    in property <[SectionItem]> sections;
    in property <[SectionItem]> composers;
    in property <string> selected-key;
    callback section-selected(string);
    callback settings-clicked();
    spacing: 16px;
    HorizontalLayout {
        height: 64px;
        padding-left: 12px;
        spacing: 12px;
        VerticalLayout { alignment: center; Image { source: @image-url("assets/notae.svg"); width: 40px; height: 40px; } }
        Text { text: "Notae"; font-size: 22px; font-weight: 700; color: Theme.foreground; vertical-alignment: center; }
    }
    Card {
        VerticalLayout {
            padding: 8px;
            for item in root.sections : NavRow {
                label: item.label;
                kind: item.kind;
                count: item.count;
                selected: item.key == root.selected-key;
                clicked => { root.section-selected(item.key); }
            }
        }
    }
    if root.composers.length > 0 : Card {
        vertical-stretch: 1;
        VerticalLayout {
            padding: 8px;
            HorizontalLayout { padding-left: 14px; Text { text: "Composers"; color: Theme.muted; font-size: 13px; font-weight: 600; height: 36px; vertical-alignment: center; } }
            ListView {
                for item in root.composers : NavRow {
                    label: item.label;
                    count: item.count;
                    selected: item.key == root.selected-key;
                    clicked => { root.section-selected(item.key); }
                }
            }
        }
    }
    if root.composers.length == 0 : Rectangle { vertical-stretch: 1; }
    NavRow { label: "Settings"; kind: "settings"; clicked => { root.settings-clicked(); } }
}

export component OperationBanner inherits Card {
    in property <string> text;
    in property <bool> busy;
    in property <float> fraction;
    in property <bool> cancel-enabled;
    in property <bool> acknowledge-enabled;
    callback cancel();
    callback acknowledge();
    VerticalLayout {
        padding: 16px;
        padding-left: 20px;
        spacing: 12px;
        HorizontalLayout {
            spacing: 12px;
            VerticalLayout { alignment: center; Glyph { kind: root.busy ? "scan" : "check"; } }
            Text { text: root.text; color: Theme.foreground; font-size: 15px; wrap: word-wrap; horizontal-stretch: 1; vertical-alignment: center; }
            if root.cancel-enabled : PillButton { text: "Cancel"; tonal: true; clicked => { root.cancel(); } }
            if root.acknowledge-enabled : PillButton { text: "Dismiss"; tonal: true; clicked => { root.acknowledge(); } }
        }
        if root.busy : Rectangle {
            height: 4px;
            border-radius: 2px;
            background: Theme.field;
            clip: true;
            accessible-role: progress-indicator;
            accessible-label: root.text;
            accessible-value: root.fraction > 0 ? root.fraction : "";
            accessible-value-minimum: 0;
            accessible-value-maximum: 1;
            Rectangle {
                width: parent.width * (root.fraction > 0 ? root.fraction : 0.3);
                x: root.fraction > 0 || Theme.reduced-motion ? 0px : -self.width + (parent.width + self.width) * mod(animation-tick(), 1600ms) / 1600ms;
                height: 4px;
                border-radius: 2px;
                background: Theme.accent;
            }
        }
    }
}

export component LibraryScreen inherits Rectangle {
    in property <bool> expanded;
    in property <[SectionItem]> sections;
    in property <[SectionItem]> composers;
    in property <string> selected-section;
    in property <string> title;
    in property <string> subtitle;
    in property <[LibraryRow]> rows;
    in property <string> selected-key;
    in property <bool> rescan-enabled;
    in property <string> progress-text;
    in property <bool> busy;
    in property <float> fraction;
    in property <bool> cancel-enabled;
    in property <bool> acknowledge-enabled;
    in property <string> pdf-error;
    callback section-selected(string);
    callback settings-clicked();
    callback search-clicked();
    callback rescan-clicked();
    callback cancel-clicked();
    callback acknowledge-clicked();
    callback score-selected(string);
    // When true, the details pane (Task 9, passed as children) replaces the grid.
    in property <bool> details-open;
    background: Theme.background;

    HorizontalLayout {
        padding: 16px;
        spacing: 16px;
        if root.expanded : SidePanel {
            width: 288px;
            sections: root.sections;
            composers: root.composers;
            selected-key: root.selected-section;
            section-selected(key) => { root.section-selected(key); }
            settings-clicked => { root.settings-clicked(); }
        }
        VerticalLayout {
            horizontal-stretch: 1;
            spacing: 12px;
            HorizontalLayout {
                padding-top: root.expanded ? 40px : 8px;
                padding-left: 8px;
                spacing: 4px;
                VerticalLayout {
                    horizontal-stretch: 1;
                    alignment: end;
                    spacing: 4px;
                    Text { text: root.title; color: Theme.foreground; font-size: root.expanded ? 40px : 30px; font-weight: 600; overflow: elide; }
                    Text { text: root.subtitle; color: Theme.muted; font-size: 15px; }
                }
                VerticalLayout {
                    alignment: end;
                    HorizontalLayout {
                        spacing: 4px;
                        IconButton { label: "Search"; icon: @image-url("material-icons/search.svg"); clicked => { root.search-clicked(); } }
                        IconButton { label: "Scan library"; icon: @image-url("material-icons/scan.svg"); enabled: root.rescan-enabled; clicked => { root.rescan-clicked(); } }
                        if !root.expanded : IconButton { label: "Settings"; icon: @image-url("material-icons/settings.svg"); clicked => { root.settings-clicked(); } }
                    }
                }
            }
            if !root.expanded : Flickable {
                height: 48px;
                viewport-width: chips.preferred-width;
                chips := HorizontalLayout {
                    spacing: 8px;
                    for item in root.sections : Chip {
                        label: item.label;
                        selected: item.key == root.selected-section;
                        clicked => { root.section-selected(item.key); }
                    }
                    for item in root.composers : Chip {
                        label: item.label;
                        selected: item.key == root.selected-section;
                        clicked => { root.section-selected(item.key); }
                    }
                }
            }
            if root.progress-text != "" : OperationBanner {
                text: root.progress-text;
                busy: root.busy;
                fraction: root.fraction;
                cancel-enabled: root.cancel-enabled;
                acknowledge-enabled: root.acknowledge-enabled;
                cancel => { root.cancel-clicked(); }
                acknowledge => { root.acknowledge-clicked(); }
            }
            if root.pdf-error != "" : Text { text: root.pdf-error; color: Theme.error; wrap: word-wrap; }
            if !root.details-open && root.rows.length > 0 : Card {
                vertical-stretch: 1;
                ScoreGrid {
                    x: 20px;
                    y: 20px;
                    width: parent.width - 40px;
                    height: parent.height - 20px;
                    rows: root.rows;
                    selected-key: root.selected-key;
                    select(key) => { root.score-selected(key); }
                }
            }
            if !root.details-open && root.rows.length == 0 : Card {
                vertical-stretch: 1;
                VerticalLayout {
                    alignment: center;
                    padding: 32px;
                    spacing: 16px;
                    HorizontalLayout { alignment: center; Glyph { kind: "folder"; width: 56px; height: 56px; } }
                    Text {
                        text: root.selected-section == "all" ? "A little space for your scores" : "Nothing here yet";
                        color: Theme.foreground; font-size: 22px; font-weight: 600; horizontal-alignment: center; wrap: word-wrap;
                    }
                    if root.selected-section == "all" : HorizontalLayout {
                        alignment: center;
                        PillButton { text: "Scan library"; primary: true; enabled: root.rescan-enabled; clicked => { root.rescan-clicked(); } }
                    }
                }
            }
            if root.details-open : Rectangle {
                vertical-stretch: 1;
                @children
            }
        }
    }
}
```

- [ ] **Step 2: Build to check the file compiles in isolation**

Add `import { LibraryScreen, LibraryRow, SectionItem } from "library.slint";` to `app.slint`, delete `app.slint`'s own `LibraryRow` struct and `ScoreRow` and `NavigationItem` components, and re-export the structs: `export { Theme, LibraryRow, SectionItem }`.

Run: `cargo build -p notae-android`
Expected: errors only where `app.slint` still uses the deleted components — fixed in Step 3.

- [ ] **Step 3: Use `LibraryScreen` in `AppWindow`**

In `AppWindow` add:

```slint
    in property <[SectionItem]> sections: [];
    in property <[SectionItem]> composers: [];
    in property <string> selected_section: "all";
    in property <string> library_title: "All scores";
    in property <string> library_subtitle: "";
    in property <string> selected_key: "";
    in property <bool> details_open: false;
    callback section_selected(string);
    callback search_opened();
    callback score_selected(string);
```

Replace the old navigation rail, header row, search field, progress banner, library list and empty state (everything rendered when `root.configured && !root.show_settings`) with:

```slint
            if root.configured && !root.show_settings && !root.show_search : LibraryScreen {
                vertical-stretch: 1;
                expanded: root.expanded;
                sections: root.sections;
                composers: root.composers;
                selected-section: root.selected_section;
                title: root.library_title;
                subtitle: root.library_subtitle;
                rows: root.library_rows;
                selected-key: root.selected_key;
                details-open: root.details_open;
                rescan-enabled: root.rescan_enabled;
                progress-text: root.operation_progress_text;
                busy: root.operation_busy;
                fraction: root.operation_fraction;
                cancel-enabled: root.cancel_enabled;
                acknowledge-enabled: root.acknowledge_enabled;
                pdf-error: root.pdf_launch_error_text;
                section-selected(key) => { root.section_selected(key); }
                settings-clicked => { root.settings_clicked(); }
                search-clicked => { root.search_opened(); }
                rescan-clicked => { root.rescan_clicked(); }
                cancel-clicked => { root.cancel_rescan_clicked(); }
                acknowledge-clicked => { root.acknowledge_clicked(); }
                score-selected(key) => { root.score_selected(key); }
            }
```

Add `in property <bool> show_search: false;` now (used in Task 10). Change the outer `VerticalLayout`'s `padding` to `0px` for the library (the screen pads itself). The settings screen keeps its current block until Task 11.

- [ ] **Step 4: Build the Slint row in Rust**

In `src/ui/search.rs` replace the Android `impl From<SearchRow> for crate::LibraryRow` with:

```rust
#[cfg(target_os = "android")]
pub(crate) fn library_row(
    row: SearchRow,
    thumbnails: Option<&crate::thumbnails::ThumbnailCache>,
) -> crate::LibraryRow {
    // Slint caches images loaded from a path, so repeated syncs do not re-decode them.
    let thumbnail = thumbnails
        .and_then(|cache| cache.existing(&row.note_key))
        .and_then(|path| slint::Image::load_from_path(&path).ok());
    let (before, hit, after) = row.text_snippet.as_deref().map(split_snippet).unwrap_or_default();
    crate::LibraryRow {
        id: row.id.unwrap_or_default().into(),
        title: row.title.into(),
        composer_or_artist: row.composer_or_artist.unwrap_or_default().into(),
        tags: row.tags.join(", ").into(),
        status: row.status.into(),
        note_key: row.note_key.into(),
        relative_path: row.relative_path.into(),
        text_pending: row.text_pending,
        has_thumbnail: thumbnail.is_some(),
        thumbnail: thumbnail.unwrap_or_default(),
        title_origin: row.title_origin.into(),
        composer_origin: row.composer_origin.into(),
        snippet_before: before.into(),
        snippet_match: hit.into(),
        snippet_after: after.into(),
    }
}
```

A PNG that fails to decode gives `None`, so the card shows the placeholder (Review Focus 2).

- [ ] **Step 5: Sync and wire in `android_main.rs`**

In `sync_window`, replace the `library_rows` block with:

```rust
    let thumbnails = model.thumbnail_cache();
    let rows: Vec<crate::LibraryRow> = model
        .visible_rows()
        .into_iter()
        .map(|row| crate::ui::search::library_row(row, thumbnails))
        .collect();
    let row_count = rows.len();
    window.set_library_rows(slint::ModelRc::new(slint::VecModel::from(rows)));

    let all_rows = model.search_rows();
    let sections: Vec<crate::SectionItem> = crate::ui::browse::sections(all_rows, model.recent())
        .into_iter()
        .map(|view| crate::SectionItem {
            key: view.key.into(),
            label: view.label.into(),
            count: i32::try_from(view.count).unwrap_or(i32::MAX),
            kind: view.kind.into(),
        })
        .collect();
    window.set_sections(slint::ModelRc::new(slint::VecModel::from(sections)));
    let composers: Vec<crate::SectionItem> = crate::ui::browse::composers(all_rows)
        .into_iter()
        .map(|composer| crate::SectionItem {
            key: crate::ui::browse::LibrarySection::Composer(composer.name.clone()).key().into(),
            label: composer.name.into(),
            count: i32::try_from(composer.count).unwrap_or(i32::MAX),
            kind: "".into(),
        })
        .collect();
    window.set_composers(slint::ModelRc::new(slint::VecModel::from(composers)));
    window.set_selected_section(model.section().key().into());
    window.set_library_title(model.section().title().into());
    window.set_library_subtitle(
        format!("{row_count} {}", if row_count == 1 { "score" } else { "scores" }).into(),
    );
    window.set_show_search(matches!(state.screen(), Screen::Search));
```

Register the section callback next to the other handlers:

```rust
    {
        let model = Rc::clone(&model);
        let window_weak = window.as_weak();
        window.on_section_selected(move |key| {
            model
                .borrow_mut()
                .select_section(crate::ui::browse::LibrarySection::from_key(key.as_str()));
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }
```

Leave `on_search_opened` and `on_score_selected` for Tasks 9 and 10 (Slint callbacks without a handler do nothing).

- [ ] **Step 6: Build, check and lint**

Run: `cargo build -p notae-android && cargo apk2 check -p notae-android --target aarch64-linux-android && cargo clippy -p notae-android --all-targets -- -D warnings`
Expected: no errors.

- [ ] **Step 7: Emulator check**

Install and launch. With a library that has PDFs: scan; expect the side panel with counts, "All scores" title, "N scores", and thumbnails appearing after the scan finishes. Tap "Text pending" and a composer: the grid and title change. Toggle dark mode (Task 1 commands): cards turn dark, thumbnails stay light. Shrink the window into split screen (< 840 px wide): the side panel becomes a chip row (Review Focus 5).

- [ ] **Step 8: Commit**

```bash
git add apps/notae-android
git commit -m "feat(android): One UI library with sections and thumbnails"
```

---

### Task 9: Score details pane

**Files:**
- Create: `apps/notae-android/ui/details.slint`
- Modify: `apps/notae-android/ui/app.slint`, `apps/notae-android/src/android_main.rs`

**Interfaces:**
- Consumes: `LibraryRow`, `PagePreview` (Task 8); `UiModel::{select_note, selected_row, record_opened}`.
- Produces: components `ScoreList`, `ScoreDetails`; `AppWindow` property `selected: LibraryRow`; callbacks `selection_cleared()`; `open_pdf_requested(string relative_path, string note_key)` (signature change).

- [ ] **Step 1: Create `ui/details.slint`**

```slint
// List-detail pane: the current section on the left, one score's details on the right.
import { Theme, Glyph, IconButton, PillButton, Card, StatusBadge, ActionInput } from "components.slint";
import { LibraryRow, PagePreview } from "library.slint";

component ScoreListItem inherits Rectangle {
    in property <LibraryRow> row;
    in property <bool> selected;
    callback clicked();
    height: 76px;
    border-radius: 18px;
    background: root.selected ? Theme.accent-soft : input.pressed ? Theme.field : transparent;
    border-width: input.has-focus ? 2px : 0px;
    border-color: Theme.accent;
    accessible-role: button;
    accessible-label: root.row.title;
    accessible-action-default => { if !Theme.modal-open { root.clicked(); } }
    forward-focus: input;
    HorizontalLayout {
        padding-left: 14px;
        padding-right: 14px;
        spacing: 14px;
        VerticalLayout { alignment: center; PagePreview { row: root.row; width: 40px; height: 52px; border-radius: 6px; } }
        VerticalLayout {
            alignment: center;
            spacing: 2px;
            Text { text: root.row.title; font-size: 16px; font-weight: 600; overflow: elide; color: root.selected ? Theme.on-accent-soft : Theme.foreground; }
            Text { text: (root.row.id != "" ? root.row.id + " · " : "") + root.row.tags; font-size: 14px; color: Theme.muted; overflow: elide; }
        }
    }
    input := ActionInput {
        enabled: !Theme.modal-open;
        clicked => { self.focus(); root.clicked(); }
    }
}

export component ScoreList inherits Card {
    in property <[LibraryRow]> rows;
    in property <string> selected-key;
    callback select(string);
    ListView {
        x: 8px;
        y: 8px;
        width: parent.width - 16px;
        height: parent.height - 16px;
        for row in root.rows : ScoreListItem {
            row: row;
            selected: row.note_key == root.selected-key;
            clicked => { root.select(row.note_key); }
        }
    }
}

component InfoRow inherits HorizontalLayout {
    in property <string> label;
    in property <string> value;
    in property <string> origin;
    in property <bool> last;
    min-height: 60px;
    spacing: 16px;
    Text { text: root.label; width: 110px; color: Theme.muted; font-size: 15px; vertical-alignment: center; }
    Text { text: root.value == "" ? "—" : root.value; horizontal-stretch: 1; color: Theme.foreground; font-size: 16px; vertical-alignment: center; wrap: word-wrap; }
    Text { text: root.origin; color: Theme.muted; font-size: 13px; vertical-alignment: center; }
}

export component ScoreDetails inherits Card {
    in property <LibraryRow> row;
    in property <bool> show-back;
    callback close();
    callback open();
    callback edit();
    VerticalLayout {
        padding: 8px;
        padding-bottom: 24px;
        HorizontalLayout {
            alignment: space-between;
            IconButton {
                label: root.show-back ? "Back to list" : "Close details";
                icon: root.show-back ? @image-url("material-icons/arrow-back.svg") : @image-url("material-icons/close.svg");
                clicked => { root.close(); }
            }
        }
        HorizontalLayout {
            padding-left: 24px;
            padding-right: 24px;
            spacing: 28px;
            PagePreview { row: root.row; width: 172px; height: 228px; border-radius: 16px; }
            VerticalLayout {
                spacing: 6px;
                padding-top: 12px;
                alignment: start;
                if root.row.id != "" : Text { text: "No. " + root.row.id; color: Theme.accent; font-size: 14px; font-weight: 600; }
                Text { text: root.row.title; color: Theme.foreground; font-size: 32px; font-weight: 600; wrap: word-wrap; }
                Text { text: root.row.composer_or_artist; color: Theme.muted; font-size: 17px; }
                if root.row.text_pending : StatusBadge { text: "Lyrics text pending"; }
                HorizontalLayout {
                    padding-top: 18px;
                    spacing: 10px;
                    alignment: start;
                    PillButton { text: "Open score"; primary: true; clicked => { root.open(); } }
                    PillButton { text: "Edit details"; tonal: true; clicked => { root.edit(); } }
                }
            }
        }
        Rectangle {
            height: 24px;
        }
        HorizontalLayout {
            padding-left: 24px;
            padding-right: 24px;
            Rectangle {
            border-radius: 20px;
            background: Theme.background;
            VerticalLayout {
                padding-left: 20px;
                padding-right: 20px;
                padding-top: 4px;
                padding-bottom: 4px;
                InfoRow { label: "Title"; value: root.row.title; origin: root.row.title_origin; }
                Rectangle { height: 1px; background: Theme.divider; }
                InfoRow { label: "Composer"; value: root.row.composer_or_artist; origin: root.row.composer_origin; }
                Rectangle { height: 1px; background: Theme.divider; }
                InfoRow { label: "Tags"; value: root.row.tags; origin: ""; }
                Rectangle { height: 1px; background: Theme.divider; }
                InfoRow { label: "File"; value: root.row.relative_path; origin: root.row.status; }
            }
            }
        }
        Rectangle { vertical-stretch: 1; }
    }
}
```

- [ ] **Step 2: Put the pane into `LibraryScreen`'s slot**

In `app.slint` import `ScoreList, ScoreDetails` from `details.slint`, add to `AppWindow`:

```slint
    in property <LibraryRow> selected;
    callback selection_cleared();
```

change `callback open_pdf_requested(string);` to `callback open_pdf_requested(string, string);`, and give the `LibraryScreen` element these children:

```slint
                HorizontalLayout {
                    spacing: 16px;
                    if root.expanded : ScoreList {
                        width: 400px;
                        rows: root.library_rows;
                        selected-key: root.selected_key;
                        select(key) => { root.score_selected(key); }
                    }
                    ScoreDetails {
                        horizontal-stretch: 1;
                        row: root.selected;
                        show-back: !root.expanded;
                        close => { root.selection_cleared(); }
                        open => { root.open_pdf_requested(root.selected.relative_path, root.selected.note_key); }
                        edit => { root.edit_metadata_requested(root.selected.note_key); }
                    }
                }
```

Escape closes the pane: in the window-level `FocusScope` added in Task 10, Escape with `details_open` calls `selection_cleared()`. Until then the close button is the way out.

- [ ] **Step 3: Wire selection and opening in `android_main.rs`**

In `sync_window` add:

```rust
    let selected = model.selected_row();
    window.set_details_open(selected.is_some());
    window.set_selected_key(selected.as_ref().map(|row| row.note_key.clone()).unwrap_or_default().into());
    if let Some(row) = selected {
        window.set_selected(crate::ui::search::library_row(row, model.thumbnail_cache()));
    }
```

Register:

```rust
    {
        let model = Rc::clone(&model);
        let window_weak = window.as_weak();
        window.on_score_selected(move |key| {
            model.borrow_mut().select_note(Some(key.to_string()));
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }
    {
        let model = Rc::clone(&model);
        let window_weak = window.as_weak();
        window.on_selection_cleared(move || {
            model.borrow_mut().select_note(None);
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }
```

Change `on_open_pdf_requested` to take `move |relative_path, note_key|`, and after `set_pdf_launch_error(...)` add:

```rust
            if result.is_ok() {
                if let Err(error) = model.borrow_mut().record_opened(note_key.as_str()) {
                    eprintln!("notae: cannot save recent scores: {error}");
                }
            }
```

(compute `let opened = result.is_ok();` before `result` is consumed by `set_pdf_launch_error` and test `opened`.)

- [ ] **Step 4: Build, check, lint**

Run: `cargo build -p notae-android && cargo apk2 check -p notae-android --target aarch64-linux-android && cargo clippy -p notae-android --all-targets -- -D warnings`
Expected: no errors.

- [ ] **Step 5: Emulator check**

Tap a score: the list appears left and details right, with the selected row highlighted. **Open score** opens the PDF viewer; going back, "Recent" counts 1. **Edit details** opens the editor; after saving, the details show "Edited" next to the changed field. In split screen the details take the full width with a back arrow (Review Focus 5).

- [ ] **Step 6: Commit**

```bash
git add apps/notae-android
git commit -m "feat(android): add score details pane"
```

---

### Task 10: Search screen and Ctrl K

**Files:**
- Create: `apps/notae-android/ui/search.slint`
- Modify: `apps/notae-android/ui/app.slint`, `apps/notae-android/src/android_main.rs`

**Interfaces:**
- Consumes: `UiEvent::{OpenSearch, CloseSearch, UpdateSearchScope}`, `OperationController::set_search`, `SearchScope::{key, from_key}`.
- Produces: `SearchScreen`; `AppWindow` property `search_scope: string`; callbacks `search_closed()`, `search_scope_changed(string)`, `search_result_chosen(string)`.

- [ ] **Step 1: Create `ui/search.slint`**

```slint
// The single search screen, opened by the library's search icon or Ctrl K.
import { Theme, Glyph, IconButton, SearchField, Card, Chip, ActionInput } from "components.slint";
import { LibraryRow, PagePreview } from "library.slint";

component ResultRow inherits Rectangle {
    in property <LibraryRow> row;
    callback clicked();
    height: root.row.snippet_match != "" ? 88px : 76px;
    border-radius: 18px;
    background: input.pressed ? Theme.field : transparent;
    border-width: input.has-focus ? 2px : 0px;
    border-color: Theme.accent;
    accessible-role: button;
    accessible-label: root.row.title;
    accessible-description: root.row.snippet_before + root.row.snippet_match + root.row.snippet_after;
    accessible-action-default => { root.clicked(); }
    forward-focus: input;
    HorizontalLayout {
        padding-left: 14px;
        padding-right: 14px;
        spacing: 16px;
        VerticalLayout { alignment: center; PagePreview { row: root.row; width: 40px; height: 52px; border-radius: 6px; } }
        VerticalLayout {
            alignment: center;
            spacing: 4px;
            Text { text: root.row.title; color: Theme.foreground; font-size: 16px; font-weight: 600; overflow: elide; }
            if root.row.snippet_match == "" : Text {
                text: (root.row.id != "" ? root.row.id + " · " : "") + root.row.composer_or_artist;
                color: Theme.muted; font-size: 14px; overflow: elide;
            }
            if root.row.snippet_match != "" : HorizontalLayout {
                Text { text: root.row.snippet_before; color: Theme.muted; font-size: 14px; }
                Text { text: root.row.snippet_match; color: Theme.accent; font-size: 14px; font-weight: 700; }
                Text { text: root.row.snippet_after; color: Theme.muted; font-size: 14px; overflow: elide; horizontal-stretch: 1; }
            }
        }
    }
    input := ActionInput { clicked => { self.focus(); root.clicked(); } }
}

export component SearchScreen inherits Rectangle {
    in-out property <string> query;
    in property <string> scope;
    in property <[LibraryRow]> rows;
    callback query-edited(string);
    callback scope-changed(string);
    callback close();
    callback chosen(string);
    public function focus-field() { field.focus(); }
    background: Theme.background;

    VerticalLayout {
        padding: 24px;
        padding-bottom: 0px;
        spacing: 16px;
        alignment: start;
        HorizontalLayout {
            spacing: 8px;
            VerticalLayout { alignment: center; IconButton { label: "Back to library"; icon: @image-url("material-icons/arrow-back.svg"); clicked => { root.close(); } } }
            field := SearchField {
                horizontal-stretch: 1;
                text <=> root.query;
                placeholder: "Search titles, composers, tags or lyrics";
                edited(value) => { root.query-edited(value); }
            }
        }
        HorizontalLayout {
            padding-left: 56px;
            spacing: 8px;
            alignment: start;
            Chip { label: "All"; selected: root.scope == "everything"; clicked => { root.scope-changed("everything"); } }
            Chip { label: "Titles"; selected: root.scope == "titles"; clicked => { root.scope-changed("titles"); } }
            Chip { label: "Composers"; selected: root.scope == "composers"; clicked => { root.scope-changed("composers"); } }
            Chip { label: "Lyrics"; selected: root.scope == "text"; clicked => { root.scope-changed("text"); } }
            Chip { label: "Tags"; selected: root.scope == "tags"; clicked => { root.scope-changed("tags"); } }
        }
        if root.query != "" && root.rows.length > 0 : Card {
            vertical-stretch: 1;
            ListView {
                x: 8px;
                y: 8px;
                width: parent.width - 16px;
                height: parent.height - 16px;
                for row in root.rows : ResultRow { row: row; clicked => { root.chosen(row.note_key); } }
            }
        }
        if root.query != "" && root.rows.length == 0 : Text {
            text: "No scores found";
            color: Theme.muted;
            font-size: 18px;
            horizontal-alignment: center;
        }
        Rectangle { vertical-stretch: 1; }
        Text {
            text: "Press Ctrl K to search from anywhere";
            color: Theme.muted;
            font-size: 13px;
            horizontal-alignment: center;
            height: 48px;
        }
    }
}
```

- [ ] **Step 2: Show it from `AppWindow` and add Ctrl K**

In `app.slint` import `SearchScreen`, add:

```slint
    in property <string> search_scope: "everything";
    callback search_closed();
    callback search_scope_changed(string);
    callback search_result_chosen(string);
```

Add the screen next to `LibraryScreen`:

```slint
            if root.configured && root.show_search : SearchScreen {
                vertical-stretch: 1;
                query: root.search_text;
                scope: root.search_scope;
                rows: root.library_rows;
                query-edited(value) => { root.search_text = value; root.search_changed(value); }
                scope-changed(key) => { root.search_scope_changed(key); }
                close => { root.search_closed(); }
                chosen(key) => { root.search_result_chosen(key); }
                init => { self.focus-field(); }
            }
```

Wrap the window's top-level `HorizontalLayout` in a `FocusScope` that handles shortcuts the focused control did not use. Key events bubble from the focused control up to this scope; `forward-focus` gives it focus when nothing else has it:

```slint
    forward-focus: shortcuts;
    shortcuts := FocusScope {
        width: root.width;
        height: root.height;
        key-pressed(event) => {
            if root.modal-open || !root.configured { return reject; }
            if event.modifiers.control && (event.text == "k" || event.text == "K") {
                if !root.show_search { root.search_opened(); }
                return accept;
            }
            if event.text == Key.Escape {
                if root.show_search { root.search_closed(); return accept; }
                if root.details_open { root.selection_cleared(); return accept; }
            }
            return reject;
        }
        // existing top-level HorizontalLayout goes here unchanged
    }
```

Delete the now-unused library `SearchField` block from Task 2 Step 8 if it is still in `app.slint`.

- [ ] **Step 3: Wire search in `android_main.rs`**

In `sync_window` add `window.set_search_scope(state.search().scope.key().into());`.

Replace the body of `on_search_changed` so it calls `controller.set_search(query, scope)` with `let scope = model.borrow().state().search().scope;`.

Add handlers (each ends with `sync_window` like the others):

```rust
    {
        let model = Rc::clone(&model);
        let window_weak = window.as_weak();
        window.on_search_opened(move || {
            if let Err(error) = model.borrow_mut().dispatch(UiEvent::OpenSearch) {
                eprintln!("notae: cannot open search: {error}");
            }
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }
    {
        let model = Rc::clone(&model);
        let operations = Rc::clone(&operations);
        let window_weak = window.as_weak();
        window.on_search_scope_changed(move |key| {
            let scope = notae_index::SearchScope::from_key(key.as_str());
            let _ = model.borrow_mut().dispatch(UiEvent::UpdateSearchScope { scope });
            let query = model.borrow().state().search().query.clone();
            if let Some(controller) = operations.borrow().as_ref() {
                if let Err(error) = controller.set_search(query, scope) {
                    eprintln!("notae: search failed: {error}");
                }
            }
            if let Some(window) = window_weak.upgrade() {
                sync_window(&window, &model.borrow());
            }
        });
    }
```

and a shared close helper used by both `on_search_closed` and `on_search_result_chosen`:

```rust
fn close_search(
    model: &Rc<RefCell<UiModel>>,
    operations: &Rc<RefCell<Option<OperationController>>>,
    selected: Option<String>,
) {
    if let Err(error) = model.borrow_mut().dispatch(UiEvent::CloseSearch) {
        eprintln!("notae: cannot close search: {error}");
    }
    model.borrow_mut().select_note(selected);
    if let Some(controller) = operations.borrow().as_ref() {
        if let Err(error) = controller.set_query(String::new()) {
            eprintln!("notae: library refresh failed: {error}");
        }
    }
}
```

`on_search_closed` calls `close_search(&model, &operations, None)`; `on_search_result_chosen(move |key| …)` calls `close_search(&model, &operations, Some(key.to_string()))`. Both then `sync_window`. The chosen score's details open once the worker sends the full library rows back.

- [ ] **Step 4: Build, check, lint**

Run: `cargo build -p notae-android && cargo apk2 check -p notae-android --target aarch64-linux-android && cargo clippy -p notae-android --all-targets -- -D warnings`
Expected: no errors.

- [ ] **Step 5: Emulator check**

Tap the search icon: the field is focused. Type a title prefix: results show. Pick **Lyrics** on a library with a text layer: rows show the snippet with the match in blue. Type `?!`: "No scores found". Choose a result: back in the library with its details open. With a hardware keyboard (`adb -s emulator-5554 shell input keycombination 113 39` sends Ctrl K): search opens; Escape closes it.

- [ ] **Step 6: Commit**

```bash
git add apps/notae-android
git commit -m "feat(android): add search screen with scopes and Ctrl K"
```

---

### Task 11: First run, settings and dialogs

**Files:**
- Create: `apps/notae-android/ui/setup.slint`
- Modify: `apps/notae-android/ui/app.slint`, `apps/notae-android/ui/components.slint` (remove legacy aliases)

**Interfaces:**
- Produces: `WelcomeScreen { recovery, folder, error-text; callback accepted() }`, `SettingsScreen { rescan-enabled, reduced-motion; callbacks back(), rescan() }`.

- [ ] **Step 1: Create `ui/setup.slint`**

```slint
// First run, folder recovery and settings in the One UI style.
import { CheckBox } from "std-widgets.slint";
import { Theme, Glyph, IconButton, PillButton, Card } from "components.slint";

component FeatureRow inherits HorizontalLayout {
    in property <string> kind;
    in property <string> heading;
    in property <string> detail;
    min-height: 80px;
    spacing: 18px;
    VerticalLayout {
        alignment: center;
        Rectangle {
            width: 44px;
            height: 44px;
            border-radius: 22px;
            background: Theme.accent-soft;
            Glyph { kind: root.kind; ink: Theme.on-accent-soft; x: 10px; y: 10px; }
        }
    }
    VerticalLayout {
        alignment: center;
        spacing: 2px;
        Text { text: root.heading; color: Theme.foreground; font-size: 16px; font-weight: 600; }
        Text { text: root.detail; color: Theme.muted; font-size: 14px; wrap: word-wrap; }
    }
}

export component WelcomeScreen inherits Rectangle {
    in property <bool> recovery;
    in property <string> folder;
    in property <string> error-text;
    callback accepted();
    background: Theme.background;
    VerticalLayout {
        padding-top: 96px;
        padding-bottom: 40px;
        alignment: start;
        spacing: 20px;
        HorizontalLayout { alignment: center; Image { source: @image-url("assets/notae.svg"); width: 96px; height: 96px; } }
        Text {
            text: root.recovery ? "Reconnect your library" : "Welcome to Notae";
            color: Theme.foreground; font-size: 40px; font-weight: 600; horizontal-alignment: center; wrap: word-wrap;
        }
        HorizontalLayout {
            alignment: center;
            Text {
                width: min(560px, root.width - 48px);
                text: root.recovery
                    ? "Notae no longer has access to “" + root.folder + "”. Choose it again to reconnect your scores."
                    : "Your sheet music, in good order. Notae reads your PDFs where they are and opens them in the PDF app you already use.";
                color: Theme.muted; font-size: 17px; horizontal-alignment: center; wrap: word-wrap;
            }
        }
        HorizontalLayout {
            alignment: center;
            Card {
                width: min(560px, root.width - 48px);
                VerticalLayout {
                    padding-left: 24px;
                    padding-right: 24px;
                    padding-top: 8px;
                    padding-bottom: 8px;
                    FeatureRow { kind: "folder"; heading: "Files stay where they are"; detail: "Nothing is moved, renamed or uploaded"; }
                    Rectangle { height: 1px; background: Theme.divider; }
                    FeatureRow { kind: "search"; heading: "Find any score fast"; detail: "Search by title, composer, tag or lyrics"; }
                }
            }
        }
        Rectangle { vertical-stretch: 1; }
        if root.error-text != "" : Text { text: root.error-text; color: Theme.error; horizontal-alignment: center; wrap: word-wrap; }
        HorizontalLayout {
            alignment: center;
            PillButton { width: 360px; text: root.recovery ? "Reconnect folder" : "Choose score folder"; primary: true; clicked => { root.accepted(); } }
        }
        Text { text: "Device storage or SD card"; color: Theme.muted; font-size: 14px; horizontal-alignment: center; }
    }
}

export component SettingsScreen inherits Rectangle {
    in property <bool> rescan-enabled;
    in-out property <bool> reduced-motion;
    callback back();
    callback rescan();
    background: Theme.background;
    VerticalLayout {
        padding: 24px;
        spacing: 16px;
        alignment: start;
        HorizontalLayout {
            spacing: 8px;
            IconButton { label: "Back to library"; icon: @image-url("material-icons/arrow-back.svg"); clicked => { root.back(); } }
            Text { text: "Settings"; color: Theme.foreground; font-size: 40px; font-weight: 600; vertical-alignment: center; }
        }
        HorizontalLayout {
            alignment: center;
            VerticalLayout {
                width: min(720px, root.width - 48px);
                spacing: 16px;
                Card {
                    VerticalLayout {
                        padding: 24px;
                        spacing: 12px;
                        Text { text: "Library"; color: Theme.foreground; font-size: 18px; font-weight: 600; }
                        Text { text: "Your PDFs stay in the folder you chose. Scanning refreshes the index, thumbnails and the desktop import check."; color: Theme.muted; font-size: 15px; wrap: word-wrap; }
                        HorizontalLayout { alignment: start; PillButton { text: "Scan library"; tonal: true; enabled: root.rescan-enabled; clicked => { root.rescan(); } } }
                    }
                }
                Card {
                    VerticalLayout {
                        padding: 24px;
                        spacing: 12px;
                        Text { text: "Motion"; color: Theme.foreground; font-size: 18px; font-weight: 600; }
                        CheckBox { text: "Reduce animations"; enabled: !Theme.modal-open; checked <=> root.reduced-motion; }
                    }
                }
                Card {
                    VerticalLayout {
                        padding: 24px;
                        spacing: 8px;
                        Text { text: "Notae"; color: Theme.foreground; font-size: 18px; font-weight: 600; }
                        Text { text: "A personal home for sheet music. Light and dark follow your system setting."; color: Theme.muted; font-size: 15px; wrap: word-wrap; }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 2: Swap them into `AppWindow`**

In `app.slint` import `WelcomeScreen, SettingsScreen` from `setup.slint`; delete `WelcomePanel`, `Label`, `SupportingText` (if no longer used) and the old settings `ScrollView`. Use:

```slint
            if !root.configured : WelcomeScreen {
                vertical-stretch: 1;
                recovery: root.show_recovery;
                folder: root.recovery_folder_name;
                error-text: root.setup_error_text;
                accepted => { root.setup_chosen(); }
            }
            if root.configured && root.show_settings : SettingsScreen {
                vertical-stretch: 1;
                rescan-enabled: root.rescan_enabled;
                reduced-motion <=> root.reduced_motion;
                back => { root.settings_closed(); }
                rescan => { root.rescan_clicked(); }
            }
```

- [ ] **Step 3: Restyle the dialog sheet**

In the modal `sheet := Rectangle`: `background: Theme.card; border-radius: 26px;` and scrim `opacity: root.sheet-progress * 0.4;`. Header text: `font-size: 24px; font-weight: 600; color: Theme.foreground`. Field labels use `color: Theme.muted; font-size: 13px; font-weight: 600`. In `ImportEntry` use `background: Theme.background; border-radius: 20px;` and replace `Theme.primary` with `Theme.accent`. The dialog buttons keep `PillButton` with `primary`/`tonal`.

- [ ] **Step 4: Remove the legacy aliases**

Delete the nine alias properties from `Theme` in `components.slint` (Task 1 Step 1 comment marks them). Run `cargo build -p notae-android`; every error names a leftover use — replace `surface`/`container` → `card`, `high` → `field`, `primary` → `accent`, `on-primary` → `on-accent`, `primary-container` → `accent-soft`, `on-container` → `on-accent-soft`, `outline` → `divider`, `success` → `accent`.

- [ ] **Step 5: Build, check, lint**

Run: `cargo build -p notae-android && cargo apk2 check -p notae-android --target aarch64-linux-android && cargo clippy -p notae-android --all-targets -- -D warnings && cargo test -p notae-android -p notae-index`
Expected: everything passes.

- [ ] **Step 6: Emulator check**

Clear app data (`adb -s emulator-5554 shell pm clear org.notae.android`), launch: the welcome screen matches canvas board 4 in both themes. Pick a folder, open Settings, toggle Reduce animations, go back. Open the editor and, with a sidecar present, the import preview: both sheets use the card style; Tab stays inside; Escape closes.

- [ ] **Step 7: Commit**

```bash
git add apps/notae-android
git commit -m "feat(android): restyle first run, settings and dialogs"
```

---

### Task 12: Spec, runbook and device verification

**Files:**
- Modify: `docs/superpowers/specs/2026-10-09-notae-design.md` (lines 131 and 308)
- Modify: `docs/android-debugging.md` (redesign smoke checks)
- Modify: `docs/superpowers/IMPLEMENTATION-STATE.md`
- Modify: `apps/notae-android/Cargo.toml` (comment "PDFs are never rendered by the app")

- [ ] **Step 1: Update the spec**

Replace line 131 "Notae does not render PDF pages." with:

```markdown
Notae renders only the first page of each PDF, once, into a private thumbnail cache for the library grid. Viewing a score is still delegated to an installed PDF app.
```

In line 308 replace "The Android app does not run OCR or render PDFs." with "The Android app does not run OCR. It renders first-page thumbnails with `PdfRenderer` during a rescan." Add a short "Android visual design" paragraph pointing to the canvas URL and the palette in this plan's Global Constraints.

- [ ] **Step 2: Update the Cargo.toml comment**

In `apps/notae-android/Cargo.toml` change "PDFs are never rendered by the app." to "PDFs are rendered only to first-page thumbnails."

- [ ] **Step 3: Update the runbook**

In `docs/android-debugging.md` replace the "Redesign smoke checks" paragraph with:

```markdown
Redesign smoke checks: in light and dark (`adb shell cmd uimode night yes|no`) check the welcome screen, scan a library and wait for thumbnails, switch sections (All, Recent, Needs details, Text pending, a composer), tap a score to open its details, use **Open score** and confirm it appears under Recent, edit details, open search from the icon and with Ctrl K, try each scope chip, choose a result, and open Settings. Check split screen (< 840 px): chips replace the side panel and details take the full width. While a sheet is open, Tab stays inside it and Escape dismisses it.
```

If Task 1 needed the launch-time fallback, add: "Light/dark is read at launch; switch the system mode, then relaunch Notae."

- [ ] **Step 4: Verify on the tablet**

Build the aarch64 APK (keep a copy of the x86_64 one first), install with `adb -s R5GL61JXE8F install -r target/release/apk/notae-android.apk`, and run the smoke checks above. Also confirm the system font is Samsung's UI font (compare the "All scores" title with the Samsung Settings app). Capture crash logs with `adb -s R5GL61JXE8F logcat -b crash -v threadtime` if anything fails.

- [ ] **Step 5: Record state**

Add a "2026-10-14 One UI redesign" section to `docs/superpowers/IMPLEMENTATION-STATE.md` listing the commits from Tasks 1–11, what was verified on the emulator and on the tablet, and the deferred item: page numbers for lyrics matches.

- [ ] **Step 6: Commit**

```bash
git add docs apps/notae-android/Cargo.toml
git commit -m "docs: record One UI redesign and thumbnail rendering"
```
