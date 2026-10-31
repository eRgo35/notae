//! The Slint app's English and Polish text for the core's typed values.
//!
//! The core returns titles, counts, dates and statuses as values; this module turns each one into
//! the string the 0.3.x screens show, in the language [`crate::language`] has selected.

#![forbid(unsafe_code)]

use notae_app_core::operations::events::RescanSummary;
use notae_app_core::sets::SetFieldError;
use notae_app_core::text::{
    Edited, PositionOf, RowStatus, SectionTitle, SetSubtitle, StatusMessage,
};
use notae_domain::MetadataOrigin;

use crate::language::{pick, polish_plural};

/// A section's title or side-panel label.
#[must_use]
pub fn section_title(title: &SectionTitle) -> String {
    match title {
        SectionTitle::All => pick("All scores", "Wszystkie utwory").to_owned(),
        SectionTitle::Recent => pick("Recent", "Ostatnie").to_owned(),
        SectionTitle::Favorites => pick("Favorites", "Ulubione").to_owned(),
        SectionTitle::Sets => pick("Sets", "Zestawy").to_owned(),
        SectionTitle::Composer(name) | SectionTitle::Set(name) => name.clone(),
    }
}

/// "1 Nov 2026 · 5 pieces · 1 note"; a missing date and zero counts are left out.
#[must_use]
pub fn set_subtitle(subtitle: &SetSubtitle) -> String {
    let SetSubtitle {
        date,
        pieces,
        notes,
    } = subtitle;
    let (pieces, notes) = (*pieces, *notes);
    let mut parts: Vec<String> = date.as_deref().map(format_set_date).into_iter().collect();
    if pieces > 0 {
        parts.push(pick(
            format!("{pieces} {}", if pieces == 1 { "piece" } else { "pieces" }),
            format!(
                "{pieces} {}",
                polish_plural(pieces, "utwór", "utwory", "utworów")
            ),
        ));
    }
    if notes > 0 {
        parts.push(pick(
            format!("{notes} {}", if notes == 1 { "note" } else { "notes" }),
            format!(
                "{notes} {}",
                polish_plural(notes, "notatka", "notatki", "notatek")
            ),
        ));
    }
    parts.join(" · ")
}

/// "Edited today", "Edited yesterday", "Edited 3 days ago" or "Edited 1 Nov 2026".
#[must_use]
pub fn edited(edited: &Edited) -> String {
    match edited {
        Edited::Today => pick("Edited today", "Edytowano dziś").to_owned(),
        Edited::Yesterday => pick("Edited yesterday", "Edytowano wczoraj").to_owned(),
        Edited::DaysAgo(days) => pick(
            format!("Edited {days} days ago"),
            format!(
                "Edytowano {days} {} temu",
                polish_plural(
                    usize::try_from(*days).unwrap_or(usize::MAX),
                    "dzień",
                    "dni",
                    "dni"
                )
            ),
        ),
        Edited::On(iso) => {
            let date = format_set_date(iso);
            pick(format!("Edited {date}"), format!("Edytowano {date}"))
        }
    }
}

/// A row's status label. A row without text shows none: the library does not nag about
/// image-only PDFs.
#[must_use]
pub fn row_status(status: RowStatus) -> &'static str {
    match status {
        RowStatus::Ready => pick("Ready", "Gotowe"),
        RowStatus::TextPending => "",
        RowStatus::MissingFromLibrary => pick("Missing from library", "Brak w bibliotece"),
        RowStatus::Unreadable => pick("Inaccessible", "Niedostępne"),
        RowStatus::InvalidPdf => pick("Invalid PDF", "Nieprawidłowy PDF"),
        RowStatus::Discovered => pick("Discovered", "Wykryto"),
        RowStatus::Indexing => pick("Indexing", "Indeksowanie"),
        RowStatus::Stale => pick("Stale", "Nieaktualne"),
        RowStatus::IdConflict => pick("ID conflict", "Konflikt numeru"),
        RowStatus::ImportMismatch => pick("Import mismatch", "Niezgodność importu"),
    }
}

/// Where a title or composer came from; empty when it is not known.
#[must_use]
pub fn origin(origin: MetadataOrigin) -> &'static str {
    match origin {
        MetadataOrigin::FilenameDerived => pick("From filename", "Z nazwy pliku"),
        MetadataOrigin::PcImported => pick("From desktop", "Z komputera"),
        MetadataOrigin::Manual => pick("Edited", "Edytowano"),
        MetadataOrigin::Unset => "",
    }
}

/// The operation banner's text.
#[must_use]
pub fn status_message(message: &StatusMessage) -> String {
    match message {
        StatusMessage::ScanningLibrary => {
            pick("Scanning library…", "Skanowanie biblioteki…").to_owned()
        }
        StatusMessage::Indexing { total: 1 } => {
            pick("Indexing 1 PDF…", "Indeksowanie 1 pliku PDF…").to_owned()
        }
        StatusMessage::Indexing { total } => pick(
            format!("Indexing {total} PDFs…"),
            format!("Indeksowanie {total} plików PDF…"),
        ),
        StatusMessage::ReadingText { completed, total } => pick(
            format!("Reading PDF text and thumbnails: {completed} of {total}"),
            format!("Odczyt tekstu i miniatur PDF: {completed} z {total}"),
        ),
        StatusMessage::Finished(summary) => {
            let summary = rescan_summary(summary);
            pick(
                format!("Finished: {summary}"),
                format!("Zakończono: {summary}"),
            )
        }
        StatusMessage::Failed { reason } => reason.clone(),
    }
}

/// "3 added, 1 ID conflicts", or "No changes".
fn rescan_summary(summary: &RescanSummary) -> String {
    let mut parts = Vec::new();
    add_count(&mut parts, summary.added, "added", "dodano");
    add_count(&mut parts, summary.updated, "updated", "zaktualizowano");
    add_count(&mut parts, summary.moved, "moved", "przeniesiono");
    add_count(&mut parts, summary.deleted, "deleted", "usunięto");
    add_count(
        &mut parts,
        summary.conflicts,
        "ID conflicts",
        "konflikty numerów",
    );
    add_count(&mut parts, summary.failures, "failures", "błędy");
    if parts.is_empty() {
        pick("No changes", "Bez zmian").to_owned()
    } else {
        parts.join(", ")
    }
}

/// English reads "3 added"; Polish reads "dodano: 3", which needs no plural agreement.
fn add_count(parts: &mut Vec<String>, count: usize, english: &str, polish: &str) {
    if count != 0 {
        parts.push(pick(
            format!("{count} {english}"),
            format!("{polish}: {count}"),
        ));
    }
}

/// "2 of 5" (Polish "2 z 5"), shown in the details pane.
#[must_use]
pub fn position_of(place: PositionOf) -> String {
    let PositionOf { position, total } = place;
    pick(
        format!("{position} of {total}"),
        format!("{position} z {total}"),
    )
}

/// A set card's date, or "No date".
#[must_use]
pub fn set_date(iso: Option<&str>) -> String {
    iso.map_or_else(|| pick("No date", "Bez daty").to_owned(), format_set_date)
}

/// "3 items" (Polish "3 pozycje"): the pieces and notes a set holds.
#[must_use]
pub fn item_count(count: usize) -> String {
    pick(
        format!("{count} {}", if count == 1 { "item" } else { "items" }),
        format!(
            "{count} {}",
            polish_plural(count, "pozycja", "pozycje", "pozycji")
        ),
    )
}

/// The Sets section's subtitle: how many sets there are and how they are ordered.
#[must_use]
pub fn sets_subtitle(count: usize) -> String {
    pick(
        format!(
            "{count} {} · recently edited first",
            if count == 1 { "set" } else { "sets" }
        ),
        format!(
            "{count} {} · ostatnio edytowane najpierw",
            polish_plural(count, "zestaw", "zestawy", "zestawów")
        ),
    )
}

/// The set dialog's message for rejected fields.
#[must_use]
pub fn set_field_error(error: SetFieldError) -> &'static str {
    match error {
        SetFieldError::EmptyName => pick("Name is required", "Nazwa jest wymagana"),
        SetFieldError::InvalidDate => pick(
            "Use a real date as YYYY-MM-DD",
            "Podaj prawdziwą datę jako RRRR-MM-DD",
        ),
    }
}

/// "3 added at the end of the set" (Polish "Dodano 3 utwory na końcu zestawu").
#[must_use]
pub fn add_picker_footer(count: usize) -> String {
    pick(
        format!("{count} added at the end of the set"),
        format!(
            "Dodano {count} {} na końcu zestawu",
            polish_plural(count, "utwór", "utwory", "utworów")
        ),
    )
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const POLISH_MONTHS: [&str; 12] = [
    "sty", "lut", "mar", "kwi", "maj", "cze", "lip", "sie", "wrz", "paź", "lis", "gru",
];

/// A stored `YYYY-MM-DD` date as "1 Nov 2026" (Polish "1 lis 2026"). Anything else is returned
/// as written.
#[must_use]
pub fn format_set_date(iso: &str) -> String {
    let mut parts = iso.splitn(3, '-');
    let parsed = (|| {
        let year: i64 = parts.next()?.parse().ok()?;
        let month: usize = parts.next()?.parse().ok()?;
        let day: u32 = parts.next()?.parse().ok()?;
        ((1..=12).contains(&month) && (1..=31).contains(&day)).then_some((year, month, day))
    })();
    parsed.map_or_else(
        || iso.to_owned(),
        |(year, month, day)| {
            let name = pick(MONTHS[month - 1], POLISH_MONTHS[month - 1]);
            format!("{day} {name} {year}")
        },
    )
}
