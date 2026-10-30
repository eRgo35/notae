# 0002. Jetpack Compose UI over a UniFFI Rust core; Android only

- **Status:** Accepted (2026-10-30; maintainer decisions in a design interview)
- **Date:** 2026-10-30
- **Supersedes:** "The UI is implemented in Slint and driven by Rust application state" in `docs/superpowers/specs/2026-10-09-notae-design.md` (UI section), and the desktop indexer and sidecar workflow described there and in `README.md`.
- **Defers:** ADR 0001 (on-device OCR). Its decision stands, but the work is postponed and `crates/notae-ocr` moves to the `archive/notae-ocr` branch.
- **Design:** [`docs/superpowers/specs/2026-10-30-compose-migration-design.md`](../superpowers/specs/2026-10-30-compose-migration-design.md)

## Context

The Android app renders with Slint 1.18 inside a `NativeActivity` built by `cargo-apk2`. This has three costs that grow with the app:

- **Accessibility.** Slint's Android backend publishes no accessibility tree. Notae carries its own bridge: `src/accessibility.rs`, `src/android/accessibility.rs` and the 943-line `NotaeAccessibility.java`, which together walk the Slint item tree and build `AccessibilityNodeInfo`s by hand.
- **Text input.** The IME is wired by hand (`src/android/search_input.rs`).
- **Look.** One UI styling is reimplemented component by component.

Before a Google Play release the maintainer wants native accessibility, native text input, predictive back, and a current target SDK. `cargo-apk2` with NDK r26d caps the target at API 34, and Play needs 16 KB-aligned native libraries.

Most of the application logic is already independent of Slint. `UiModel`, the state machine, the stores, browse/search projections and the operation worker hold no Slint types; about 3,300 of the app's 10,000 Rust lines touch Slint.

The desktop indexer (`apps/notae-index`) and its sidecar import exist only to give scanned PDFs searchable text. The maintainer has decided Notae is Android only.

## Decision

1. **UI:** Jetpack Compose with Material 3, themed to match the current Slint app's One UI tokens exactly. It uses the system font, light and dark, tablet only, with dynamic colour off.
2. **Core:** a new crate `crates/notae-app-core` holds the code that is free of Slint. It exposes one UniFFI object. Rust owns the screen state and pushes snapshots to Kotlin; Kotlin sends actions back. Rust no longer calls Android APIs.
3. **Platform code in Kotlin:** folder picker and persisted permission, document listing and hashing, `PdfRenderer` text layer and thumbnails, opening PDFs, voice search, per-app language, and accessibility/motion settings. Kotlin passes plain data into Rust through a UniFFI callback interface.
4. **Build:** Gradle (Kotlin DSL, AGP 9) with `cargo-ndk` replaces `cargo-apk2`. The Rust toolchain moves to NDK r28 or newer. The build produces an AAB and an APK, with target SDK 36 and min SDK 26. The application id is `com.c2yz.notae`, signed with the existing release key.
5. **Navigation:** Navigation 3, with the back stack derived from Rust's screen state. Android Back calls the core's `back()`.
6. **Strings:** `strings.xml` with `values-pl`. Rust returns data and status codes, never display text. `pick()`, `@tr` and the `.po` catalog are removed.
7. **Cutover:** a single switch on `feat/compose-migration`. Slint is deleted only after a parity check against reference screenshots and an action checklist.
8. **Android only:** remove `apps/notae-index`, sidecar import, the Tesseract CI setup and the indexer docs. Keep `crates/notae-ocr` on `archive/notae-ocr` for later on-device OCR. Until then, scanned PDFs are found by file name only.
9. **Release:** 0.3.0 is the last Slint release. It ships favorites, sets and an "Export sets" button so data can move to the new package. The Compose app is 0.5.0. There is no 0.4.0. Play submission is a separate step.

## Options considered

| Option | Verdict | Reason |
| --- | --- | --- |
| Stay on Slint | Rejected | Accessibility and IME remain hand-built; target SDK capped at 34 by the cargo-apk2/NDK pairing. |
| **Compose + UniFFI, Rust owns state** | **Accepted** | Native accessibility, input and back; reuses the tested Rust state machine; Kotlin stays a rendering and platform layer. |
| Compose + hand-written JNI | Rejected | Dozens of calls and records; JNI glue is where bugs and `unsafe` accumulate. |
| Compose with Kotlin ViewModels owning state, Rust as a query library | Rejected | Rewrites the tested `UiModel` logic in Kotlin. |
| Screen-by-screen migration | Rejected | A `NativeActivity` cannot host Slint and Compose screens together in a practical way. |

## Consequences

- New dependencies, approved 2026-10-30: Gradle and AGP 9, Kotlin, `cargo-ndk`, the Compose BOM, Material 3, Navigation 3, AndroidX Activity, UniFFI with JNA, Robolectric, Compose UI test and Roborazzi. The app still has no Google Play Services, no DI framework and no networking.
- The `unsafe` exception for the `android_main` FFI shim disappears. Every crate can `forbid(unsafe_code)`.
- About 15 Slint UI test files are replaced by Robolectric Compose tests and Roborazzi screenshot tests. The Rust logic tests carry over. The screenshot tests close engineering-standards gap COND-005.
- Users of `org.notae.android` must export from 0.3.0 and import in 0.5.0. The two apps install side by side.
- In 0.5.0, scanned PDFs lose the searchable text that desktop sidecars provided.
