//! UniFFI's binding generator, built from the same `uniffi` version as the library's scaffolding.
//!
//! `cargo run -p notae-app-core --bin uniffi-bindgen generate --library <libnotae_app_core.so>
//! --language kotlin --out-dir <dir>` writes the Kotlin bindings.

fn main() {
    uniffi::uniffi_bindgen_main();
}
