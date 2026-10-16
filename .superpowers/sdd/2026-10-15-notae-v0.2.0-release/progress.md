# SDD ledger — plan: docs/superpowers/plans/2026-10-15-notae-v0.2.0-release.md

BASE: 41f7926afe29e65edfb104c11a7bc94b716bb4d8 (branch `fix/v0.2.0-tablet-feedback`, worktree `/home/mike/Work/notae-v0.2.0`).

## Preflight scan

| Task pair | Shared files/interface | Scan result |
|---|---|---|
| 1 ↔ 2 | build.rs, lib.rs, generated AppWindow and UI tests | Task 1 establishes host bindings/test backend before Task 2 uses behavior tests; sequence is valid. |
| 1 ↔ 3 | build.rs, lib.rs, UI tests | Task 1's harness precedes Search behavior tests; sequence is valid. |
| 1 ↔ 4 | lib.rs, UI tests | Startup/input tests consume Task 1 host seam; sequence is valid. |
| 2 ↔ 3 | app.slint, build.rs, UI tests, docs | Header contract is shared; Task 2 precedes Task 3 and both converge on C3. |
| 2 ↔ 4 | app.slint, setup/search UI, docs | Shared navigation/header projections; sequential plan order resolves ownership. |
| 3 ↔ 4 | search/app/components UI, Android activity/manifest, tests | Search/input callbacks and focus are shared; Task 3 precedes Task 4. |
| 4 ↔ 5 | app/components/search/library UI, android_main.rs | Motion wraps screen/input presentation; Task 4 establishes startup/focus before Task 5. |
| 4 ↔ 6 | app/library UI, ui.rs, android_main.rs, browse.rs | Startup and library preferences share app state; plan sequences them. |
| 5 ↔ 6 | app/components/library UI | Motion tokens affect selection/layout components; C2 is consumed by Task 6. |
| 5 ↔ 7 | app/components/details/library UI | Sheet and preview transitions share mounting/navigation; Task 5 precedes Task 7. |
| 5 ↔ 8 | components/library/details UI | Scrollbar fade consumes C2 motion tokens; Task 5 precedes Task 8. |
| 6 ↔ 7 | app/library UI, ui.rs, browse.rs, tests | Task 6 produces ScoreRow/preferences consumed by Task 7; sequence is required. |
| 7 ↔ 8 | library/details UI | Preview layout and scrollbar attachment overlap; Task 7 precedes Task 8. |
| 7 ↔ 9 | app/details/library UI, ui.rs | Shared screen mounting/status placement; sequential ownership avoids duplicate state. |
| 8 ↔ 9 | library/details UI | Scrollbars and status surfaces share scrolling/layout; plan order is safe. |
| 9 ↔ 10 | Android operation state and docs only where app display described | No production interface conflict. |
| 10 ↔ 11 | README, docs/images and icon assets | No production code overlap; release order preserves screenshots as actual evidence. |
| 0 ↔ 1–11 | CI/workflow, catalog fixture, engineering docs, plan ledger | Independent except final qualification; CI repair first is correct. |

| Task | Self-consistency scan |
|---|---|
| 0 | Existing dependency fix is incomplete: latest workflow needs trained tessdata for CLI integration tests. No gate weakening is allowed; add the real runner prerequisite. External push/run cannot be performed without separate authorization. |
| 1 | Test seam/API and planned files align; use resolved backend API and ensure test initialization remains thread-local. |
| 2 | UI work, model projections, behavior checks and docs align; maintenance values must not fabricate timestamps/counts. |
| 3 | Voice-result adapter and keyboard metadata are cross-language; task owns distinct activity result routing and tests it. |
| 4 | Issue hypotheses are not diagnoses; plan requires reproducing tablet issues and leaves device-dependent acceptance unverified absent device. |
| 5 | Motion scale mapping and mounted exit animations are specified; check modal input blocking and zero-scale semantics. |
| 6 | Preferences are separately persisted for portrait/landscape; Task 7 depends on its row callback contract. |
| 7 | Context-menu/accessibility and Recent interaction contracts are explicit; TalkBack acceptance cannot be inferred from host labels. |
| 8 | Scrollbar dragging/fade must use shared motion tokens and avoid binding loops. |
| 9 | Pending text is not Android OCR; status must report actual indexing/sidecar phases and correct completion ordering. |
| 10 | Contrast and matching adaptive/legacy/themed artwork are acceptance requirements; existing screenshot is invalid baseline. |
| 11 | README acceptance requires actual screenshots; invalid issue artifact cannot be used. |
| 12 | Release qualification depends on host gates plus real device acceptance and release artifacts; no publish/tag is authorized here. |

Ruling: Execute product implementation on the isolated branch; keep #2/#13/#17 device-dependent acceptance open until observed on the tablet — the plan mandates actual-device proof — cost if wrong: release checklist remains open and may require a follow-up verification session. Do not push, publish, tag, or close issues from this session without explicit authorization; cost if wrong: remote branch/issue state would change outside the approved local worktree.

## Progress
Task 0: local fix complete (commits 41f7926..8ccbc28, review approved); #21 remains externally open pending push, both Actions jobs passing, and successful run recorded.
Device check: SDK ADB reports no attached devices. Tablet-only acceptance for #2, #13, #17 and visual/device acceptance for the release will remain open unless a device becomes available.
Task 1: complete (commits 8ccbc28..a07dae8, review clean); Android arm64 check unverified because local NDK clang is unavailable.
User authorization: commits should be GPG-signed using `pinentry-qt` for the key passphrase; do not disable signing. `commit.gpgsign=true`, `/usr/bin/pinentry-qt` is installed, and GUI display is available in this session.
Task 2: Minor (deferred): `UiStatus.pending_text_count` calculation and a stale “lives in Settings only” comment remain despite removing that UI; final review should confirm cleanup.
Task 2: complete locally (commits a07dae8..7f46ee9, geometry/scroll review finding addressed); physical tablet light/dark/large-text acceptance remains pending. Exact 12 px paragraph-to-card spacing is implemented but not directly exposed to the headless element query.
Task 2: fix round 1/5 (1 addressed, 0 open; geometry/scroll assertions; commits 67d0822..7de67d6); signed evidence correction 7f46ee9 records the verified commit/signature.
Task 3: reviewer’s low-severity blank-first-candidate test gap addressed by `d4cb269`; focused voice tests now pass 6 cases. `344fa76` formats changed Rust sources after the workspace formatting check exposed them.
Workspace check: `cargo fmt --all -- --check` and `cargo test --workspace` pass (415 tests, 37 suites). First Clippy gate passes; production-only Clippy gate currently fails on generated `OUT_DIR/app.rs` unwrap/expect lints after host bindings were enabled. Task `ScopeGeneratedLint` is addressing the narrow generated-code lint scope.

Task 1 quality-gate follow-up: Clippy's production panic-policy gate exposed `unwrap_used`/`expect_used` diagnostics from Slint-generated `OUT_DIR/app.rs`. A lint attribute directly on `include!` is ignored by Rust (`unused attribute`), so the generated include is now isolated in a private `generated` module with only those two lint allowances; `pub use generated::*` preserves root-level public generated bindings including `AppWindow`. Handwritten production code retains the denied policy. Both documented Clippy gates, workspace fmt check, focused `headers_ui` tests, and Android arm64 check pass; see `lint-fix-report.md` for exact outputs.
Task 4 and downstream tasks 5–11: blocked pending target-device availability; no emulator command is installed. See `device-blocker.md`. Task 4 fixes are not guessed from issue hypotheses.
