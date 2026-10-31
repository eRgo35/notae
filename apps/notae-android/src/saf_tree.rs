//! Host-testable traversal of a Storage Access Framework document tree.

use std::io;

use notae_app_core::platform::{DocumentContent, DocumentEntry};
use notae_domain::{RelativePdfPath, Sha256Digest};
use notae_index::CatalogError;
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

/// Resolves a validated library-relative PDF path to its opaque SAF document ID.
pub fn find_document_id(
    reader: &impl DocumentTreeReader,
    root_id: &str,
    relative_path: &str,
) -> Result<String, CatalogError> {
    let path = RelativePdfPath::new(relative_path.to_owned())
        .map_err(|error| CatalogError::Failed(format!("invalid PDF path: {error}")))?;
    let mut segments = path.as_str().split('/').peekable();
    let mut parent_id = root_id.to_owned();
    while let Some(segment) = segments.next() {
        let is_file = segments.peek().is_none();
        if is_file && !is_pdf_name(segment) {
            return Err(CatalogError::Failed(format!(
                "not a PDF library path: {relative_path}"
            )));
        }
        let children = reader.children(&parent_id)?;
        let mut matches = children.iter().filter(|row| {
            row.display_name == segment && (row.mime_type == DIRECTORY_MIME) != is_file
        });
        let Some(row) = matches.next() else {
            return Err(CatalogError::Failed(format!(
                "SAF document not found: {relative_path}"
            )));
        };
        if matches.next().is_some() {
            return Err(CatalogError::Failed(format!(
                "ambiguous SAF path segment {segment:?} in {relative_path}"
            )));
        }
        parent_id.clone_from(&row.document_id);
    }
    Ok(parent_id)
}

/// Recursively enumerates PDFs below a SAF root as root-relative, hashed entries.
///
/// A display name that is not one path segment, or a joined path the domain rejects, fails the
/// whole enumeration.
pub fn enumerate_document_tree(
    reader: &impl DocumentTreeReader,
    root_id: &str,
) -> Result<Vec<DocumentEntry>, CatalogError> {
    let mut files = Vec::new();
    enumerate_children(reader, root_id, "", &mut files)?;
    Ok(files)
}

fn enumerate_children(
    reader: &impl DocumentTreeReader,
    parent_id: &str,
    parent_path: &str,
    files: &mut Vec<DocumentEntry>,
) -> Result<(), CatalogError> {
    for child in reader.children(parent_id)? {
        if child.mime_type == DIRECTORY_MIME {
            let path = join_path(parent_path, &child.display_name)?;
            enumerate_children(reader, &child.document_id, &path, files)?;
        } else if is_pdf_name(&child.display_name) {
            let path = RelativePdfPath::new(join_path(parent_path, &child.display_name)?)
                .map_err(|error| CatalogError::Failed(format!("invalid SAF path: {error}")))?;
            let (content, size) = match reader.read_pdf(&child.document_id) {
                Ok(bytes) => (
                    DocumentContent::Sha256(sha256_hex(&bytes)),
                    byte_len(&bytes),
                ),
                Err(_) => (DocumentContent::Unreadable, 0),
            };
            files.push(DocumentEntry {
                path: path.as_str().to_owned(),
                display_name: child.display_name,
                size,
                content,
            });
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

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256Digest::from_bytes(Sha256::digest(bytes).into()).to_string()
}

// The SAF byte slice is bounded by addressable Android memory, which fits u64 on supported targets.
#[allow(clippy::expect_used)]
fn byte_len(bytes: &[u8]) -> u64 {
    u64::try_from(bytes.len()).expect("a byte vector length fits in u64")
}
