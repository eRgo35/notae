# Task 0 report: Restore CI prerequisites (#21)

## Status

The local repair is implemented and committed. The GitHub Actions/push acceptance is unresolved: the required changes were not pushed, no new remote Actions run was observed, and issue #21 remains open.

## Root cause and repair

The CI host installed Tesseract development headers but not any trained language data. The CLI integration tests run the real binary and require at least one installed language pack; without one, successful-indexing cases exit non-zero. On Ubuntu, `tesseract-ocr-eng` provides the English pack under `/usr/share/tesseract-ocr/5/tessdata`, rather than the CLI's legacy fallback path. The CI job now installs that real package and sets `TESSDATA_PREFIX` to its data directory, using the supported CLI environment override. OCR and test behavior are unchanged, and no CI gate, toolchain pin, or coverage floor was removed or weakened.

The task's already-landed fixture repair renames the original directory before creating its replacement, keeping the device/inode identity refusal assertion deterministic without changing production behavior.

Engineering prerequisites, release-plan task status, and implementation-state tracking were updated. Push and successful Actions completion remain unchecked because this worker must not change remote state.

## Verification performed

Local focused suite:

```text
$ cargo test -p notae-index-cli --test exit_codes
   Compiling notae-index-cli v0.1.0 (/home/mike/Work/notae-v0.2.0/apps/notae-index)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 5.19s
     Running tests/exit_codes.rs (target/debug/deps/exit_codes-9512648ee128390d)

running 12 tests
test an_unusable_root_exits_non_zero ... ok
test an_unknown_flag_is_a_usage_error ... ok
test a_missing_root_argument_is_a_usage_error ... ok
test version_exits_successfully ... ok
test an_unreadable_root_exits_non_zero_and_names_it ... ok
test a_symlinked_root_never_indexes_the_directory_it_points_at ... ok
test help_exits_successfully ... ok
test an_unreadable_file_under_a_readable_root_still_exits_zero ... ok
test a_completed_run_with_isolated_failures_exits_zero_and_reports_them ... ok
test a_clean_run_exits_zero ... ok
test a_real_root_exits_zero_however_the_binary_is_spelled ... ok
test no_spelling_of_a_symlinked_root_is_followed_by_the_binary ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
```

Local workspace suite:

```text
$ cargo test --workspace
cargo test: 404 passed (36 suites, 0.00s)
```

The focused suite command was initially tried with the wrong package name (`-p notae-index`), which Cargo rejected and identified `notae-index-cli` as the package owning `exit_codes`; the corrected command above passed. The local host already has `/usr/share/tesseract/tessdata/eng.traineddata`, so these local runs do not independently prove the Ubuntu runner's package installation/path. CI package setup must be confirmed by a pushed Actions run.

## Files in this repair commit

- `.github/workflows/ci.yml` — install `tesseract-ocr-eng` and set the job's `TESSDATA_PREFIX`.
- `docs/engineering-standards.md` — document the English language-data prerequisite and Ubuntu path.
- `docs/superpowers/plans/2026-10-15-notae-v0.2.0-release.md` — record the newly exposed CI failure and mark locally completed work; leave remote acceptance unchecked.
- `docs/superpowers/IMPLEMENTATION-STATE.md` — record local status and outstanding remote acceptance.
- `.superpowers/sdd/2026-10-15-notae-v0.2.0-release/task-0-report.md` — this report.

The deterministic directory-replacement fixture change is already present at the assigned branch's starting commit `41f7926` (`test(index): prevent inode reuse in directory replacement fixture`).

## Outstanding concern

Ubuntu's `tesseract-ocr-eng` package and `/usr/share/tesseract-ocr/5/tessdata` path are configured, but no remote run has validated them. Do not close #21 until both Actions jobs pass and the successful run is recorded on the issue.
