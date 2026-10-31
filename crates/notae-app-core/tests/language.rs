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
fn prefs_keep_the_language_choice() {
    assert_eq!(UiPrefs::default().language, LanguageChoice::System);
    let prefs = UiPrefs::from_text("language=pl\n");
    assert_eq!(prefs.language, LanguageChoice::Polish);
    assert_eq!(UiPrefs::from_text(&prefs.to_text()), prefs);
    assert_eq!(
        UiPrefs::from_text("language=klingon\n").language,
        LanguageChoice::System
    );
}
