//! Turns a listing into hashed files without reading what the index or the sidecar already knows.

use std::collections::{HashMap, HashSet};

use notae_domain::{IndexStatus, PdfFingerprint, RelativePdfPath, Sha256Digest, SidecarEntry};
use notae_index::{CatalogError, FileCondition, IndexStore, PdfFile};

use crate::sidecar::{Sidecar, entry_digest};
use crate::storage::{LibraryCatalog, ListedPdf};

/// The listed files with their content keys, and what the rescan still has to do for them.
pub(crate) struct Resolved {
    /// One file per listed PDF, in listing order.
    pub files: Vec<PdfFile>,
    /// Paths whose size and modified time match the index: nothing needs reading.
    pub unchanged: HashSet<RelativePdfPath>,
    /// Paths matched to a sidecar entry, with that entry: their text and thumbnail come from it.
    pub sidecar: HashMap<RelativePdfPath, SidecarEntry>,
    /// The paths in `sidecar` whose entry differs from the one last applied.
    pub apply: HashSet<RelativePdfPath>,
}

/// Resolves each listed PDF's content: from the index when its size and time match the stored
/// ones and its note finished indexing, else from a sidecar entry with the same path and size,
/// else by hashing it.
///
/// A note has finished indexing when its status is [`IndexStatus::Ready`] or
/// [`IndexStatus::OcrPending`] (the text layer was read and had no text, which reading it again
/// would not change). Every other status means a scan stopped part way or a step failed
/// (`Discovered`, `Indexing`, `OcrIncomplete`, `Stale`), or that the file has to be read again to
/// know its condition (`Inaccessible`, `InvalidPdf`, `DuplicateIdConflict`, the legacy
/// `ImportMismatch`), so the file is resolved again and its text and thumbnail steps run.
///
/// The sidecar entry of an unchanged file is attached only when its `sha256` is the stored one.
///
/// Returns `None`, hashing nothing more, once `cancelled` answers `true` before a file is hashed.
///
/// # Errors
/// [`CatalogError`] when the index cannot be read or the catalog's grant was revoked.
pub(crate) fn resolve(
    local: &IndexStore,
    catalog: &dyn LibraryCatalog,
    listed: Vec<ListedPdf>,
    sidecar: Option<&Sidecar>,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Resolved>, CatalogError> {
    let stored: HashMap<RelativePdfPath, PdfFingerprint> = local
        .list_all()
        .map_err(index_error)?
        .into_iter()
        .filter(|note| matches!(note.status(), IndexStatus::Ready | IndexStatus::OcrPending))
        .map(|note| (note.path().clone(), note.fingerprint().clone()))
        .collect();
    let mut resolved = Resolved {
        files: Vec::with_capacity(listed.len()),
        unchanged: HashSet::new(),
        sidecar: HashMap::new(),
        apply: HashSet::new(),
    };
    for pdf in listed {
        let entry = sidecar.and_then(|sidecar| sidecar.matching(pdf.path.as_str(), pdf.size));
        let known = stored.get(&pdf.path).filter(|fingerprint| {
            fingerprint.byte_len() == pdf.size
                && pdf.modified.is_some()
                && fingerprint.modified() == pdf.modified
        });
        let from_entry = entry.and_then(|entry| Sha256Digest::parse(&entry.sha256).ok());
        let condition = if let Some(fingerprint) = known {
            resolved.unchanged.insert(pdf.path.clone());
            FileCondition::Readable(PdfFingerprint::new(
                fingerprint.sha256(),
                pdf.size,
                pdf.modified,
            ))
        } else if let Some(digest) = from_entry {
            FileCondition::Readable(PdfFingerprint::new(digest, pdf.size, pdf.modified))
        } else if cancelled() {
            return Ok(None);
        } else {
            catalog.hash(&pdf.path, pdf.size, pdf.modified)?
        };
        // Never attach an unchanged file to an entry that describes other content.
        let entry = entry.filter(|entry| {
            known.is_none_or(|fingerprint| {
                Sha256Digest::parse(&entry.sha256).ok() == Some(fingerprint.sha256())
            })
        });
        if let Some(entry) = entry {
            let digest = entry_digest(entry);
            if local.sidecar_digest(&pdf.path).map_err(index_error)? != Some(digest) {
                resolved.apply.insert(pdf.path.clone());
            }
            resolved.sidecar.insert(pdf.path.clone(), entry.clone());
        }
        resolved
            .files
            .push(PdfFile::with_condition(pdf.path, pdf.filename, condition));
    }
    Ok(Some(resolved))
}

fn index_error(error: notae_index::IndexError) -> CatalogError {
    CatalogError::Failed(error.to_string())
}
