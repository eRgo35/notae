# 0001. On-device OCR for image-only PDFs

- **Status:** Proposed (pending maintainer approval; nothing here is implemented)
- **Date:** 2026-10-18
- **Issue:** #28 (companion: #33, related: #18)
- **Supersedes:** the settled decision "The Android app does not run OCR" in `docs/superpowers/specs/2026-10-09-notae-design.md` (line 308) and the note "Android never runs OCR" in `docs/android-debugging.md`. Both stay as written until this ADR is accepted; both carry a pointer to it now.
- **Evidence and plan:** [`docs/superpowers/specs/2026-10-18-on-device-ocr-design.md`](../superpowers/specs/2026-10-18-on-device-ocr-design.md)

## Context

Image-only PDFs (scanned score pages) are not searchable on Android. The app reads only an embedded text layer through `PdfRenderer` (`operations/rescan.rs::apply_pdf_text_layer` → `classify_text_layer`), and only on API 35 and later (`PdfRenderer.Page.getTextContents`). A PDF without text becomes `IndexStatus::OcrPending`. Text for such files exists only if the desktop `notae-index` (Tesseract 5 + Leptonica, 300 dpi, `pol`+`eng`) produced a sidecar that the user imports. The maintainer wants the tablet to do this itself, automatically and locally (#28), and wants the "Text pending" section and "No text yet" badge gone (#33).

Constraints from #28 and the product: fully local, no telemetry, no network, no Google-service dependency, Polish diacritics (ą ć ę ł ń ó ś ź ż), minSdk 23, arm64, small APK growth, background and cancellable work with truthful progress (#18), battery-conscious, pages rendered with `PdfRenderer`, and a build that works in `cargo-apk2` (no Gradle) and in the hosted CI runner (which has already run out of disk once).

## Options considered

| Option | Verdict | Deciding reason |
| --- | --- | --- |
| **A. Tesseract 5.5.3 + Leptonica 1.87.0 cross-compiled with the NDK, bundled `tessdata_fast` `pol`+`eng`** | **Proposed** | Meets every constraint. Observed: builds with NDK r26d for `aarch64-linux-android`, existing `tesseract`/`tesseract-sys` crates cross-link against it, the arm64 binary recognised Polish on the emulator and matched desktop output. Same engine and version as the desktop indexer. |
| B. Google ML Kit Text Recognition v2 (bundled Latin) | Rejected | Google documents that the SDK sends device, app, identifier and performance data for "diagnostics and usage analytics" over HTTPS: a telemetry dependency by design. Proprietary binary under the ML Kit Terms of Service. Needs Gradle-style dependency resolution, manifest merging and a `ContentProvider` that `cargo-apk2` cannot declare. Camera/scene oriented. |
| C1. `ocrs` (pure Rust, `rten`) | Rejected | The shipped recognition alphabet is ASCII only; the README says "Latin alphabet only (eg. English)" and "early preview". No Polish diacritics without training new models. |
| C2. PaddleOCR PP-OCR latin via ONNX Runtime | Not selected; kept as the revisit candidate | Plausible Polish coverage and Apache-2.0, but not prototyped: needs ONNX Runtime for Android (~10+ MB native), own detection post-processing, and `ort` is a release candidate with unconfirmed Android binaries. |
| D. Hybrid: desktop sidecar only | Rejected as the answer; kept as an accelerator | Does not meet "no desktop step". The sidecar import stays and outranks device OCR. |

## Decision (proposed)

1. **Engine.** Run Tesseract 5.5.3 (+ Leptonica 1.87.0, + google/cpu_features 0.10.1, which Tesseract's CMake requires on Android) on the device, linked statically with `c++_static` into `libnotae_android.so`. Use the same Tesseract version as the desktop indexer. Bundle `tessdata_fast` `pol` and `eng` as APK assets (no `osd`), copied to app-private storage under a versioned name.
2. **Recognition settings.** Fixed `pol+eng`, page segmentation mode 3, `SetSourceResolution` always set (measured: omitting it drops a test page from 8/9 to 4/9 correct lines), one reused engine handle, words kept only if they contain letters and clear a confidence threshold chosen on real scans (staves, clefs and noteheads otherwise become junk tokens in FTS).
3. **Pages.** Rendered by `PdfRenderer` into a white-backed bitmap at 150-200 dpi (final value set by the P0 spike), capped by total pixel count, converted to 8-bit gray and handed to Rust.
4. **Build.** Native dependencies are built from pinned, SHA-256-verified sources by a repository script (not by `build.rs`), cached in CI by script hash and NDK version, and consumed by the existing `tesseract-sys`/`leptonica-sys` through target-suffixed `pkg-config`/`bindgen` environment variables, so host builds are unchanged. A CI check on the produced `.so` asserts it is self-contained.
5. **Workflow.** After a scan completes and any desktop sidecar decision is resolved, the worker recognises `OcrPending` documents one at a time, page by page, as a new cancellable `RescanPhase::RecognizingText`. Each document commits atomically with `reindex_note_with_ocr`; failure marks it `OcrIncomplete` and is never retried automatically. The `OcrPending` set is the queue, so a restart resumes without extra state.
6. **Provenance.** A schema migration adds `ocr_source` (`text_layer` | `sidecar` | `device`) and `ocr_engine` (engine version + tessdata hash + mode). Precedence is `text_layer` > `sidecar` > `device`. Device text never replaces the other two; a sidecar may replace device text once. No automatic re-OCR when the engine changes.
7. **Background.** Phase 1 is in-app only: runs in the foreground, pauses on `onStop`, on battery saver, below 20 % battery unless charging, and at thermal status SEVERE or worse (API 29+). No wake locks, no screen-on. A foreground service is a separate decision (not part of this ADR's acceptance).
8. **Control.** A setting "Recognise text in scanned PDFs", on by default. Pause persists across launches. A one-time estimate is shown for large backlogs.

## Interim behaviour until OCR lands (#33)

No "Text pending" section and no "No text yet" badge anywhere. Settings → Library maintenance shows **PDFs without extractable text** as a plain count, so removing the badge does not remove the only signal. On API 23-34 the app does not read text layers at all, so the interim wording for those devices must say text was not read rather than that none exists (the maintainer's tablet runs API 35). When OCR lands, the same Settings line becomes "N PDFs waiting for text recognition" and the banner carries the live phase.

## Invariants this decision adds (each to be enforced by a check)

- **No network.** The manifest declares no `INTERNET` permission. Verified on the v0.2.0 release APK (no `uses-permission` entries); a CI check keeps it so.
- **Self-contained native code.** `readelf -d` shows only `libc`, `libm`, `libdl`, `liblog` as NEEDED, and `nm -uD` shows no `_ZNSt`/`__cxa` symbols (a cdylib otherwise links with undefined libc++ symbols and fails at load time; observed).
- **Truthful progress.** Page and file counters change only on a real completed page or file. No estimate before three measured pages.
- **Crash safety.** Tesseract/Leptonica may `abort()`. An `ocr_attempting` marker is written before each document; on startup a marked note becomes `OcrIncomplete`, so one bad file cannot crash the app on every launch.
- **Commit safety.** The file's content hash is re-checked before a result is committed; a rescan or deletion during OCR discards the result.

## Consequences

- The APK grows by about 5.5-6 MB (11.4 MB → about 17 MB; estimate: ~1.6 MB code + ~4 MB compressed `pol`+`eng`). Budget: at most about 18 MB.
- Library text becomes searchable without a desktop step, and on API 23-34 it also covers PDFs that do have a text layer (which the app cannot read there today).
- New obligations: Apache-2.0/BSD-2 notices (Tesseract, tessdata, cpu_features, Leptonica) shown in an About screen; an ADR-level quality statement (OCR serves titles, composers, lyrics, not notation); a CI job for the native prefix (about 2-3 minutes cached-miss on 4 vCPUs, estimated from 4 min 20 s CPU measured).
- `allowBackup` is not set in the manifest (platform default is on), so OCR text in the database may be sent to Google backup. Decide whether to disable it in the same change; this ADR recommends disabling it.
- Code that states the old decision must change with the implementation: `apps/notae-android/src/state.rs` (`RescanPhase::CheckingDesktopImport` doc), `apps/notae-android/src/ui/browse.rs` (Text pending explanation), the Android section of `docs/android-debugging.md`, the OCR sentence in the design spec, and the `Cargo.toml` comment on `notae-ocr`.

## Revert triggers

Written down so the decision can be re-opened on evidence:

- More than about 8 s per page at 200 dpi after warm-up on the Galaxy Tab S10 FE+: make OCR opt-in or charging-only.
- On real scans, a high rate of garbage tokens that confidence filtering cannot remove while PaddleOCR is clearly better on the same pages: reconsider C2.
- APK above about 18 MB, or the native CI job making the pipeline unreliable (disk, time).

## Gate before acceptance

The accepting maintainer should require the P0 device spike (see the design doc): load the cross-built library on the tablet and recognise about 30 real scanned pages, recording seconds per page, memory and temperature for PSM 3 vs sparse modes, `pol` vs `pol+eng`, and `fast` vs `best`. The author of this ADR was not allowed to touch the tablet; no on-device (arm64 hardware) timing exists yet.

## Decisions the maintainer must make

1. Accept Tesseract on-device (engine) and the roughly 6 MB APK growth?
2. Default behaviour: automatic and foreground-only with a setting (recommended), or opt-in?
3. Approve the P0 spike on the physical tablet, and who runs it?
4. Disable `allowBackup` in the same release?
5. Foreground service for off-screen OCR: separate follow-up issue, or out of scope?
