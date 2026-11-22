//! Turns a listing into hashed files without reading what the index or the sidecar already knows.

use std::collections::{HashMap, HashSet};

use notae_domain::{PdfFingerprint, RelativePdfPath, Sha256Digest, SidecarEntry};
use notae_index::{CatalogError, FileCondition, IndexStore, PdfFile};

use crate::sidecar::{Sidecar, entry_digest};
use crate::storage::{LibraryCatalog, ListedPdf};

/// The listed files with their content keys, and what the rescan still has to do for them.
pub(crate) struct Resolved {
    /// One file per listed PDF, in listing order.
    pub files: Vec<PdfFile>,
    /// Paths whose size and modified time match the index: nothing needs reading.
    pub unchanged: HashSet<RelativePdfPath>,
    /// Paths whose sidecar entry differs from the one last applied, with that entry.
    pub sidecar: HashMap<RelativePdfPath, SidecarEntry>,
}

/// Resolves each listed PDF's content: from the index when its size and time match the stored
/// ones, else from a sidecar entry with the same path and size, else by hashing it.
///
/// # Errors
/// [`CatalogError`] when the index cannot be read or the catalog's grant was revoked.
pub(crate) fn resolve(
    local: &IndexStore,
    catalog: &dyn LibraryCatalog,
    listed: Vec<ListedPdf>,
    sidecar: Option<&Sidecar>,
) -> Result<Resolved, CatalogError> {
    let stored: HashMap<RelativePdfPath, PdfFingerprint> = local
        .list_all()
        .map_err(index_error)?
        .into_iter()
        .map(|note| (note.path().clone(), note.fingerprint().clone()))
        .collect();
    let mut resolved = Resolved {
        files: Vec::with_capacity(listed.len()),
        unchanged: HashSet::new(),
        sidecar: HashMap::new(),
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
        } else {
            catalog.hash(&pdf.path, pdf.size, pdf.modified)?
        };
        if let Some(entry) = entry {
            let digest = entry_digest(entry);
            if local.sidecar_digest(&pdf.path).map_err(index_error)? != Some(digest) {
                resolved.sidecar.insert(pdf.path.clone(), entry.clone());
            }
        }
        resolved
            .files
            .push(PdfFile::with_condition(pdf.path, pdf.filename, condition));
    }
    Ok(resolved)
}

fn index_error(error: notae_index::IndexError) -> CatalogError {
    CatalogError::Failed(error.to_string())
}
