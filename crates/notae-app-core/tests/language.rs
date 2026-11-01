//! The language choice and its Android tags.

use notae_app_core::ui_prefs::{LanguageChoice, UiPrefs};

#[test]
fn android_locale_tags_map_to_choices() {
    assert_eq!(LanguageChoice::from_tag(""), LanguageChoice::System);
    assert_eq!(LanguageChoice::from_tag("en"), LanguageChoice::English);
    assert_eq!(LanguageChoice::from_tag("en-US"), LanguageChoice::English);
    assert_eq!(LanguageChoice::from_tag("pl-PL"), LanguageChoice::Polish);
    assert_eq!(LanguageChoice::from_tag("fr"), LanguageChoice::System);
    for choice in [
        LanguageChoice::System,
        LanguageChoice::English,
        LanguageChoice::Polish,
    ] {
        assert_eq!(LanguageChoice::from_tag(choice.tag()), choice);
    }
}

#[test]
fn settings_indices_round_trip() {
    for choice in [
        LanguageChoice::System,
        LanguageChoice::English,
        LanguageChoice::Polish,
    ] {
        assert_eq!(LanguageChoice::from_index(choice.index()), choice);
    }
    assert_eq!(LanguageChoice::from_index(7), LanguageChoice::System);
}

#[test]
fn prefs_ignore_the_language_line() {
    // AppCompat keeps the per-app language (spec §4.3, ruling R5): a line from an older
    // build is ignored, and none is written.
    let prefs = UiPrefs::from_text("library_view=list\nlanguage=pl\n");
    assert_eq!(prefs, UiPrefs::from_text("library_view=list\n"));
    assert!(!prefs.to_text().contains("language"), "{}", prefs.to_text());
}
