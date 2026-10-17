//! Motion policy shared by the Android adapter and host regression tests.

/// Android's zero animator scale requests reduced motion; unknown or non-zero scales keep motion enabled.
pub const fn animations_enabled_for_scale(scale: f32) -> bool {
    scale != 0.0
}
