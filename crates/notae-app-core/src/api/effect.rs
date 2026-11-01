//! Platform work the core asks Kotlin to do, through [`CoreListener::on_effect`].
//!
//! [`CoreListener::on_effect`]: super::CoreListener::on_effect

/// One piece of platform work.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Effect {
    /// Launch the folder picker; report the folder with
    /// [`NotaeCore::library_root_picked`](super::NotaeCore::library_root_picked). A cancelled
    /// picker reports nothing.
    PickLibraryRoot {
        /// The open or saved folder's handle (the SAF tree URI), to start the picker there
        /// (`EXTRA_INITIAL_URI`); `None` before a folder was ever chosen.
        initial_uri: Option<String>,
    },
    /// Open a PDF in the external viewer; report the outcome with
    /// [`NotaeCore::pdf_opened`](super::NotaeCore::pdf_opened).
    OpenPdf {
        /// The library root's handle (the SAF tree URI) the PDF is under.
        uri: String,
        /// The PDF's path below the root; Kotlin resolves the document from the two.
        relative_path: String,
        /// The score's content key.
        note_key: String,
        /// The set item, when the score was opened inside a set.
        item_id: Option<String>,
    },
    /// Launch the speech recognizer; the phrase comes back as
    /// [`Action::SearchChanged`](super::Action::SearchChanged).
    StartVoiceSearch,
    /// Offer the save-file picker for an export; report the outcome with
    /// [`NotaeCore::file_written`](super::NotaeCore::file_written).
    WriteExport {
        /// The file name to suggest.
        suggested_name: String,
        /// The file's contents.
        json: String,
    },
    /// Offer the open-file picker for an import; report the text with
    /// [`NotaeCore::import_file_read`](super::NotaeCore::import_file_read).
    PickImportFile,
}
