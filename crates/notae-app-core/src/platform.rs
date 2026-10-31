//! The one boundary between the core and the platform's document storage.
//!
//! The platform (the Slint app's JNI code today, Kotlin from 0.5.0) owns every platform API. It
//! lists the library, reads a PDF's text layer and renders a thumbnail, and hands the results to
//! the core as plain data through [`PlatformLibrary`]. [`PlatformCatalog`] turns that data into
//! the [`LibraryCatalog`] the rescan worker reads.
//!
//! The trait's shape is meant to cross a foreign-function boundary unchanged: owned `String`s,
//! no lifetimes, sizes as `u64` and named fields on every enum variant.

#![forbid(unsafe_code)]

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use notae_domain::{PdfFingerprint, RelativePdfPath, Sha256Digest};
use notae_index::{CatalogError, FileCondition, PdfCatalog, PdfFile};

use crate::pdf_pages::PdfError;
use crate::storage::{LibraryCatalog, StorageError};

/// The platform's document storage, called on the worker thread.
///
/// `handle` is the library root's opaque handle ([`LibraryRoot::handle`](crate::storage::LibraryRoot::handle));
/// `path` is a library-relative path as [`DocumentEntry::path`] reported it.
pub trait PlatformLibrary: Send + Sync {
    /// Lists every PDF below the library root.
    ///
    /// A PDF that cannot be read is still listed, with [`DocumentContent::Unreadable`] or
    /// [`DocumentContent::InvalidPdf`].
    ///
    /// # Errors
    /// [`PlatformError`] when the tree cannot be listed at all.
    fn enumerate(&self, handle: String) -> Result<Vec<DocumentEntry>, PlatformError>;

    /// Extracts the PDF's text layer; `Ok(None)` when the platform cannot or it has none.
    ///
    /// # Errors
    /// [`PlatformError`] when the PDF cannot be opened.
    fn extract_text_layer(
        &self,
        handle: String,
        path: String,
    ) -> Result<Option<String>, PlatformError>;

    /// Renders the first page as a PNG at `out_png`; `Ok(false)` when the platform cannot.
    ///
    /// # Errors
    /// [`PlatformError`] when the PDF cannot be opened or rendered.
    fn render_thumbnail(
        &self,
        handle: String,
        path: String,
        out_png: String,
    ) -> Result<bool, PlatformError>;
}

/// One PDF the platform listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentEntry {
    /// The library-relative path, `/`-separated.
    pub path: String,
    /// The document's own name, which filename metadata is parsed from.
    pub display_name: String,
    /// Size in bytes.
    pub size: u64,
    /// What the platform could read of it.
    pub content: DocumentContent,
}

/// What the platform could read of one document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentContent {
    /// Read end to end; the lowercase hex SHA-256 of its bytes.
    Sha256(String),
    /// The document could not be opened or read.
    Unreadable,
    /// The bytes are not a readable PDF.
    InvalidPdf,
}

/// Why a platform call failed. `message` is the platform's own detail, not display text.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlatformError {
    /// The platform failed; `message` says how.
    #[error("{message}")]
    Unavailable {
        /// The platform's message.
        message: String,
    },
    /// The library root's permission was revoked.
    #[error("permission for the library root was lost")]
    PermissionLost,
}

impl From<PlatformError> for StorageError {
    fn from(error: PlatformError) -> Self {
        match error {
            PlatformError::PermissionLost => Self::PermissionLost,
            PlatformError::Unavailable { message } => Self::Other(message),
        }
    }
}

impl From<PlatformError> for CatalogError {
    fn from(error: PlatformError) -> Self {
        match error {
            PlatformError::PermissionLost => Self::PermissionLost,
            PlatformError::Unavailable { message } => Self::Failed(message),
        }
    }
}

impl From<PlatformError> for PdfError {
    fn from(error: PlatformError) -> Self {
        match error {
            PlatformError::Unavailable { message } => Self::Platform(message),
            PlatformError::PermissionLost => Self::Platform(error.to_string()),
        }
    }
}

/// The [`LibraryCatalog`] over a [`PlatformLibrary`] and one library root.
///
/// Each entry becomes a [`PdfFile`]: a [`DocumentContent::Sha256`] with its size becomes a
/// readable fingerprint (no modification time), [`DocumentContent::Unreadable`] and
/// [`DocumentContent::InvalidPdf`] keep their conditions, and a hash that is not 64 lowercase hex
/// characters counts as unreadable. An entry whose path the domain rejects is dropped and kept
/// for [`LibraryCatalog::take_rejected_paths`].
pub struct PlatformCatalog {
    platform: Arc<dyn PlatformLibrary>,
    handle: String,
    rejected: Mutex<Vec<String>>,
}

impl PlatformCatalog {
    /// Reads the library root `handle` through `platform`.
    #[must_use]
    pub fn new(platform: Arc<dyn PlatformLibrary>, handle: String) -> Self {
        Self {
            platform,
            handle,
            rejected: Mutex::new(Vec::new()),
        }
    }
}

impl PdfCatalog for PlatformCatalog {
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError> {
        let entries = self.platform.enumerate(self.handle.clone())?;
        let mut files = Vec::with_capacity(entries.len());
        let mut rejected = Vec::new();
        for entry in entries {
            let Ok(path) = RelativePdfPath::new(entry.path.clone()) else {
                rejected.push(entry.path);
                continue;
            };
            let condition = match entry.content {
                DocumentContent::Sha256(hex) => Sha256Digest::parse(&hex)
                    .map_or(FileCondition::Unreadable, |digest| {
                        FileCondition::Readable(PdfFingerprint::new(digest, entry.size, None))
                    }),
                DocumentContent::Unreadable => FileCondition::Unreadable,
                DocumentContent::InvalidPdf => FileCondition::InvalidPdf,
            };
            files.push(PdfFile::with_condition(path, entry.display_name, condition));
        }
        *self.rejected.lock().unwrap_or_else(PoisonError::into_inner) = rejected;
        Ok(files)
    }
}

impl LibraryCatalog for PlatformCatalog {
    fn extract_text_layer(&self, path: &RelativePdfPath) -> Result<Option<String>, PdfError> {
        Ok(self
            .platform
            .extract_text_layer(self.handle.clone(), path.as_str().to_owned())?)
    }

    fn render_thumbnail(&self, path: &RelativePdfPath, out: &Path) -> Result<bool, PdfError> {
        Ok(self.platform.render_thumbnail(
            self.handle.clone(),
            path.as_str().to_owned(),
            out.to_string_lossy().into_owned(),
        )?)
    }

    fn take_rejected_paths(&self) -> Vec<String> {
        std::mem::take(&mut *self.rejected.lock().unwrap_or_else(PoisonError::into_inner))
    }
}
