//! Text built in Rust follows the selected language.
//!
//! The active language is process-wide, so these tests live in their own test binary and
//! only one of them changes it.

use notae_android::language::{polish_plural, set_active_bundle};
use notae_android::state::{OperationState, RescanPhase};
use notae_android::ui::OperationBanner;
use notae_android::ui::browse::Neighbours;

fn banner(operation: OperationState) -> String {
    OperationBanner::project(&operation).text
}

#[test]
fn rust_built_messages_switch_between_english_and_polish() {
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
    assert_eq!(place.label(), "2 of 5");
    assert_eq!(
        notae_android::ui::add_picker_footer(3),
        "3 added at the end of the set"
    );
    set_active_bundle("pl");
    assert_eq!(place.label(), "2 z 5");
    for (count, expected) in [
        (1, "Dodano 1 utwór na końcu zestawu"),
        (3, "Dodano 3 utwory na końcu zestawu"),
        (5, "Dodano 5 utworów na końcu zestawu"),
    ] {
        assert_eq!(notae_android::ui::add_picker_footer(count), expected);
    }
    assert_eq!(scanning(), "Skanowanie biblioteki…");
    assert_eq!(reading(), "Odczyt tekstu i miniatur PDF: 3 z 7");
    set_active_bundle("");
    assert_eq!(reading(), "Reading PDF text and thumbnails: 3 of 7");
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
