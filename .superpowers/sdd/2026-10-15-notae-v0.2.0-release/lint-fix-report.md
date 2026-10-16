# Generated Slint Clippy lint-boundary fix

## Change

The production Clippy gate started linting the host-included Slint-generated `OUT_DIR/app.rs`, which contains framework-generated `unwrap()` and `expect()` calls. An `#[allow]` attached directly to `include!` is ignored by Rust, so the first attempted boundary produced `unused attribute` under `-D warnings`.

The include is now contained by a private `generated` module carrying only `#[allow(clippy::unwrap_used, clippy::expect_used)]`. `pub use generated::*` re-exports the generated bindings at the crate root, preserving the existing public `AppWindow` API and all other public generated items. The lint policy remains strict for handwritten production code, tests retain their existing lint behavior, and no UI behavior changed.

## Verification

- `cargo clippy --workspace --all-targets -- -D warnings -A clippy::unwrap_used -A clippy::expect_used`

  ```text
      Checking notae-android v0.1.0 (/home/mike/Work/notae-v0.2.0/apps/notae-android)
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.07s
  ```

- `cargo clippy --workspace --lib --bins -- -D warnings -D clippy::unwrap_used -D clippy::expect_used`

  ```text
      Checking notae-domain v0.1.0 (/home/mike/Work/notae-v0.2.0/crates/notae-domain)
     Compiling notae-ocr v0.1.0 (/home/mike/Work/notae-v0.2.0/crates/notae-ocr)
     Compiling notae-android v0.1.0 (/home/mike/Work/notae-v0.2.0/apps/notae-android)
      Checking notae-index v0.1.0 (/home/mike/Work/notae-v0.2.0/crates/notae-index)
      Checking notae-index-cli v0.1.0 (/home/mike/Work/notae-v0.2.0/apps/notae-index)
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.70s
  ```

- `cargo fmt --all -- --check`

  ```text
  (no output; exit status 0)
  ```

- `cargo test -p notae-android --test headers_ui`

  ```text
      Compiling notae-android v0.1.0 (/home/mike/Work/notae-v0.2.0/apps/notae-android)
      Finished `test` profile [unoptimized + debuginfo] target(s) in 18.68s
       Running tests/headers_ui.rs (target/debug/deps/headers_ui-b33d4401d4d31d19)

  running 8 tests
  test short_landscape_welcome_scrolls_its_content_without_clipping_the_action ... ok
  test maintenance_scan_is_available_when_settings_is_open ... ok
  test settings_shows_absent_and_real_scan_results_and_runs_maintenance_scan ... ok
  test settings_actions_stay_in_the_screen_and_expose_folder_change ... ok
  test independently_initialized_threads_own_their_generated_ui ... ok
  test settings_is_reachable_and_exposes_an_accessible_header_action ... ok
  test search_header_has_no_scan_and_keyboard_hint_is_contextual ... ok
  test welcome_and_recovery_actions_remain_reachable_in_compact_and_short_landscape ... ok

  test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.43s
  ```

- `ANDROID_NDK_ROOT=/home/mike/Android/Sdk/ndk/android-ndk-r26d cargo apk2 check -p notae-android --target aarch64-linux-android`

  ```text
  Using package `notae-android` in `/home/mike/Work/notae-v0.2.0/apps/notae-android/Cargo.toml`
     Compiling notae-android v0.1.0 (/home/mike/Work/notae-v0.2.0/apps/notae-android)
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.04s
  ```

The module re-export and `AppWindow` public use compile in the host `headers_ui` integration test and in the Android arm64 check. The all-targets and production Clippy gates both pass; handwritten production unwrap/expect lints remain denied.
