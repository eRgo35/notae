# Task 2 report — Compact Settings and welcome (#8, #9, #11, #12)

## Implementation

- Welcome now groups the intro and feature card with a 12 px gap, caps tall-window top padding at 64 px, and keeps the setup/recovery content in its scroll view above the fixed action.
- Settings uses 16 px card padding, displays the folder name and compact **Change** action in one wrapping row, removes pending-text prose and the separate privacy card, and leaves only Back/title in the header. The maintenance card contains the indexed count, actual scan reconciliation counts (including conflicts/failures), and a body **Scan** wired to the existing `rescan_clicked` path. No history displays **No scan this session**.
- The indexed count is retained separately from filtered search rows. It initializes from unfiltered rows, is preserved when query-filtered rows arrive, and is adjusted by scan additions/deletions.
- About displays package version, checked Git short commit identity (`unknown` when Git metadata is unavailable), MIT License, `https://github.com/eRgo35/notae`, the schema constant `notae_index::SIDECAR_SCHEMA_VERSION`, and one local-only privacy line. Build script reruns on HEAD, the active ref and packed refs.
- `docs/android-debugging.md` now describes the updated Settings and marks tablet visual acceptance as pending.

## Generated UI/headless checks

Used the existing `tests/support/mod.rs` test backend and `ElementHandle` against generated `AppWindow` instances. Exercised setup/recovery actions at 360×640 and short landscape 800×360; Settings at 360×600 with a long folder name; and the Settings action surface through compact and expanded initialized windows. The generated UI tests confirm one accessible Settings maintenance Scan (and no extra header Scan), reachable Change/Back, absent and populated scan summaries, and invocation of the existing rescan callback. The indexed-count regression test confirms filtered Search rows do not replace the full count.

No rendered screenshot was captured and no device visual check was performed. In particular, generated element interaction did not verify light/dark raster output, large-text wrapping, contrast ratios, or pixel-level clipping. No Samsung tablet is attached per the available SDK ADB status; physical tablet acceptance remains pending and is not claimed.

## Verification outputs

Commands were run separately as requested:

- `cargo test -p notae-android --test headers_ui` — **6 passed, 0 failed**.
- `cargo test -p notae-android --test smoke` — **8 passed, 0 failed**.
- `cargo test -p notae-android --test storage_state` — **9 passed, 0 failed**.
- `cargo test -p notae-android --test operation_state` — **11 passed, 0 failed**.
- `rustfmt --edition 2024 --check apps/notae-android/build.rs apps/notae-android/src/android_main.rs apps/notae-android/src/ui.rs apps/notae-android/tests/headers_ui.rs apps/notae-android/tests/operation_state.rs` — **passed**.
- `git diff --check` — **passed**.

`cargo fmt --all -- --check` was also attempted; it reported formatting differences in the pre-existing, untouched `apps/notae-android/tests/support/mod.rs` import order as well as formatting in changed files. Changed Rust files were formatted with `rustfmt` and the scoped check above passes. No source-text assertion tests were added.

## Commit

Signed commit **not created**. Two attempts using normal signing failed with:

```text
error: gpg failed to sign the data:
[GNUPG:] KEY_CONSIDERED E52C8772910037991DACDC9C9745B9E27E2B1ABB 0
[GNUPG:] BEGIN_SIGNING H10
[GNUPG:] USERID_HINT 0E720C34E813F133 Michał Czyż <mike@c2yz.com>
[GNUPG:] NEED_PASSPHRASE 0E720C34E813F133 9745B9E27E2B1ABB 1 0
[GNUPG:] INQUIRE_MAXLEN 100
gpg: cannot open '/dev/tty': No such device or address

fatal: failed to write commit object

pty requested but unavailable in this environment; ran without a terminal
```

The configured `pinentry-program` is `/usr/bin/pinentry-qt` and was reloaded before the second attempt; the GUI prompt did not appear. `commit.gpgsign` remains enabled; no unsigned commit was created. Changes remain staged for signing in a terminal-capable session.

## Round 1 review fix — generated geometry and scrolling

Extended `tests/headers_ui.rs` with generated-element bounds and interaction assertions; no UI implementation change was needed:

- Settings at 360×600 checks actual folder-name and Change bounds: their vertical ranges overlap on the same wrapped row, and Change remains inside the viewport. Card and child bounds verify 16 px horizontal card padding on both sides.
- Welcome and recovery actions at 360×640 and 800×360 are checked against viewport bounds.
- Short landscape Welcome checks the generated ScrollView's actual interaction: a -1000 px scroll-wheel event moves the generated Welcome title, while the fixed Choose action remains inside 800×360. The UI still uses its specified 12 px paragraph-to-card gap, but the paragraph is not exposed as an accessible generated element for direct bounds querying; this round does not claim an exact 12 px geometry assertion.

TDD evidence: the first short-landscape draft failed because the feature heading had no accessible label; a drag gesture also left the title at y=148. Reworked the check to use the accessible Welcome title and the test backend's scroll-wheel interaction, which moved its generated bounds. Final verification:

```text
$ cargo test -p notae-android --test headers_ui
running 7 tests
test short_landscape_welcome_scrolls_its_content_without_clipping_the_action ... ok
test independently_initialized_threads_own_their_generated_ui ... ok
test maintenance_scan_is_available_when_settings_is_open ... ok
test settings_actions_stay_in_the_screen_and_expose_folder_change ... ok
test settings_shows_absent_and_real_scan_results_and_runs_maintenance_scan ... ok
test settings_is_reachable_and_exposes_an_accessible_header_action ... ok
test welcome_and_recovery_actions_remain_reachable_in_compact_and_short_landscape ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

These are generated headless geometry/interaction checks only. They do not substitute for tablet light/dark/large-text visual acceptance, which remains pending; no physical device is attached. The deferred P3 projection/comment is unchanged.

Signed commit:

```text
$ git commit -m "test(android): assert compact UI geometry and scrolling"
[fix/v0.2.0-tablet-feedback 290a818] test(android): assert compact UI geometry and scrolling
 2 files changed, 151 insertions(+), 20 deletions(-)
pty requested but unavailable in this environment; ran without a terminal

$ git verify-commit HEAD
gpg: Signature made Fri 16 Oct 2026 09:28:30 PM CEST
gpg:                using RSA key 74A614A5722F92C7C769F7E50E720C34E813F133
gpg: Good signature from "Michał Czyż <mike@c2yz.com>" [ultimate]
```