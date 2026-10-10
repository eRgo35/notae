use std::collections::{HashMap, HashSet};
use std::io;

use notae_android::saf_tree::{DocumentRow, DocumentTreeReader, enumerate_document_tree};
use notae_domain::{PdfFingerprint, Sha256Digest};
use notae_index::{CatalogError, FileCondition};
use sha2::{Digest, Sha256};

const DIRECTORY_MIME: &str = "vnd.android.document/directory";
const PDF_MIME: &str = "application/pdf";

#[derive(Default)]
struct FakeTree {
    children: HashMap<String, Vec<DocumentRow>>,
    failed_children: HashSet<String>,
    pdfs: HashMap<String, Vec<u8>>,
    unreadable: HashSet<String>,
}

impl DocumentTreeReader for FakeTree {
    fn children(&self, parent_id: &str) -> Result<Vec<DocumentRow>, CatalogError> {
        if self.failed_children.contains(parent_id) {
            return Err(CatalogError::Failed(format!("cannot list {parent_id}")));
        }
        self.children
            .get(parent_id)
            .cloned()
            .ok_or_else(|| CatalogError::Unreadable(parent_id.to_owned()))
    }

    fn read_pdf(&self, document_id: &str) -> io::Result<Vec<u8>> {
        if self.unreadable.contains(document_id) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "provider denied access",
            ));
        }
        self.pdfs
            .get(document_id)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "document disappeared"))
    }
}

fn row(document_id: &str, display_name: &str, mime_type: &str) -> DocumentRow {
    DocumentRow {
        document_id: document_id.to_owned(),
        display_name: display_name.to_owned(),
        mime_type: mime_type.to_owned(),
    }
}

fn fingerprint(bytes: &[u8]) -> PdfFingerprint {
    PdfFingerprint::new(
        Sha256Digest::from_bytes(Sha256::digest(bytes).into()),
        u64::try_from(bytes.len()).expect("fixture length fits in u64"),
        None,
    )
}

#[test]
fn recursive_enumeration_fingerprints_nested_pdfs_and_reports_unreadable_files() {
    let root_bytes = b"%PDF-1.4\nroot".to_vec();
    let nested_bytes = b"%PDF-1.7\nnested".to_vec();
    let mut tree = FakeTree::default();
    tree.children.insert(
        "root".to_owned(),
        vec![
            row("root-pdf", "root.pdf", PDF_MIME),
            row("nested", "nested", DIRECTORY_MIME),
            row("locked", "locked.pdf", PDF_MIME),
            row("notes", "notes.txt", "text/plain"),
        ],
    );
    tree.children.insert(
        "nested".to_owned(),
        vec![row("practice", "practice.pdf", PDF_MIME)],
    );
    tree.pdfs.insert("root-pdf".to_owned(), root_bytes.clone());
    tree.pdfs
        .insert("practice".to_owned(), nested_bytes.clone());
    tree.unreadable.insert("locked".to_owned());

    let files = enumerate_document_tree(&tree, "root").expect("tree traversal succeeds");
    let paths: Vec<_> = files.iter().map(|file| file.path().as_str()).collect();

    assert_eq!(paths, ["root.pdf", "nested/practice.pdf", "locked.pdf"]);
    assert_eq!(files[0].fingerprint(), Some(&fingerprint(&root_bytes)));
    assert_eq!(files[1].fingerprint(), Some(&fingerprint(&nested_bytes)));
    assert!(matches!(files[2].condition(), FileCondition::Unreadable));
    assert!(files.iter().all(|file| file.filename() != "notes.txt"));
}

#[test]
fn child_query_failure_aborts_enumeration_instead_of_returning_partial_files() {
    let mut tree = FakeTree::default();
    tree.children.insert(
        "root".to_owned(),
        vec![
            row("root-pdf", "root.pdf", PDF_MIME),
            row("nested", "nested", DIRECTORY_MIME),
        ],
    );
    tree.pdfs
        .insert("root-pdf".to_owned(), b"%PDF-1.4\nroot".to_vec());
    tree.failed_children.insert("nested".to_owned());

    assert!(matches!(
        enumerate_document_tree(&tree, "root"),
        Err(CatalogError::Failed(_))
    ));
}

#[test]
fn directory_display_name_cannot_create_an_implicit_path() {
    let mut tree = FakeTree::default();
    tree.children.insert(
        "root".to_owned(),
        vec![row("nested", "nested/alias", DIRECTORY_MIME)],
    );
    tree.children.insert(
        "nested".to_owned(),
        vec![row("practice", "practice.pdf", PDF_MIME)],
    );
    tree.pdfs
        .insert("practice".to_owned(), b"%PDF-1.4\npractice".to_vec());

    assert!(matches!(
        enumerate_document_tree(&tree, "root"),
        Err(CatalogError::Failed(_))
    ));
}

#[test]
fn pdf_display_name_cannot_create_an_implicit_path() {
    let mut tree = FakeTree::default();
    tree.children.insert(
        "root".to_owned(),
        vec![row("pdf", "alias/score.pdf", PDF_MIME)],
    );
    tree.pdfs
        .insert("pdf".to_owned(), b"%PDF-1.4\nscore".to_vec());

    assert!(matches!(
        enumerate_document_tree(&tree, "root"),
        Err(CatalogError::Failed(_))
    ));
}
