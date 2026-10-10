//! Host-testable traversal of a Storage Access Framework document tree.

use std::io;

use notae_domain::{PdfFingerprint, RelativePdfPath, Sha256Digest};
use notae_index::{CatalogError, FileCondition, PdfFile, SidecarManifest};
use sha2::{Digest, Sha256};

const DIRECTORY_MIME: &str = "vnd.android.document/directory";

/// One child returned by a SAF document provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentRow {
    pub document_id: String,
    pub display_name: String,
    pub mime_type: String,
}

/// The platform operations needed to enumerate a document tree.
pub trait DocumentTreeReader {
    /// Lists the immediate children of `parent_id`.
    fn children(&self, parent_id: &str) -> Result<Vec<DocumentRow>, CatalogError>;

    /// Reads one PDF document end-to-end.
    fn read_pdf(&self, document_id: &str) -> io::Result<Vec<u8>>;

    /// Reads a non-PDF document through the same SAF document handle.
    fn read_document(&self, document_id: &str) -> io::Result<Vec<u8>> {
        self.read_pdf(document_id)
    }
}

/// Finds and validates `<root>/.notae/manifest.json`, if the selected tree has one.
pub fn discover_sidecar_manifest(
    reader: &impl DocumentTreeReader,
    root_id: &str,
) -> Result<Option<SidecarManifest>, CatalogError> {
    let root_children = reader.children(root_id)?;
    let mut sidecar_dirs = root_children
        .iter()
        .filter(|row| row.display_name == ".notae" && row.mime_type == DIRECTORY_MIME);
    let Some(directory) = sidecar_dirs.next() else {
        return Ok(None);
    };
    if sidecar_dirs.next().is_some() {
        return Err(CatalogError::Failed(
            "SAF root contains multiple .notae directories".to_owned(),
        ));
    }

    let children = reader.children(&directory.document_id)?;
    let mut manifests = children
        .iter()
        .filter(|row| row.display_name == "manifest.json" && row.mime_type != DIRECTORY_MIME);
    let Some(manifest) = manifests.next() else {
        return Ok(None);
    };
    if manifests.next().is_some() {
        return Err(CatalogError::Failed(
            "sidecar contains multiple manifest.json documents".to_owned(),
        ));
    }

    let bytes = reader
        .read_document(&manifest.document_id)
        .map_err(|error| CatalogError::Failed(format!("cannot read sidecar manifest: {error}")))?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| CatalogError::Failed(format!("sidecar manifest is not UTF-8: {error}")))?;
    SidecarManifest::from_json(text)
        .map(Some)
        .map_err(|error| CatalogError::Failed(format!("invalid sidecar manifest: {error}")))
}

/// Recursively enumerates PDFs below a SAF root as root-relative index rows.
pub fn enumerate_document_tree(
    reader: &impl DocumentTreeReader,
    root_id: &str,
) -> Result<Vec<PdfFile>, CatalogError> {
    let mut files = Vec::new();
    enumerate_children(reader, root_id, "", &mut files)?;
    Ok(files)
}

fn enumerate_children(
    reader: &impl DocumentTreeReader,
    parent_id: &str,
    parent_path: &str,
    files: &mut Vec<PdfFile>,
) -> Result<(), CatalogError> {
    for child in reader.children(parent_id)? {
        if child.mime_type == DIRECTORY_MIME {
            let path = join_path(parent_path, &child.display_name)?;
            enumerate_children(reader, &child.document_id, &path, files)?;
        } else if is_pdf_name(&child.display_name) {
            let path = RelativePdfPath::new(join_path(parent_path, &child.display_name)?)
                .map_err(|error| CatalogError::Failed(format!("invalid SAF path: {error}")))?;
            let condition = match reader.read_pdf(&child.document_id) {
                Ok(bytes) => FileCondition::Readable(fingerprint(&bytes)),
                Err(_) => FileCondition::Unreadable,
            };
            files.push(PdfFile::with_condition(path, child.display_name, condition));
        }
    }
    Ok(())
}

fn join_path(parent: &str, name: &str) -> Result<String, CatalogError> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return Err(CatalogError::Failed(format!(
            "SAF display name is not one path segment: {name:?}"
        )));
    }
    if parent.is_empty() {
        Ok(name.to_owned())
    } else {
        Ok(format!("{parent}/{name}"))
    }
}

fn is_pdf_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() >= 4 && bytes[bytes.len() - 4..].eq_ignore_ascii_case(b".pdf")
}

fn fingerprint(bytes: &[u8]) -> PdfFingerprint {
    let digest = Sha256::digest(bytes);
    PdfFingerprint::new(
        Sha256Digest::from_bytes(digest.into()),
        u64::try_from(bytes.len()).expect("a byte vector length fits in u64"),
        None,
    )
}
