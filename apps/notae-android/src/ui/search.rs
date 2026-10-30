use crate::language::pick;
use notae_domain::{IndexStatus, IndexedNote, MetadataField, MetadataOrigin};
use notae_index::IndexError;

/// One library record projected for search and display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRow {
    /// Note number, when one is assigned.
    pub id: Option<String>,
    /// Display title.
    pub title: String,
    /// Composer or artist, when known.
    pub composer_or_artist: Option<String>,
    /// Normalized tags in their stable order.
    pub tags: Vec<String>,
    /// User-facing indexing/OCR status.
    pub status: String,
    /// Library-relative path used to resolve the SAF document for PDF viewing.
    pub relative_path: String,
    /// Stable content key used by open and edit actions.
    pub note_key: String,
    /// Whether the PDF has no extractable text (OCR pending or incomplete). It stays in the
    /// library and is searchable by title and metadata; only the Settings count surfaces it.
    pub text_pending: bool,
    /// Where the title came from, as a short label ("From filename", "From desktop", "Edited").
    pub title_origin: String,
    /// Where the composer came from; empty when there is no composer.
    pub composer_origin: String,
    /// FTS5 snippet of matching lyrics/page text, with `\u{2}`/`\u{3}` around the match.
    pub text_snippet: Option<String>,
    /// The set item this row shows; `None` outside a set.
    pub item_id: Option<String>,
    /// A set's piece whose PDF is no longer in the library.
    pub missing: bool,
}

impl SearchRow {
    pub(crate) fn from_note(note: IndexedNote) -> Self {
        let metadata = note.metadata();
        let composer = metadata.composer_or_artist().map(str::to_owned);
        Self {
            id: note.id().map(|id| id.to_string()),
            title: note.title().to_owned(),
            composer_origin: if composer.is_some() {
                origin_label(metadata.origin_of(MetadataField::ComposerOrArtist)).to_owned()
            } else {
                String::new()
            },
            composer_or_artist: composer,
            tags: metadata.tags().iter().cloned().collect(),
            status: status_label(note.status()).to_owned(),
            text_pending: matches!(
                note.status(),
                IndexStatus::OcrPending | IndexStatus::OcrIncomplete
            ),
            title_origin: origin_label(metadata.origin_of(MetadataField::Title)).to_owned(),
            note_key: note.fingerprint().sha256().to_string(),
            relative_path: note.path().as_str().to_owned(),
            text_snippet: None,
            item_id: None,
            missing: false,
        }
    }
}

/// Projects a search result onto the Slint row the Library and Search screens show.
///
/// `android_main` calls this for every row it publishes; host tests call it to
/// drive the generated UI with the real projection.
pub fn library_row(
    row: SearchRow,
    thumbnail: Option<slint::Image>,
    favorite: bool,
) -> crate::LibraryRow {
    let (before, hit, after) = row
        .text_snippet
        .as_deref()
        .map(split_snippet)
        .unwrap_or_default();
    crate::LibraryRow {
        id: row.id.unwrap_or_default().into(),
        title: row.title.into(),
        composer_or_artist: row.composer_or_artist.unwrap_or_default().into(),
        tags: row.tags.join(", ").into(),
        // An image-only PDF is searchable by title and metadata; the library does not nag about it.
        status: if row.text_pending {
            String::new()
        } else {
            row.status
        }
        .into(),
        note_key: row.note_key.into(),
        relative_path: row.relative_path.into(),
        has_thumbnail: thumbnail.is_some(),
        thumbnail: thumbnail.unwrap_or_default(),
        title_origin: row.title_origin.into(),
        composer_origin: row.composer_origin.into(),
        snippet_before: before.into(),
        snippet_match: hit.into(),
        snippet_after: after.into(),
        favorite,
        item_id: row.item_id.unwrap_or_default().into(),
        // Set by the caller inside a set, where it knows the item's number.
        position: "".into(),
        missing: row.missing,
        just_opened: false,
    }
}

fn origin_label(origin: MetadataOrigin) -> &'static str {
    match origin {
        MetadataOrigin::FilenameDerived => pick("From filename", "Z nazwy pliku"),
        MetadataOrigin::PcImported => pick("From desktop", "Z komputera"),
        MetadataOrigin::Manual => pick("Edited", "Edytowano"),
        _ => "",
    }
}

fn status_label(status: IndexStatus) -> &'static str {
    match status {
        IndexStatus::Discovered => pick("Discovered", "Wykryto"),
        IndexStatus::Indexing => pick("Indexing", "Indeksowanie"),
        IndexStatus::Ready => pick("Ready", "Gotowe"),
        IndexStatus::OcrPending => pick("OCR pending", "Czeka na OCR"),
        IndexStatus::OcrIncomplete => pick("OCR incomplete", "OCR niepełny"),
        IndexStatus::Stale => pick("Stale", "Nieaktualne"),
        IndexStatus::DuplicateIdConflict => pick("ID conflict", "Konflikt numeru"),
        IndexStatus::Inaccessible => pick("Inaccessible", "Niedostępne"),
        IndexStatus::InvalidPdf => pick("Invalid PDF", "Nieprawidłowy PDF"),
        IndexStatus::ImportMismatch => pick("Import mismatch", "Niezgodność importu"),
    }
}

/// An error returned while projecting search results or applying a library operation.
#[derive(Debug, thiserror::Error)]
pub enum UiError {
    /// The local index could not produce the requested rows.
    #[error("library search failed: {0}")]
    Index(#[from] IndexError),
    /// A metadata value failed domain validation.
    #[error(transparent)]
    Domain(#[from] notae_domain::DomainError),
    /// The requested content key is not in the local index.
    #[error("indexed note not found: {0}")]
    NoteNotFound(String),
    /// A sidecar preview could not be read or applied.
    #[error(transparent)]
    Import(#[from] notae_index::SidecarError),
    /// The user has not confirmed the visible import preview.
    #[error("sidecar import requires explicit confirmation")]
    ConfirmationRequired,
    /// No preview is available to apply.
    #[error("there is no sidecar preview to apply")]
    NoImportPreview,
    /// Another index operation is already running.
    #[error("a rescan or import is already in progress")]
    OperationAlreadyInProgress,
    /// Cancellation was requested while no operation was active.
    #[error("no operation is in progress")]
    NoOperationInProgress,
    /// Reconciliation has committed and the remaining text extraction cannot be canceled.
    #[error("the rescan has already committed and can no longer be canceled")]
    RescanNotCancellable,
    /// The background operation thread has stopped.
    #[error("the index worker has stopped")]
    WorkerStopped,
    /// The background operation thread could not be created.
    #[error("failed to start the index worker: {0}")]
    WorkerStart(#[source] std::io::Error),
}

/// Splits an FTS5 snippet into (before, match, after) around its first marked match.
#[must_use]
pub fn split_snippet(snippet: &str) -> (String, String, String) {
    let clean = |text: &str| text.replace(['\u{2}', '\u{3}'], "");
    let Some((before, rest)) = snippet.split_once('\u{2}') else {
        return (clean(snippet), String::new(), String::new());
    };
    let (hit, after) = rest.split_once('\u{3}').unwrap_or((rest, ""));
    let before = clean(before);
    let count = before.chars().count();
    let before = if count > 48 {
        format!("…{}", before.chars().skip(count - 47).collect::<String>())
    } else {
        before
    };
    (before, hit.to_owned(), clean(after))
}
