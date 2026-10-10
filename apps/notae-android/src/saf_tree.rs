//! Host-testable traversal of a Storage Access Framework document tree.

use std::io;

use notae_domain::{PdfFingerprint, RelativePdfPath, Sha256Digest};
use notae_index::{CatalogError, FileCondition, PdfFile};
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
