//! Text built in Rust follows the selected language, and the bundled translation a choice selects.
//!
//! The active language is process-wide, so these tests live in their own test binary and the
//! ones that read or change it take turns.

use std::sync::{Mutex, MutexGuard, PoisonError};

use notae_android::language::{bundle_for, polish_plural, set_active_bundle};
use notae_android::texts;
use notae_app_core::operations::events::RescanSummary;
use notae_app_core::state::{OperationState, RescanPhase};
use notae_app_core::text::{RowStatus, SetSubtitle};
use notae_app_core::ui::OperationBanner;
use notae_app_core::ui::browse::{Neighbours, edited};
use notae_app_core::ui_prefs::LanguageChoice;

/// The active language is process-wide; tests that read or switch it take turns.
static LANGUAGE: Mutex<()> = Mutex::new(());

fn language() -> MutexGuard<'static, ()> {
    LANGUAGE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn banner(operation: OperationState) -> String {
    OperationBanner::project(&operation)
        .message
        .as_ref()
        .map_or_else(String::new, texts::status_message)
}

#[test]
fn rust_built_messages_switch_between_english_and_polish() {
    let _language = language();
    set_active_bundle("");
    let scanning = || {
        banner(OperationState::Rescan {
            phase: RescanPhase::Enumerating,
        })
    };
    let reading = || {
        banner(OperationState::Rescan {
            phase: RescanPhase::ReadingText {
                completed: 3,
                total: 7,
            },
        })
    };

    let place = Neighbours {
        previous: None,
        next: None,
        position: 2,
        total: 5,
    };
    assert_eq!(scanning(), "Scanning library…");
    assert_eq!(texts::position_of(place.position_of()), "2 of 5");
    assert_eq!(texts::add_picker_footer(3), "3 added at the end of the set");
    set_active_bundle("pl");
    assert_eq!(texts::position_of(place.position_of()), "2 z 5");
    for (count, expected) in [
        (1, "Dodano 1 utwór na końcu zestawu"),
        (3, "Dodano 3 utwory na końcu zestawu"),
        (5, "Dodano 5 utworów na końcu zestawu"),
    ] {
        assert_eq!(texts::add_picker_footer(count), expected);
    }
    assert_eq!(scanning(), "Skanowanie biblioteki…");
    assert_eq!(reading(), "Odczyt tekstu i miniatur PDF: 3 z 7");
    set_active_bundle("");
    assert_eq!(reading(), "Reading PDF text and thumbnails: 3 of 7");
}

#[test]
fn banner_text_follows_each_phase_and_the_scan_summary() {
    let _language = language();
    set_active_bundle("");
    let indexing = |total| {
        banner(OperationState::Rescan {
            phase: RescanPhase::Indexing { total },
        })
    };
    let finished = |summary| banner(OperationState::Completed { summary });
    assert_eq!(banner(OperationState::Idle), "");
    assert_eq!(indexing(1), "Indexing 1 PDF…");
    assert_eq!(indexing(3), "Indexing 3 PDFs…");
    assert_eq!(
        banner(OperationState::Rescan {
            phase: RescanPhase::ReadingText {
                completed: 0,
                total: 3,
            },
        }),
        "Reading PDF text and thumbnails: 0 of 3"
    );
    assert_eq!(
        finished(RescanSummary {
            added: 1,
            ..RescanSummary::default()
        }),
        "Finished: 1 added"
    );
    let summary = RescanSummary {
        added: 2,
        conflicts: 1,
        failures: 1,
        ..RescanSummary::default()
    };
    assert_eq!(
        finished(summary),
        "Finished: 2 added, 1 ID conflicts, 1 failures"
    );
    assert_eq!(finished(RescanSummary::default()), "Finished: No changes");
    assert_eq!(
        banner(OperationState::Failed {
            reason: "boom".to_owned(),
        }),
        "boom"
    );

    set_active_bundle("pl");
    assert_eq!(indexing(1), "Indeksowanie 1 pliku PDF…");
    assert_eq!(indexing(3), "Indeksowanie 3 plików PDF…");
    assert_eq!(
        finished(summary),
        "Zakończono: dodano: 2, konflikty numerów: 1, błędy: 1"
    );
    assert_eq!(finished(RescanSummary::default()), "Zakończono: Bez zmian");
    set_active_bundle("");
}

#[test]
fn polish_plural_picks_one_few_or_many() {
    let form = |count| polish_plural(count, "utwór", "utwory", "utworów");
    for (count, expected) in [
        (0, "utworów"),
        (1, "utwór"),
        (2, "utwory"),
        (4, "utwory"),
        (5, "utworów"),
        (12, "utworów"),
        (14, "utworów"),
        (21, "utworów"),
        (22, "utwory"),
        (104, "utwory"),
        (112, "utworów"),
    ] {
        assert_eq!(form(count), expected, "{count}");
    }
}

const NOV_1_2026: u64 = 1_793_491_200;
const DAY: u64 = 86_400;

#[test]
fn format_edited_buckets() {
    let _language = language();
    let format_edited = |modified, now| texts::edited(&edited(modified, now));
    let now = NOV_1_2026 + 30 * DAY + 12 * 3600;
    let today = now - 3600;
    let yesterday = now - DAY;
    let three_days = now - 3 * DAY;
    let long_ago = NOV_1_2026 + 600;

    set_active_bundle("");
    assert_eq!(format_edited(today, now), "Edited today");
    assert_eq!(format_edited(yesterday, now), "Edited yesterday");
    assert_eq!(format_edited(three_days, now), "Edited 3 days ago");
    assert_eq!(format_edited(long_ago, now), "Edited 1 Nov 2026");
    // A clock that went backwards is not "in the future".
    assert_eq!(format_edited(now + DAY, now), "Edited today");

    set_active_bundle("pl");
    assert_eq!(format_edited(today, now), "Edytowano dziś");
    assert_eq!(format_edited(yesterday, now), "Edytowano wczoraj");
    assert_eq!(format_edited(three_days, now), "Edytowano 3 dni temu");
    assert_eq!(format_edited(long_ago, now), "Edytowano 1 lis 2026");
    set_active_bundle("");
}

#[test]
fn format_set_date_en_and_pl() {
    let _language = language();
    set_active_bundle("");
    assert_eq!(texts::format_set_date("2026-11-01"), "1 Nov 2026");
    assert_eq!(texts::format_set_date("2027-05-24"), "24 May 2027");
    set_active_bundle("pl");
    assert_eq!(texts::format_set_date("2026-11-01"), "1 lis 2026");
    assert_eq!(texts::format_set_date("2027-05-24"), "24 maj 2027");
    set_active_bundle("");
    // Anything that is not a stored ISO date is shown as written.
    assert_eq!(texts::format_set_date("soon"), "soon");
}

#[test]
fn missing_piece_status_en_and_pl() {
    let _language = language();
    set_active_bundle("");
    assert_eq!(
        texts::row_status(RowStatus::MissingFromLibrary),
        "Missing from library"
    );
    set_active_bundle("pl");
    assert_eq!(
        texts::row_status(RowStatus::MissingFromLibrary),
        "Brak w bibliotece"
    );
    set_active_bundle("");
}

#[test]
fn set_subtitle_joins_date_and_counts() {
    let _language = language();
    let subtitle = |date: Option<&str>, pieces, notes| {
        texts::set_subtitle(&SetSubtitle {
            date: date.map(str::to_owned),
            pieces,
            notes,
        })
    };
    set_active_bundle("");
    assert_eq!(
        subtitle(Some("2026-11-01"), 5, 1),
        "1 Nov 2026 · 5 pieces · 1 note"
    );
    set_active_bundle("pl");
    assert_eq!(
        subtitle(Some("2026-11-01"), 5, 1),
        "1 lis 2026 · 5 utworów · 1 notatka"
    );

    // Zero counts and a missing date are left out.
    set_active_bundle("");
    assert_eq!(subtitle(None, 2, 0), "2 pieces");
    set_active_bundle("pl");
    assert_eq!(subtitle(None, 2, 0), "2 utwory");
    assert_eq!(subtitle(None, 0, 0), "");
    set_active_bundle("");
}

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
