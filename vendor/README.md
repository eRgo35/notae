# Vendored third-party code

Everything under `vendor/` is third-party source that Notae carries unmodified except for the
patches listed here. **The repository's MIT licence does not apply to `vendor/`.** Each vendored
crate keeps the licence it was published under.

## `i-slint-backend-android-activity` 1.18.1 + upstream PR #13112

| | |
|---|---|
| Upstream crate | [`i-slint-backend-android-activity` 1.18.1](https://crates.io/crates/i-slint-backend-android-activity/1.18.1) (Slint's Android backend; `path_in_vcs = internal/backends/android-activity`, upstream `372cf0ee5577c3dfec309a45e7b778ba4e81b734`) |
| `.crate` SHA-256 | `70d538003a566d7312572ac1435be0704934e46a63d55abdabede513f72332cb` (the checksum Cargo.lock held for the registry copy) |
| Applied patch | [slint-ui/slint#13112](https://github.com/slint-ui/slint/pull/13112), commit [`3a7e700`](https://github.com/slint-ui/slint/commit/3a7e700487dc74d404cad0726378053ab763d4e0) "android: honor keyboard layout for physical-keyboard text input", merged to Slint master on 2026-09-22 |
| Patch file | [`patches/i-slint-backend-android-activity-3a7e700.patch`](patches/i-slint-backend-android-activity-3a7e700.patch), the upstream diff unchanged (one file, +84/-12) |
| Licence of the vendored code | the crate's own SPDX expression, unchanged: `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0` (texts in `i-slint-backend-android-activity/LICENSES/`). Notae continues to use it under the same terms as the registry copy. Not relicensed. |

### Why it exists

Issue [#32](https://github.com/eRgo35/notae/issues/32): Polish letters cannot be typed from a
hardware keyboard. Slint 1.18.1's Android backend turns a hardware `KeyEvent` into text from the
key code alone (`map_key_code` in `androidwindowadapter.rs`, a lowercase ASCII table). Meta state
(Shift, AltGr) and the device `KeyCharacterMap` are never consulted, so AltGr+A types `a` and
Shift+A types `a`. The patch asks `KeyCharacterMap::get(key_code, meta_state)` for the character,
composes dead keys with `get_dead_char`, and falls back to the old table for keys that produce no
Unicode (arrows, function keys, modifiers, Back, Ctrl combinations). No Slint release contains it
(the latest, 1.18.1, predates the merge).

### What differs from the published crate

- `androidwindowadapter.rs` has the patch applied (`patch -p4 < patches/...patch` run inside the
  crate directory reproduces it from the published file).
- The published `Cargo.lock` of the crate is omitted. It is a build artefact of Slint's own
  workspace and is not used when the crate is a path dependency.
- Everything else, including `Cargo.toml` (version `1.18.1`, so Cargo resolves it for Slint's
  `=1.18.1` requirements) and the licence files, is byte-identical to the published `.crate`.

To re-check: download the `.crate` (`cargo download` or `~/.cargo/registry/cache/`), extract it,
and `diff -r` against this directory. Only `androidwindowadapter.rs` and the missing
`Cargo.lock` should differ.

### How it is wired

`[patch.crates-io]` in the workspace `Cargo.toml` points at this directory. It is not a workspace
member, so `cargo fmt`, `cargo clippy --workspace` and `cargo test --workspace` do not touch it.
`Cargo.lock` therefore lists the package without a `source`/`checksum` (a path package); builds
with `--locked` keep working.

### Removal condition

Drop it as soon as a **released** Slint contains PR #13112 (expected 1.19; a 1.18.2 backport would
also do). Then:

1. Upgrade the `slint` dependency to that release.
2. Delete the `[patch.crates-io]` entry from the workspace `Cargo.toml`.
3. Delete `vendor/i-slint-backend-android-activity/`, `vendor/patches/i-slint-backend-android-activity-3a7e700.patch` and this section (and `vendor/` if it is then empty).
4. Run `cargo update -p slint` as needed, so `Cargo.lock` points back at the registry, then re-run the checks.
5. Repeat the physical-keyboard check from `docs/android-debugging.md` on the new release.

If the upgrade ships the fix but a later Slint release changes the same function, do not carry
this patch across versions; it only applies to 1.18.1.
