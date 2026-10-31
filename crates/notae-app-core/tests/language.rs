//! The language choice, its Android tags and the bundled translation it selects.

use notae_app_core::language::{LanguageChoice, bundle_for};
use notae_app_core::ui_prefs::UiPrefs;

#[test]
fn system_choice_follows_a_polish_system_language() {
    assert_eq!(bundle_for(LanguageChoice::System, "pl-PL"), "pl");
    assert_eq!(bundle_for(LanguageChoice::System, "pl"), "pl");
}

#[test]
fn system_choice_falls_back_to_english_for_other_languages() {
    assert_eq!(bundle_for(LanguageChoice::System, "en-GB"), "");
    assert_eq!(bundle_for(LanguageChoice::System, "de-DE"), "");
    assert_eq!(bundle_for(LanguageChoice::System, ""), "");
    assert_eq!(bundle_for(LanguageChoice::System, "plx"), "");
}

#[test]
fn an_explicit_choice_ignores_the_system_language() {
    assert_eq!(bundle_for(LanguageChoice::English, "pl-PL"), "");
    assert_eq!(bundle_for(LanguageChoice::Polish, "en-US"), "pl");
}

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
