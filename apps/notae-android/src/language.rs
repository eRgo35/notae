//! Which bundled translation the UI shows.
//!
//! English is the source language; Polish is the only bundled translation.
//! Any other system language falls back to English.

#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicBool, Ordering};

/// Whether text built in Rust is Polish. Slint text follows `select_bundled_translation`;
/// this mirrors it for messages Rust formats, including on the scan worker thread.
static POLISH: AtomicBool = AtomicBool::new(false);

/// Records the selected Slint bundle so Rust-built text matches it.
pub fn set_active_bundle(bundle: &str) {
    POLISH.store(bundle == "pl", Ordering::Relaxed);
}

/// The English or the Polish form of a message built in Rust, for the active language.
///
/// UI text in `.slint` files goes through `@tr()` instead; only text Rust formats lives here.
pub fn pick<T>(english: T, polish: T) -> T {
    if POLISH.load(Ordering::Relaxed) {
        polish
    } else {
        english
    }
}

/// The Polish plural form for `count`: `one` for 1, `few` for 2–4 (but not 12–14), `many` otherwise.
#[must_use]
pub const fn polish_plural<'a>(count: usize, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    let tens = count % 100;
    if count == 1 {
        one
    } else if count % 10 >= 2 && count % 10 <= 4 && (tens < 10 || tens >= 20) {
        few
    } else {
        many
    }
}

/// The language picked in Settings, or in Android's per-app language setting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LanguageChoice {
    /// Follow the system language.
    #[default]
    System,
    /// English, whatever the system language.
    English,
    /// Polish, whatever the system language.
    Polish,
}

impl LanguageChoice {
    /// The BCP 47 tag Android stores for this choice; empty for [`Self::System`].
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::System => "",
            Self::English => "en",
            Self::Polish => "pl",
        }
    }

    /// Reads an Android locale tag. Unsupported languages mean "follow the system".
    #[must_use]
    pub fn from_tag(tag: &str) -> Self {
        match primary_language(tag) {
            "en" => Self::English,
            "pl" => Self::Polish,
            _ => Self::System,
        }
    }

    /// Position in the Settings list.
    #[must_use]
    pub const fn index(self) -> i32 {
        match self {
            Self::System => 0,
            Self::English => 1,
            Self::Polish => 2,
        }
    }

    /// Reads a Settings list position; unknown positions mean "follow the system".
    #[must_use]
    pub const fn from_index(index: i32) -> Self {
        match index {
            1 => Self::English,
            2 => Self::Polish,
            _ => Self::System,
        }
    }
}

/// The Slint bundle name to select: `"pl"` for Polish, `""` for English.
#[must_use]
pub fn bundle_for(choice: LanguageChoice, system_tag: &str) -> &'static str {
    let choice = match choice {
        LanguageChoice::System => LanguageChoice::from_tag(system_tag),
        explicit => explicit,
    };
    match choice {
        LanguageChoice::Polish => "pl",
        LanguageChoice::System | LanguageChoice::English => "",
    }
}

fn primary_language(tag: &str) -> &str {
    tag.split(['-', '_']).next().unwrap_or_default()
}
