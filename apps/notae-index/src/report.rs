//! The run report: what one pipeline execution observed.
//!
//! [`RunReport`] is produced by [`crate::run`] and by
//! [`crate::pipeline::IndexPipeline::run`]. Catalog-only runs leave the
//! pipeline counters at zero, because the pipeline is not what discovered the
//! files; pipeline runs leave them populated from the index activity the run
//! actually performed, plus the failure counts that explain anything the run
//! could not finish.

use std::path::{Path, PathBuf};

use crate::catalog::{CatalogFailure, FilesystemCatalog};
use crate::PdfState;

/// What a run observed, independent of what it wrote.
///
/// The catalog fields enumerate what the filesystem exposed; the pipeline
/// fields count the index activity the run actually performed. Both stay
/// populated together, so a run summary can describe its files and its index
/// writes in one place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    /// The library root that was enumerated.
    library_root: PathBuf,
    /// The files the catalog saw, in stable relative-path order.
    files: Vec<PdfState>,
    /// Traversal problems that did not stop enumeration.
    failures: Vec<CatalogFailure>,
    /// Files that were processed by the pipeline.
    ///
    /// This is the count of files the pipeline saw, including ones it could
    /// not finish with a successful write.
    processed: usize,
    /// Files whose fingerprint matched what the index already held at the same
    /// path.
    reused: usize,
    /// Files whose fingerprint matched a record at a different path.
    moved: usize,
    /// Records deleted because their files are gone.
    deleted: usize,
    /// Files whose OCR step ran but produced no usable text.
    ///
    /// This counts files where the OCR engine refused at least one page or
    /// where every OCR call succeeded but the result was empty.
    ocr_failed: usize,
    /// Files whose PDF could not be opened or parsed at all.
    pdf_failed: usize,
    /// Duplicate-ID conflicts left over after this rescan.
    conflicts: usize,
}

impl RunReport {
    /// Builds a catalog-only report.
    ///
    /// The pipeline counters all start at zero; the catalog counters describe
    /// what the catalog saw.
    #[must_use]
    pub fn from_catalog(catalog: &FilesystemCatalog, files: Vec<PdfState>) -> Self {
        Self {
            library_root: catalog.root().to_path_buf(),
            files,
            failures: catalog.failures(),
            processed: 0,
            reused: 0,
            moved: 0,
            deleted: 0,
            ocr_failed: 0,
            pdf_failed: 0,
            conflicts: 0,
        }
    }

    /// Builds a pipeline report from the catalog and the run's counters.
    ///
    /// `counters.processed` is the number of files the catalog yielded, so the
    /// counter is independent of the per-file outcomes the pipeline recorded.
    #[must_use]
    pub fn from_pipeline(
        catalog: &FilesystemCatalog,
        files: Vec<PdfState>,
        counters: PipelineCounters,
    ) -> Self {
        Self {
            library_root: catalog.root().to_path_buf(),
            files,
            failures: catalog.failures(),
            processed: counters.processed,
            reused: counters.reused,
            moved: counters.moved,
            deleted: counters.deleted,
            ocr_failed: counters.ocr_failed,
            pdf_failed: counters.pdf_failed,
            conflicts: counters.conflicts,
        }
    }

    /// The library root that was enumerated.
    #[must_use]
    pub fn library_root(&self) -> &Path {
        &self.library_root
    }

    /// The enumerated files, in stable relative-path order.
    #[must_use]
    pub fn files(&self) -> &[PdfState] {
        &self.files
    }

    /// The library-relative path of each enumerated file, in order.
    #[must_use]
    pub fn paths(&self) -> Vec<&str> {
        self.files.iter().map(|file| file.path()).collect()
    }

    /// Traversal problems that did not stop enumeration.
    #[must_use]
    pub fn failures(&self) -> &[CatalogFailure] {
        &self.failures
    }

    /// How many PDFs were enumerated.
    #[must_use]
    pub fn enumerated(&self) -> usize {
        self.files.len()
    }

    /// How many PDFs were read and fingerprinted.
    #[must_use]
    pub fn readable(&self) -> usize {
        self.files.iter().filter(|file| file.is_readable()).count()
    }

    /// How many PDFs could not be read.
    #[must_use]
    pub fn unreadable(&self) -> usize {
        self.files.iter().filter(|file| !file.is_readable()).count()
    }

    /// How many files the pipeline processed.
    #[must_use]
    pub fn processed(&self) -> usize {
        self.processed
    }

    /// Files whose fingerprint matched what the index already held at the
    /// same path.
    #[must_use]
    pub fn reused(&self) -> usize {
        self.reused
    }

    /// Files whose fingerprint matched a record at a different path.
    #[must_use]
    pub fn moved(&self) -> usize {
        self.moved
    }

    /// Records deleted because their files are gone.
    #[must_use]
    pub fn deleted(&self) -> usize {
        self.deleted
    }

    /// Files whose OCR step did not produce text.
    #[must_use]
    pub fn ocr_failed(&self) -> usize {
        self.ocr_failed
    }

    /// Files whose PDF could not be opened or parsed at all.
    #[must_use]
    pub fn pdf_failed(&self) -> usize {
        self.pdf_failed
    }

    /// Duplicate-ID conflicts surfaced by the rescan.
    #[must_use]
    pub fn conflicts(&self) -> usize {
        self.conflicts
    }
}

/// Snapshot of the per-file outcome counters the pipeline compiles as it
/// runs.
///
/// The struct is constructed only by the pipeline and only after the run has
/// completed; it lives in this module so the report does not have to know
/// what a `Counters` looks like inside the pipeline.
#[derive(Debug, Clone, Copy, Default)]
pub struct PipelineCounters {
    /// Number of files the pipeline saw.
    pub processed: usize,
    /// Files whose record was reused untouched.
    pub reused: usize,
    /// Files whose record was moved to a new path.
    pub moved: usize,
    /// Records deleted because their files are gone.
    pub deleted: usize,
    /// Files where OCR did not produce text.
    pub ocr_failed: usize,
    /// Files where the PDF layer could not produce text.
    pub pdf_failed: usize,
    /// Duplicate-ID conflicts left over after this rescan.
    pub conflicts: usize,
}
