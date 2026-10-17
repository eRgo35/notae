# On-device OCR for image-only PDFs: design and feasibility

Status: proposal for maintainer approval. The decision record is [ADR 0001](../../adr/0001-on-device-ocr.md); this document holds the evidence, the risks and the phased plan. Issue: #28 (companion #33, related #18). Nothing in the app crates was changed to produce it.

How to read the evidence. Every fact carries a tag.

- **[O]** observed by the author on this host or the shared emulator during the spike (commands in the spike log).
- **[D]** read from documentation, package metadata or source (URL or file given).
- **[I]** inference, not verified.

## 1. What the app does today

| Area | Fact | Source |
| --- | --- | --- |
| Text extraction | The rescan worker calls `apply_pdf_text_layer` for every catalogued PDF; it asks the catalog for the text layer and maps the answer with `classify_text_layer`. No text (or an empty string) becomes `IndexStatus::OcrPending` unless searchable text is already stored, in which case the record stays `Ready`. A platform failure becomes `OcrIncomplete`. [D] | `apps/notae-android/src/operations/rescan.rs` (`apply_pdf_text_layer`, ~565), `src/pdf_pages.rs` (`classify_text_layer`, 86; `missing_text_outcome`, 111) |
| API gate | The text layer is read only when `Build.VERSION.SDK_INT >= 35` (`PdfRenderer.Page.getTextContents`). On API 23-34 the layer is not read at all, so every PDF, including born-digital ones, ends `OcrPending`. [D] | `src/android/pdf_pages.rs` (`API_PDF_TEXT_CONTENTS`, 16; 45) |
| Rendering | `PdfRenderer` is used only for the first-page thumbnail: `NotaeActivity.renderNotaeThumbnail(uri, outPath, widthPx)` opens the SAF file descriptor, renders page 0 into a white-backed ARGB bitmap and writes a PNG. [D] | `apps/notae-android/android/src/main/java/org/notae/android/NotaeActivity.java` (153) |
| Operation model | `OperationState::Rescan { phase }` with `RescanPhase::{Enumerating, Indexing{total}, ReadingText{completed,total}, CheckingDesktopImport}`. Only `Enumerating` is cancellable; `Completed` is the last event of a rescan and is sent after the sidecar check. [D] | `src/state.rs` (166, 200, 224), `src/operations/events.rs` |
| Desktop OCR | `notae-index` renders each page with `hayro` at 300 dpi (`DEFAULT_OCR_DPI`), runs Tesseract per page with `PSM_AUTO_OSD` and a `LanguagePolicy`, joins pages with a blank line, and fails the whole file if any page errors. [D] | `apps/notae-index/src/pdf.rs` (82), `src/pipeline.rs` (369-440), `crates/notae-ocr/src/tesseract.rs` |
| `notae-ocr` | `OcrProvider::recognize(&GrayImage, &LanguagePolicy) -> Result<OcrText, OcrError>`; Tesseract implementation via `tesseract` 0.15.2. It never calls `SetSourceResolution`. It does not build for Android: `tesseract-sys` 0.6.3 / `leptonica-sys` 0.4.9 find the libraries only through `pkg-config` and run `bindgen`; the crate's own `build.rs` hard-codes Linux paths. [D] | `crates/notae-ocr/{README.md,build.rs,src}` |
| Storage | `notes.ocr_text` and `notes.ocr_language` hold one whole-document normalised string per note (whitespace collapsed); FTS5 row is rewritten with it. `IndexStatus` has `OcrPending` and `OcrIncomplete`. Migrations are tracked in `schema_migrations`. [D] | `crates/notae-index/src/store.rs`, `schema.rs`, `crates/notae-domain/src/status.rs` |
| Sidecar | The desktop writes `.notae/manifest.json` (schema version 2) with per-entry `status` and OCR text. Import replaces local OCR text only when missing or different (`OcrAssessment::{Current, Importing, NotOffered, NotApplied}`) and never blanks it. [D] | `crates/notae-index/src/sidecar.rs`, `import.rs` (57-63, 171) |
| Manifest | No `uses-permission` entries in the v0.2.0 release APK; no `allowBackup` attribute (platform default: on). `libnotae_android.so` is 30.1 MB raw, 11.3 MB deflated; APK 11.4 MB. [O] `aapt2 dump badging` / `unzip -lv` on the downloaded release asset. | |
| Packaging | `cargo-apk2` 1.4.0 compiles `java_sources` with `javac` into one `classes.jar` and dexes it; it can declare `<service>` and `<activity>` but has no provider or receiver, no jar/AAR inputs, no dependency resolution and no manifest merging. [D] | `cargo-apk2-1.4.0/src/apk.rs`, `ndk-build2-1.4.2/src/manifest.rs` (registry sources) |

## 2. Options evaluated

Legend for the constraint columns: yes / no / partial, with the evidence tag.

| Constraint | A. Tesseract (proposed) | B. ML Kit bundled Latin | C1. `ocrs` | C2. PaddleOCR via ONNX Runtime | D. Desktop sidecar only |
| --- | --- | --- | --- | --- | --- |
| Fully local, no network at runtime | yes: no network code; manifest has no `INTERNET` [O] | model is local [D]; SDK telemetry over HTTPS by default [D] | yes [D] | yes [D] | yes |
| No telemetry | yes: no network permission, and the build disables curl and archive support [O]; Tesseract itself was not audited by the author [I] | **no**: collects device/app info, per-installation identifiers, performance metrics, event types, error codes "for diagnostics and usage analytics", HTTPS [D: developers.google.com/ml-kit/android-data-disclosure, updated 2026-07-15]; the only hard block is omitting `INTERNET` [I] | yes [D] | yes [D] | yes |
| No Google-service dependency | yes | bundled variant statically links the model, but pulls `play-services-base/basement/tasks`, Firebase components and datatransport Java libraries (POM) [D, O: AAR/POM downloaded]; runtime behaviour without Play services not run [I] | yes | yes | yes |
| Polish diacritics | yes: `tessdata_fast` `pol`; 75/80 diacritic characters correct on a synthetic 150 dpi page, same engine as the desktop [O] | `pl` listed under "Supported languages" (Latin) [D: ML Kit languages page] | **no**: `DEFAULT_ALPHABET` is ASCII only [D: `ocrs/src/lib.rs`]; README "Latin alphabet only (eg. English)", "early preview" [D] | Polish is in PaddleOCR's latin model list [I: from memory and the reviewer, not re-checked]; not prototyped | yes (desktop) |
| APK size | +about 1.6 MB code (gzip of stripped test `.so`) + 4.0 MB `pol`+`eng` fast (gzip) = about 5.6 MB [O] | arm64 `libmlkit_google_ocr_pipeline.so` 11.1 MB raw / 4.4 MB gzip + models 1.5 MB + Java about 1.8 MB [O: AARs unpacked] | models 2.5 MB + 9.7 MB [D: HTTP `Content-Length`]; runtime pure Rust | ONNX Runtime Android native (~10+ MB) + models [I] | 0 |
| Build in `cargo-apk2` (no Gradle) | yes with a prebuilt prefix: cross-link verified [O]; CMake from source works with NDK r26d [O] | **no** without patching `cargo-apk2`: needs about 20 transitive jars/AARs, manifest merge, a `ContentProvider` [D, O] | yes (pure Rust) [I] | hard: native runtime, `ort` 2.0.0-rc.14 is a release candidate, Android prebuilt binaries unconfirmed [D/I] | n/a |
| Cancel / progress | page granularity between pages; no monitor exposed by `tesseract-plumbing` 0.11.1 [D] | per-image task [D] | per-image [I] | per-image [I] | n/a |
| CPU / battery | CPU-bound, about 0.4-0.9 s per A4 page single-thread on a desktop core (tessdata_fast) [O]; tablet timing unknown | NN, likely faster per page [I] | NN | NN | none |
| Failure modes | `abort()` on internal assertions can kill the process [I: reviewer's claim, not reproduced]; junk tokens from staves [O: visible in the output of the synthetic page] | proprietary, opaque; model updates only via library upgrade | wrong characters on every diacritic | integration complexity | library stays unsearchable until a desktop run |
| Licence | Apache-2.0 (Tesseract, tessdata, cpu_features), BSD-2-Clause (Leptonica) [O: licence files] | ML Kit Terms of Service, proprietary [D: POM `licenses`] | MIT OR Apache-2.0 [D: crates.io] | Apache-2.0 [D] | n/a |

Conclusion: A is the only option that satisfies every constraint on current evidence. B is excluded by telemetry and packaging. C1 is excluded by language. C2 is the revisit candidate (exit criteria in the ADR). D is kept as an accelerator: a sidecar still wins on precedence.

A cheaper variant of A was found and not chosen: the `xberg-tesseract` 1.3.x crate (MIT, third-party fork of `tesseract-rs`) builds Tesseract 5.5.3 and Leptonica 1.87.0 from SHA-256-pinned sources through CMake and has Android handling (`build.rs`: NDK ABI, compiler, `ANDROID_PLATFORM`) [D, not run]. It brings a different Rust API than the `tesseract` crate `notae-ocr` uses, a build that downloads sources, and a young maintainer. Keep it as the fallback if the script approach proves costly.

## 3. Spike log: what was actually run

All work is under `/tmp/ocr-spike` (not in the repo). Host: Fedora 44, 16 cores, NDK r26d, CMake 4.3, Rust 1.98.1.

1. **Tesseract/Leptonica for Android.** [O]
   - Downloaded Tesseract 5.5.3, Leptonica 1.87.0, google/cpu_features 0.10.1 (GitHub tag archives) and `tessdata_fast` `pol`/`eng` at commit `87416418`. SHA-256 of the exact files:
     - Tesseract 5.5.3 tarball `9218e62793116d42a9f6d14cd9348518b27f382096eea3d0f2d1a24616bb5884`
     - Leptonica 1.87.0 tarball `fa2b40c5caea96d1bb93a97486262aed8731b69ce25a84a6bf5d25323e33f631`
     - cpu_features v0.10.1 tarball `52639b380fced11d738f8b151dbfee63fb94957731d07f1966c812e5b90cbad4`
     - `pol.traineddata` `c4476cdbc0e33d898d32345122b7be1cbf85ace15f920f06c7714756e1ef79b2`
     - `eng.traineddata` `7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2` (equal to the value `xberg-tesseract` pins for the same file)
     - GitHub regenerates tag archives, so the real script should pin a vendored or release-asset copy rather than trust these hashes forever [I].
   - Leptonica: `cmake -DCMAKE_TOOLCHAIN_FILE=$NDK/build/cmake/android.toolchain.cmake -DANDROID_ABI=arm64-v8a -DANDROID_PLATFORM=android-23 -DBUILD_SHARED_LIBS=OFF -DSW_BUILD=OFF` with PNG/JPEG/TIFF/GIF/WebP/OpenJPEG/zlib disabled: built in 9.6 s.
   - Tesseract first failed at configure: `find_package(CpuFeaturesNdkCompat REQUIRED)` (it requires google/cpu_features when `ANDROID` is set). After building and installing cpu_features, it failed again because the NDK toolchain restricts `find_package` to the NDK sysroot (fixed with `-DCMAKE_FIND_ROOT_PATH=<prefix>`), and then because `try_run(LEPT_TIFF_RESULT)` cannot run when cross-compiling (fixed with `-DLEPT_TIFF_RESULT=0`). With `-DANDROID_STL=c++_static -DOPENMP_BUILD=OFF -DDISABLE_ARCHIVE=ON -DDISABLE_CURL=ON -DGRAPHICS_DISABLED=ON -DBUILD_TRAINING_TOOLS=OFF`: built in **100 s wall** (4 min 20 s CPU). No source patches.
   - Artefacts: `libtesseract.a` 96 MB and `libleptonica.a` 18 MB (static archives, build intermediates), CLI 3.4 MB stripped; build directories 248 MB + 36 MB, sources 36 MB.
2. **Rust bindings cross-compile.** [O] A scratch `cdylib` depending on `tesseract = "0.15.2"` (the version `notae-ocr` uses) built for `aarch64-linux-android` with `PKG_CONFIG_ALLOW_CROSS=1`, `PKG_CONFIG_PATH`/`PKG_CONFIG_LIBDIR` at the NDK-built prefix, `BINDGEN_EXTRA_CLANG_ARGS` (NDK sysroot, target triple) and the NDK clang as linker and `CC`/`CXX`/`AR`. Two stumbles: the installed `lept.pc` carries `Requires.private: libwebp libwebpmux` (removed), and the first link **succeeded with 110 undefined libc++ symbols** (a cdylib allows that; it would fail at `dlopen`). Adding `-lc++_static -lc++abi -lndk_compat -Wl,--exclude-libs,ALL` (and `-L<prefix>/lib`) gave a 4.5 MB `.so` (3.4 MB stripped, 1.6 MB gzip) with 0 undefined C++ symbols and only `libdl`, `libm`, `libc` NEEDED. The library was **not loaded on a device**.
3. **Runs on Android.** [O] The NDK-built arm64 `tesseract` CLI plus `pol`/`eng` fast data ran on the x86_64 API 35 emulator (ARM translation) inside `flock /tmp/notae-avd.lock`, using only `adb -s emulator-5554` and `/data/local/tmp/ocrspike`, removed afterwards (`ls /data/local/tmp` shows only `dalvik-cache`). It recognised `Pieśń żółta — Łódź` and `Muz. Józef Żmudziński`; the full text is identical (whitespace-normalised) to the host Tesseract 5.5.3 output on the same PGM input. The CLI was built without TIFF/PNG, so it prints harmless `pixReadMemTiff`/`bmfCreate` messages (debug font only) and reads only PNM/BMP.
4. **Measurements.** [O] Synthetic A4 pages (staves, noteheads, 9 text lines, 0.4° skew, blur, noise) in Polish and English at 150/200/300 dpi; host `tesseract` 5.5.3 x86_64, `OMP_THREAD_LIMIT=1`, `--psm 3`. Per page CPU seconds and peak RSS:

   | Page (dpi) | data | languages | CPU s | RSS MB | lines ≥ 90 % | diacritics |
   | --- | --- | --- | --- | --- | --- | --- |
   | pl 150 | fast | pol | 0.44 | 42 | 8/9 | 75/80 |
   | pl 150 | fast | pol+eng | 0.74 | 59 | 8/9 | 75/80 |
   | pl 150 | best | pol | 0.81 | 50 | 8/9 | 74/80 |
   | pl 150 | best | pol+eng | 1.54 | 87 | 8/9 | 73/80 |
   | pl 200 | fast | pol | 0.37 | 50 | 6/9 | 41/80 |
   | pl 300 | fast | pol | 0.65 | 67 | 6/9 | 68/80 |
   | pl 300 | best | pol+eng | 1.57 | 93 | 6/9 | 66/80 |
   | en 150 | fast | eng | 0.35 | 42 | 9/9 | n/a |
   | en 300 | fast | eng | 0.56 | 66 | 5/9 | n/a |

   Read this honestly: the fixture is synthetic and the 200/300 dpi results are non-monotonic because staff lines disturb layout analysis, so accuracy is indicative only. What is robust: `best` costs about 2x CPU and showed no gain here; `pol+eng` costs 1.4-1.7x `pol`; memory stays under 100 MB; and **a missing resolution tag is costly**: the same 150 dpi page as PGM (no DPI, Tesseract estimated 243) scored 4/9 lines and 27/80 diacritics on both the host and the arm64 build, against 8/9 and 75/80 with the PNG that carries 150 dpi.
   On the emulator under binary translation (not representative of the tablet): 1.9 s user for a 150 dpi A4 page (`pol`), 2.7 s (`pol+eng`), 12.5 s at 300 dpi, 3.3 s for an English page.
5. **ML Kit.** [O/D] Downloaded from `dl.google.com/android/maven2` and unpacked: `com.google.mlkit:text-recognition:16.0.1` (1.4 MB; Java glue + models), `text-recognition-bundled-common:17.0.0` (18 MB AAR, four ABIs; arm64 `libmlkit_google_ocr_pipeline.so` 11.1 MB), `common:18.11.0`, `vision-common:17.3.0`. The POMs pull `transport-runtime`/`transport-backend-cct`, `firebase-components`, `play-services-*`, `androidx.appcompat`. Their manifests declare `MlKitInitProvider` (a `ContentProvider`) and `MlKitComponentDiscoveryService`. `cargo-apk2` cannot express the provider and cannot take jars. Not run: no Gradle, and consuming it would defeat the telemetry constraint anyway.
6. **Not done.** No benchmark on arm64 hardware (the tablet was off limits), no test on real scans, no `dlopen` of the cross-built `.so`, no measurement of battery or temperature, no run of Paddle/ONNX.

## 4. Proposed design

### 4.1 Engine and build

- Native inputs are pinned by version and SHA-256 and built by a repository script (working name `scripts/build-android-ocr-deps.sh`) for `arm64-v8a` and, because the emulator CI job builds an x86_64 APK, `x86_64`. Output is a prefix with `lib/pkgconfig/{tesseract,lept}.pc` (stale `Requires.private` stripped), static libraries and headers.
- `.cargo/config.toml` points the target-suffixed variables (`PKG_CONFIG_PATH_aarch64_linux_android`, `PKG_CONFIG_ALLOW_CROSS_aarch64_linux_android`, `BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android`) at the prefix, so host builds and the 28-minute host quality job are untouched (they keep using the system Tesseract). A small `build.rs` in `notae-android` emits the extra link libraries (`c++_static`, `c++abi`, `ndk_compat`, `cpu_features`, `--exclude-libs,ALL`, `-z max-page-size=16384`).
- A CI step after `cargo apk2 build` asserts the `.so` is self-contained (`readelf -d` NEEDED limited to `libc`, `libm`, `libdl`, `liblog`; `nm -uD` has no `_ZNSt` or `__cxa` symbols) and the packaged manifest has no `INTERNET` permission. A script is preferred over a `cmake`-crate `build.rs` because that would re-run about 5 minutes of CPU on every clean build and run CMake inside cargo's environment.
- `tessdata_fast` `pol`/`eng` are `cargo-apk2` `assets`; first use copies them to `internal_data_path()/ocr/<tessdata-hash>/` and verifies size and hash. A new model set gets a new directory.

### 4.2 Page pipeline

`NotaeActivity` gains one method, for example `renderNotaePageGray(uri, pageIndex, dpi, maxPixels)`. It opens the SAF file descriptor, renders one page into a white-backed `ARGB_8888` bitmap sized from the page's points × dpi / 72 and capped by total pixel count, converts to 8-bit gray in Java and returns a `byte[]` with width and height (a `PdfRenderer` allows one open page at a time and is not thread-safe; the method holds the renderer open for the whole document). Rust wraps it behind a `PageRaster` trait next to `PdfPageBackend`, so host tests use a fake. Page count comes from `PdfRenderer.getPageCount()` when the document is opened, which is what makes "page P of N" truthful. Rust creates one `TessBaseApi` per worker (init loads about 9 MB of model), calls `set_source_resolution(dpi)`, PSM 3, recognises, and keeps words that contain letters and exceed a confidence threshold; the threshold is set on real scans in P0. Candidate resolution: 150-200 dpi, chosen in P0 (the desktop uses 300). On the synthetic page 150 dpi scored best (8/9 lines), 200 and 300 dpi tied on lines (6/9) with 41/80 and 68/80 diacritics, i.e. non-monotonic, so the choice needs real scans; cost scales with pixel count.

### 4.3 State model

- New `RescanPhase::RecognizingText { files_done, files_total, page, pages }`. It is cancellable (every document commits atomically, so there is no half-applied state, unlike `ReadingText`). `is_cancellable()` changes accordingly.
- New events: `RecognitionStarted { files_total }`, `RecognitionPage { path, file_index, files_total, page, pages }`, `RecognitionFileSucceeded { path }`, `RecognitionFileFailed { path, reason }`, `RecognitionFinished { recognized, failed }`; cancel reuses `Canceled` after the current page.
- Sequencing (recommended, "option A"): `Completed { summary, rows, desktop_import }` keeps its contract and fires first so the scan summary appears at once. When the sidecar decision is resolved (none, applied or dismissed) the controller enqueues a `RecognizeText` worker command and the banner moves to `Rescan { RecognizingText }`. Rejected "option B": run recognition before `Completed`; that delays the scan result by minutes or hours and breaks the #18 rule that `Completed` follows the sidecar check.
- Queue = all notes with `status == OcrPending`, in stable order (path). No queue table. A cancel or an app exit leaves the remaining notes `OcrPending`, so resume is automatic.
- `ocr_attempting` crash marker per document; hash re-check before commit; pause on `onStop`/`onStart`, battery saver, battery below 20 % unless charging, thermal SEVERE or worse (API 29+). The banner reads for example "Recognising text · 3 of 120 files · page 2 of 6" with Pause and Cancel; no time estimate until three pages have been measured.
- Statuses: success with text → `Ready`, `ocr_language = "pol+eng"`; success with no usable words → `OcrIncomplete`; renderer/engine failure → `OcrIncomplete` plus a `FileFailed` line. No automatic retry; retry is an explicit user action.
- After this lands the comments that claim "Android never runs OCR" (`state.rs` 217, `ui/browse.rs` 66) and the Settings sentence must change.

### 4.4 Schema and provenance

One migration (the `schema_migrations` mechanism exists) adds `ocr_source TEXT` (`text_layer` | `sidecar` | `device`, `NULL` for rows that predate it) and `ocr_engine TEXT` (for example `tesseract-5.5.3/fast-<hash8>/psm3`). Precedence `text_layer > sidecar > device`. `apply_pdf_text_layer` writes `text_layer`; sidecar import writes `sidecar` and may replace `device` once (it no longer sees a "different text" loop because the source flips); device OCR never writes over the other two. The sidecar manifest does not need a version bump unless the maintainer wants to export `ocr_source`. No re-OCR when the engine string changes unless the user asks.

### 4.5 Interim state (#33) and API 23-34

`#33` removes the section and the badge now. The scan summary reports the number of PDFs with no extractable text (needed until P3). On API 23-34 the app does not read text layers, so the interim wording must be "text not read" for that population, otherwise the summary claims born-digital PDFs have no text. On the maintainer's API 35 tablet the plain wording is correct. When P3 lands the summary line becomes "N PDFs waiting for text recognition".

### 4.6 Background execution

- Phase 1: in-app only. A Rust thread keeps running after `onStop` until the system freezes the process, so pausing on `onStop` is explicit, not implied. Nothing survives the process being killed except the `OcrPending` set, which is the point of the queue design.
- WorkManager is `androidx` and unavailable without Gradle. A foreground `Service` (declarable in `cargo-apk2` metadata, class compiled from `java_sources`, needs a notification and a `foregroundServiceType` on target 34 and `POST_NOTIFICATIONS` on 33+) is the realistic way to continue off-screen; its permission and timeout rules under Android 14/15 must be checked when that issue is written. `JobScheduler` is a framework alternative with execution limits. Recommended: separate follow-up issue, not part of acceptance.

### 4.7 Threads and memory

One recognition worker thread by default (the library worker is already serialised against the index). Two workers would double peak memory (about 100 MB each at 300 dpi on the host) and heat; revisit after P0 numbers. Render at most one page at a time; recycle the bitmap immediately.

## 5. Test strategy

- **Host unit (no engine):** fake `OcrProvider` and fake `PageRaster` in the `ocr_adapter.rs` style: queue order; cancel between pages leaves remaining notes `OcrPending`; failure → `OcrIncomplete`; resume after restart; hash change discards the result; crash marker converts to `OcrIncomplete`; precedence matrix for `text_layer`/`sidecar`/`device`; phase and event ordering; progress never ahead of work; `Completed` still last for the scan.
- **Host with the real engine:** reuse the committed `crates/notae-ocr/tests/tessdata` (`pol`, `eng`, `lat`, `osd`) and `tests/fixtures/{polish,english}.png`; add a generated image-only PDF fixture with Polish text (extend `apps/notae-index/tests/fixtures/generate.py`) and assert the diacritic words come out through the same adapter code path, with the DPI set.
- **Link/packaging checks in CI:** the `readelf`/`nm` assertions, no `INTERNET` in the packaged manifest, APK size budget.
- **Emulator (x86_64 API 35):** the existing fixture directory `/sdcard/Download/NotaeOps` has two text PDFs and two "Scanned" PDFs. Pulled read-only and inspected [O]: each is one 930×1315.5 pt page holding a single 1240×1754 gray image (96 ppi as stored) of one English sentence, and host Tesseract reads them as `Scanned page - image only` and `Another scanned page`. They are valid English smoke fixtures but contain no Polish and no staves. Add a real image-only fixture with known Polish words, scan, wait for the banner to leave `RecognizingText`, and search for one word (and for its diacritic-free spelling, see the FTS note in `docs/android-debugging.md`). The emulator proves integration only; its timing is not evidence for the tablet. Needs an x86_64 build of the native prefix.
- **Device (Galaxy Tab S10 FE+, maintainer):** P0 spike and a final acceptance pass with about 30 real scanned pages: seconds per page, RSS, temperature, battery drain over a 50-file backlog, cancel latency.

## 6. CI impact

- Native prefix: about 2.5 minutes of wall time at 16 cores for both libraries plus cpu_features [O], about 5 minutes of CPU, so roughly 2-3 minutes on four hosted vCPUs [I]; double for the x86_64 prefix. Cached by script hash + NDK version, it costs seconds on a hit. Disk: about 0.3 GB of build output per ABI [O], small against the 16 GB `target/` the host quality job already produces; keep it out of that job.
- `cargo apk2 check` (about 5 minutes today [D: run history]) must find the prefix or the Android compile job fails; the check job restores the cache. The release job (about 9.5 minutes today) grows by the link step and the new assertions.
- `cargo deny`: the Android graph gains no new crates (`tesseract` and friends are already in the workspace graph); the native components are not seen by `cargo deny`, so notices are tracked in the About screen and `docs/`.
- Dependabot cannot update the pinned sources; the script header documents the upgrade procedure.

## 7. Risks

| Risk | Level | Mitigation |
| --- | --- | --- |
| Tablet speed unknown | high | P0 spike gates acceptance; revert trigger at about 8 s/page |
| `.so` never loaded on a device | high | P0 loads it before anything else is built |
| Tesseract/Leptonica `abort()` kills the process; repeated on every launch because the queue is `OcrPending` | high | `ocr_attempting` marker, no automatic retry |
| Notation produces junk tokens that pollute FTS and bloat `ocr_text` | medium | word-level confidence filter, letters-only, thresholds from real scans; OCR is for titles, composers and lyrics |
| libc++_static and Rust unwinding in one `.so` | medium | one unwinder (NDK libunwind); exercised in P0 and CI; `--exclude-libs,ALL` |
| `PdfRenderer` single open page, memory on large or odd MediaBox | medium | one page at a time, pixel cap, bitmap recycle |
| Background kill, app freezer, thermal throttling | medium | foreground-only phase 1, pause rules, resume via `OcrPending` |
| Stale model after an app update | low | model directory keyed by hash |
| Backup of OCR text via `allowBackup` (default on) | low-medium | recommend disabling in the same release |
| Hosted runner disk or time | low | separate cached job, no change to the host quality job |
| 4 KB vs 16 KB page size (NDK r26d default 4 KB) | low | link with `-z max-page-size=16384` now |
| `tessdata_fast` vs `best` decided on one synthetic page | low | decide in P0 on real scans |

## 8. Phased plan

Order follows the review by Claude Opus (see section 10): spike first, provenance before the worker, adapter merged with the worker, service split off.

| Phase | Content | Output / gate |
| --- | --- | --- |
| **P0. Device spike** (throwaway, maintainer-run or explicitly approved) | Load the cross-built library on the tablet, recognise about 30 real scanned pages; compare PSM 3 and sparse modes, `pol` vs `pol+eng`, `fast` vs `best`, dpi 150/200/300; record seconds per page, RSS, temperature, garbage-token rate, a confidence threshold. | Numbers appended to this document; ADR acceptance depends on them. |
| **P1. Build infrastructure** | Script, pinned sources, cache, x86_64 + arm64, link and manifest assertions, developer docs, no behaviour change. | CI green; `.so` assertions pass; APK size delta recorded. |
| **P2. Provenance schema** | Migration `ocr_source`/`ocr_engine`; sidecar precedence; desktop sidecar import writes `sidecar`; text-layer writes `text_layer`. Useful even before OCR. | Host tests for the precedence matrix. |
| **P3. Adapter + worker** | Android `OcrProvider`, Java page renderer, tessdata installer, `RecognizingText` phase and events, pause/cancel/resume, crash marker, hash re-check, `onStop` pause, setting, banner, scan summary line, About notices. | Host tests; emulator fixture search passes; tablet acceptance pass. |
| **P4. Docs and release** | Update `android-debugging.md`, the design spec and code comments; release notes; APK size check. | Release candidate. |
| **P5. Foreground service** (separate issue) | Off-screen continuation, notification, service type decision. | Only if the maintainer asks. |

`#33` ships ahead of all of this. P1 and P2 can run in parallel after P0.

## 9. Open questions for the maintainer

1. Accept Tesseract on-device and about 6 MB of APK growth?
2. Automatic and foreground-only with a setting (recommended), or opt-in?
3. Who runs P0 on the tablet (the tablet holds user library data and was not touched)?
4. Disable `allowBackup` in the same release?
5. Is a foreground service wanted at all?
6. Should the sidecar manifest export `ocr_source` (schema bump) or stay as is?

## 10. Review notes

An independent review by Claude Opus (`claude-opus-5-5`, high effort) was asked to attack the draft. It agreed with Tesseract and changed the plan in these ways, all adopted above: device spike moved to the front of the plan and made a gate; provenance schema before the worker; adapter merged with the worker; foreground service split into its own issue; crash marker (`abort()` on a bad page would loop forever on resume); word-level confidence filtering instead of plain `GetUTF8Text`; explicit `onStop` pause; CI assertions on `NEEDED`/undefined symbols and on the absent `INTERNET` permission; `-z max-page-size=16384`; pixel-count cap; hash re-check before commit; versioned tessdata directory; precedence `text_layer > sidecar > device`; `ocr_engine` field; `allowBackup`; a script instead of a `cmake` `build.rs`; budgets with revert triggers; and a quality statement that OCR serves titles, composers and lyrics, not notation. Its suggestion to compare `pol` alone with `pol+eng` was added to P0 (the measured cost of `pol+eng` is 1.4-1.7x). The review's factual claims about Tesseract aborting and the Android thermal API (API 29) are from the reviewer, not verified by the author.

## 11. Sources

- Tesseract 5.5.3, Apache-2.0: github.com/tesseract-ocr/tesseract; Leptonica 1.87.0, BSD-2-Clause: github.com/DanBloomberg/leptonica; google/cpu_features 0.10.1, Apache-2.0: github.com/google/cpu_features; `tessdata_fast` (commit `87416418`), Apache-2.0: github.com/tesseract-ocr/tessdata_fast.
- Crates (registry sources read): `tesseract` 0.15.2, `tesseract-sys` 0.6.3, `leptonica-sys` 0.4.9, `tesseract-plumbing` 0.11.1 (MIT); `tesseract-rs` 0.4.0 and `xberg-tesseract` 1.3.7 (MIT); `cargo-apk2` 1.4.0, `ndk-build2` 1.4.2; `ocrs` 0.13.1 (MIT OR Apache-2.0); `ort` 2.0.0-rc.14 (MIT OR Apache-2.0); `paddle-ocr-rs` 0.6.1 (Apache-2.0).
- ML Kit: developers.google.com/ml-kit/vision/text-recognition/v2/android (bundled vs unbundled, about 4 MB per script per architecture bundled), .../v2/languages (Polish under "Supported languages"), developers.google.com/ml-kit/android-data-disclosure (collected data; last updated 2026-07-15); Maven `dl.google.com/android/maven2` (`com.google.mlkit:text-recognition:16.0.1`, `text-recognition-bundled-common:17.0.0`, `common:18.11.0`, `vision-common:17.3.0`), licence "ML Kit Terms of Service" in the POM.
- ocrs: github.com/robertknight/ocrs (`ocrs/src/lib.rs` `DEFAULT_ALPHABET`, README); model sizes from `ocrs-models.s3-accelerate.amazonaws.com` `Content-Length`.
