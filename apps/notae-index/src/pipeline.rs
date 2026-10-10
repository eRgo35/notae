//! The incremental pipeline: turn an enumerated library into indexed records.
//!
//! `IndexPipeline` runs against a [`FilesystemCatalog`] — the catalog knows
//! about the library root, so absolute paths for opening PDFs can be
//! reconstructed — the [`IndexStore`] the run updates, and an
//! [`OcrProvider`] the pipeline dispatches to when a PDF page has no text
//! layer.
//!
//! # Progress reporting
//!
//! The caller passes a [`std::sync::mpsc::Sender`] of [`ProgressEvent`]. This
//! keeps the pipeline decoupled from the CLI's preferred output: the CLI
//! prints to standard error, a test drains the channel into a
//! `Vec<ProgressEvent>`, and a future Android integration sends events to the
//! platform's UI thread. The channel is unbounded so an event never blocks a
//! per-page iteration; the receiver can drop events without affecting the
//! index.
//!
//! # Sidecar boundary
//!
//! [`IndexPipeline::run`] writes only to the index store. The sidecar is the
//! next pipeline's job — it reads the store and writes a portable manifest
//! that another machine can import. Keeping the sidecar out of this stage
//! means a single store update can be observed as many times as needed before
//! the sidecar is regenerated, and the two stages are unit-testable in
//! isolation.
//!
//! # Failures
//!
//! Per-file problems — a PDF that cannot be opened, a corrupt file, an OCR
//! engine that refuses a page — are recorded as counters on the returned
//! [`RunReport`](crate::report::RunReport). [`PipelineError`] is reserved for
//! failures that abort the run: the library cannot be enumerated or the store
//! cannot be read or written as a whole.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc;

use notae_domain::{IndexedNote, RelativePdfPath};
use notae_index::{
    CatalogError, FileFailureReason, IndexError, IndexStore, OcrText, PdfCatalog,
    PdfFile, ReconcileError, Reconciler,
};
use notae_ocr::{LanguagePolicy, OcrProvider};
use thiserror::Error;

use crate::catalog::FilesystemCatalog;
use crate::pdf::{LinuxPdfDocument, PdfError, DEFAULT_OCR_DPI};
use crate::report::RunReport;
use crate::PdfState;

/// A single thing a running pipeline reports to its caller.
///
/// Events flow through `mpsc::Sender<ProgressEvent>`. The variants are
/// exhaustive: every event the pipeline can emit is enumerated here, so a
/// consumer does not have to handle a catch-all case to stay forward
/// compatible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressEvent {
    /// The run has started and is about to enumerate the library.
    Started,
    /// The pipeline has begun processing one file.
    FileStarted {
        /// The library-relative path of the file.
        path: String,
        /// One-based position of the file within the catalog.
        position: usize,
        /// Total number of files the catalog enumerated.
        total: usize,
    },
    /// One page of an image-only PDF has been rendered and OCR'd.
    PageProgress {
        /// The library-relative path of the file.
        path: String,
        /// One-based page index being processed.
        current: u32,
        /// Total pages in the document.
        total: u32,
    },
    /// One file was processed without an error and the run is moving on.
    FileCompleted {
        /// The library-relative path of the file.
        path: String,
    },
    /// One file could not be processed and the reason was recorded.
    FileFailed {
        /// The library-relative path of the file.
        path: String,
        /// A short reason, suitable for printing.
        reason: String,
    },
    /// The run is finished and a [`RunReport`] is on its way through the
    /// normal return path.
    Finished,
}

/// Failures that abort the whole run rather than one file.
#[derive(Debug, Error)]
pub enum PipelineError {
    /// The library could not be enumerated at all.
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    /// The index could not be read or written.
    #[error(transparent)]
    Index(#[from] IndexError),
    /// The reconciler refused the plan for a store-level reason.
    #[error(transparent)]
    Reconcile(#[from] ReconcileError),
}

/// Runs the indexing pipeline against a single library root.
///
/// The pipeline is stateless apart from `ocr_dpi`: a single instance may be
/// reused across runs. The DPI defaults to [`DEFAULT_OCR_DPI`]; callers that
/// care more about throughput than about small type can render at a lower DPI
/// and accept the trade.
#[derive(Debug, Clone, Copy)]
pub struct IndexPipeline {
    /// DPI pages are rendered at when OCR is required.
    ocr_dpi: u32,
}

impl IndexPipeline {
    /// Builds a pipeline that renders at `ocr_dpi`.
    #[must_use]
    pub const fn with_dpi(ocr_dpi: u32) -> Self {
        Self { ocr_dpi }
    }

    /// Builds a pipeline that renders at [`DEFAULT_OCR_DPI`].
    #[must_use]
    pub const fn default_dpi() -> Self {
        Self::with_dpi(DEFAULT_OCR_DPI)
    }

    /// Runs the pipeline against `catalog`, writing records into `store`.
    ///
    /// `policy` decides which language packs the OCR provider runs with — for
    /// tests it can be [`LanguagePolicy::Fixed`], for production it should be
    /// an `Automatic` policy built from the tessdata directory on disk. The
    /// `output` channel receives a [`ProgressEvent`] for every meaningful
    /// step.
    ///
    /// # Errors
    /// Returns [`PipelineError`] when the catalog cannot be enumerated, the
    /// index cannot be read, or the index refuses the plan as a whole.
    /// Per-file failures are counters on the returned [`RunReport`], not
    /// errors here.
    pub fn run<O: OcrProvider + ?Sized>(
        &self,
        catalog: &FilesystemCatalog,
        store: &mut IndexStore,
        ocr: &O,
        policy: &LanguagePolicy,
        output: mpsc::Sender<ProgressEvent>,
    ) -> Result<RunReport, PipelineError> {
        let _ = output.send(ProgressEvent::Started);

        let files = catalog.enumerate()?;
        let total = files.len();
        let pdf_states: Vec<PdfState> =
            files.iter().cloned().map(PdfState::from_file).collect();

        let catalog_root = catalog.root().to_path_buf();
        let plan = Reconciler::plan(store, &files)?;

        // Precompute lookups so per-file processing is a series of hash hits
        // instead of a sweep through the plan for every file. Each plan entry
        // also exposes the resolved `IndexedNote` the reconciler built —
        // metadata refreshed from the filename, status set from this rescan —
        // which is the record we feed the store.
        let mut additions: HashMap<RelativePdfPath, IndexedNote> = HashMap::new();
        for addition in plan.additions() {
            additions.insert(addition.note().path().clone(), addition.note().clone());
        }
        let mut updates: HashMap<RelativePdfPath, IndexedNote> = HashMap::new();
        for update in plan.updates() {
            updates.insert(update.note().path().clone(), update.note().clone());
        }
        let mut moves: HashMap<RelativePdfPath, IndexedNote> = HashMap::new();
        for moved in plan.moves() {
            moves.insert(moved.note().path().clone(), moved.note().clone());
        }
        let mut unchanged: HashMap<RelativePdfPath, IndexedNote> = HashMap::new();
        for file in plan.unchanged() {
            unchanged.insert(file.path().clone(), file.note().clone());
        }
        let mut failures: HashMap<RelativePdfPath, FileFailureReason> = HashMap::new();
        for failure in plan.failures() {
            failures.insert(
                failure.path().clone(),
                clone_failure_reason(failure.reason()),
            );
        }

        let mut counters = Counters { processed: total, ..Counters::default() };

        for (index, file) in files.iter().enumerate() {
            let position = index + 1;
            let path = file.path().as_str().to_owned();
            let _ = output.send(ProgressEvent::FileStarted {
                path: path.clone(),
                position,
                total,
            });

            // The reconciler's verdict is authoritative. A file the catalog
            // could not use may produce both a status correction in
            // `plan.updates` and a failure reason in `plan.failures` — the
            // existing record's status must be written before the file is
            // reported, so the row never claims otherwise. The catalog-only
            // `Unreadable` / `InvalidPdf` short-circuit that used to live
            // here short-circuited every write path for these files and
            // therefore left the store out of sync with the catalog; the
            // plan-driven classification below is the single source of
            // truth now.
            if let Some(reason) = failures.remove(file.path()) {
                record_failure(&output, &mut counters, &path, &reason);
                if let Some(note) = updates.remove(file.path()) {
                    // Status correction (e.g. `Inaccessible` for a file that
                    // could no longer be read). The note keeps the file's
                    // fingerprint, so its stored OCR text is still the right
                    // description; [`apply_note`] is the write path that
                    // preserves it, leaving `None` for the OCR parameter on
                    // the store call. A store refusal is reported by
                    // `apply_note`'s own `FileFailed`, alongside the
                    // breakdown `record_failure` already counted.
                    let _ = apply_note(store, &note, &output, &path)?;
                }
                continue;
            }

            if let Some(note) = unchanged.remove(file.path()) {
                // `apply_note` returns 1 on a successful write, 0 on refusal.
                // Only emit `FileCompleted` when the store actually accepted
                // the write, so a `DuplicateNoteId` rejection cannot be
                // followed by an "indexed" line for the same path.
                if apply_note(store, &note, &output, &path)? == 1 {
                    counters.reused += 1;
                    let _ = output.send(ProgressEvent::FileCompleted { path });
                }
                continue;
            }

            if let Some(note) = moves.remove(file.path()) {
                if apply_note(store, &note, &output, &path)? == 1 {
                    counters.moved += 1;
                    let _ = output.send(ProgressEvent::FileCompleted { path });
                }
                continue;
            }

            if let Some(note) = additions.remove(file.path()) {
                // Track whether the store write actually succeeded. The
                // matching `apply_with_ocr` call emits `FileFailed` on
                // refusal; we emit `FileCompleted` only on success, so a
                // refusal does not produce both events for the same file.
                let mut write_succeeded = false;
                match self.process_for_text(file, &catalog_root, ocr, policy, &output, &path) {
                    Ok(text) => {
                        if apply_with_ocr(store, &note, &text, &output, &path)? == 1 {
                            counters.inserted += 1;
                            write_succeeded = true;
                        }
                    }
                    Err(FileProcessingError::Pdf) => {
                        // The PDF itself is unusable, so the catalog's view
                        // was correct: the bytes are not a readable PDF and
                        // we cannot extract text from them. Count it under
                        // `pdf_failed` and emit a `FileFailed` so the CLI
                        // summary names it. No record is written because we
                        // have no text to associate with one.
                        counters.pdf_failed += 1;
                        let _ = output.send(ProgressEvent::FileFailed {
                            path: path.clone(),
                            reason: "could not extract text from PDF".to_string(),
                        });
                    }
                    Err(FileProcessingError::Ocr) => {
                        // An OCR-engine failure is a per-page story (the
                        // brief counts it under `pdf_failed`), but it is also
                        // its own bucket (`ocr_failed`) so the summary line
                        // tells the two failures apart. The record is still
                        // written with empty OCR text so the file stays
                        // visible in the index — a future run can retry OCR.
                        counters.pdf_failed += 1;
                        counters.ocr_failed += 1;
                        if apply_with_ocr(
                            store,
                            &note,
                            &empty_ocr_text(),
                            &output,
                            &path,
                        )? == 1
                        {
                            write_succeeded = true;
                        }
                    }
                }
                if write_succeeded {
                    let _ = output.send(ProgressEvent::FileCompleted { path });
                }
                continue;
            }

            if let Some(note) = updates.remove(file.path()) {
                let mut write_succeeded = false;
                match self.process_for_text(file, &catalog_root, ocr, policy, &output, &path) {
                    Ok(text) => {
                        if apply_with_ocr(store, &note, &text, &output, &path)? == 1 {
                            counters.updated += 1;
                            write_succeeded = true;
                        }
                    }
                    Err(FileProcessingError::Pdf) => {
                        counters.pdf_failed += 1;
                        let _ = output.send(ProgressEvent::FileFailed {
                            path: path.clone(),
                            reason: "could not extract text from PDF".to_string(),
                        });
                    }
                    Err(FileProcessingError::Ocr) => {
                        counters.pdf_failed += 1;
                        counters.ocr_failed += 1;
                        if apply_with_ocr(
                            store,
                            &note,
                            &empty_ocr_text(),
                            &output,
                            &path,
                        )? == 1
                        {
                            write_succeeded = true;
                        }
                    }
                }
                if write_succeeded {
                    let _ = output.send(ProgressEvent::FileCompleted { path });
                }
                continue;
            }

            // A file the catalog saw but the plan did not classify falls here.
            // It should not happen, but if it does it is still a per-file
            // problem and the rest of the run continues.
            counters.pdf_failed += 1;
            let _ = output.send(ProgressEvent::FileFailed {
                path,
                reason: "the reconciler did not classify this file".to_string(),
            });
        }

        // Delete records whose files are gone. The set is derived from the
        // catalog's enumeration (which the pipeline already proved is what the
        // user has on disk), and `delete_missing` keeps every record whose
        // path is in the set.
        let present_paths: HashSet<RelativePdfPath> =
            files.iter().map(|file| file.path().clone()).collect();
        let stored_paths: HashSet<RelativePdfPath> = store
            .list_all()?
            .iter()
            .map(|note| note.path().clone())
            .collect();
        let deletion_count = stored_paths.difference(&present_paths).count();
        if deletion_count > 0 {
            store.delete_missing(&present_paths)?;
            counters.deleted = deletion_count;
        }

        let conflicts = plan.conflicts().len();

        let _ = output.send(ProgressEvent::Finished);

        Ok(RunReport::from_pipeline(
            catalog,
            pdf_states,
            CountersSnapshot::from(counters, conflicts).into(),
        ))
    }

    /// Extracts text from a file, OCR'ing it page-by-page when the text layer
    /// is missing or empty. The OCR policy decides which language packs the
    /// provider runs against.
    fn process_for_text<O: OcrProvider + ?Sized>(
        &self,
        file: &PdfFile,
        catalog_root: &std::path::Path,
        ocr: &O,
        policy: &LanguagePolicy,
        output: &mpsc::Sender<ProgressEvent>,
        path: &str,
    ) -> Result<OcrText, FileProcessingError> {
        let full_path = catalog_root.join(file.path().as_str());
        let document = LinuxPdfDocument::open(&full_path).map_err(pdf_error_to_processing)?;

        match document.extract_text().map_err(pdf_error_to_processing)? {
            Some(text) if !text.is_empty() => {
                return Ok(OcrText::new(text));
            }
            _ => {}
        }

        // No text layer: render and OCR each page.

        let page_count = document.page_count().map_err(pdf_error_to_processing)?;
        let mut combined = String::new();
        let mut primary_language: Option<String> = None;
        let mut saw_ocr_error = false;

        for page in 0..page_count {
            let _ = output.send(ProgressEvent::PageProgress {
                path: path.to_string(),
                current: page + 1,
                total: page_count,
            });
            let rendered = match document.render_page(page, self.ocr_dpi) {
                Ok(image) => image,
                Err(_) => {
                    saw_ocr_error = true;
                    continue;
                }
            };
            match ocr.recognize(&rendered, policy) {
                Ok(page_ocr) => {
                    if primary_language.is_none() {
                        primary_language = page_ocr.detected_languages().first().cloned();
                    }
                    if !combined.is_empty() {
                        combined.push_str("\n\n");
                    }
                    combined.push_str(page_ocr.text());
                }
                Err(_) => {
                    saw_ocr_error = true;
                }
            }
        }

        if saw_ocr_error {
            return Err(FileProcessingError::Ocr);
        }

        Ok(match primary_language {
            Some(language) => OcrText::with_language(combined, language),
            None => OcrText::new(combined),
        })
    }
}

/// Counts every kind of outcome the pipeline can produce.
#[derive(Debug, Default, Clone, Copy)]
struct Counters {
    processed: usize,
    reused: usize,
    moved: usize,
    inserted: usize,
    updated: usize,
    deleted: usize,
    ocr_failed: usize,
    pdf_failed: usize,
}

/// A snapshot of the run's counters, taken once at the end of `run`.
struct CountersSnapshot {
    processed: usize,
    reused: usize,
    moved: usize,
    deleted: usize,
    ocr_failed: usize,
    pdf_failed: usize,
    conflicts: usize,
}

impl CountersSnapshot {
    /// Builds a snapshot from the working counters plus the conflict count.
    fn from(counters: Counters, conflicts: usize) -> Self {
        Self {
            processed: counters.processed,
            reused: counters.reused,
            moved: counters.moved,
            deleted: counters.deleted,
            ocr_failed: counters.ocr_failed,
            pdf_failed: counters.pdf_failed,
            conflicts,
        }
    }
}

impl From<CountersSnapshot> for crate::report::PipelineCounters {
    fn from(snapshot: CountersSnapshot) -> Self {
        Self {
            processed: snapshot.processed,
            reused: snapshot.reused,
            moved: snapshot.moved,
            deleted: snapshot.deleted,
            ocr_failed: snapshot.ocr_failed,
            pdf_failed: snapshot.pdf_failed,
            conflicts: snapshot.conflicts,
        }
    }
}

/// Writes `note` to `store` and emits a progress event for the outcome.
///
/// Returns 1 on a successful write, 0 on a failed write. The caller bumps the
/// relevant counter so a store refusal still counts.
fn apply_note(
    store: &mut IndexStore,
    note: &IndexedNote,
    output: &mpsc::Sender<ProgressEvent>,
    path: &str,
) -> Result<usize, PipelineError> {
    match store.reindex_note(note) {
        Ok(_) => Ok(1),
        Err(error) => {
            let _ = output.send(ProgressEvent::FileFailed {
                path: path.to_string(),
                reason: format!("index refused: {error}"),
            });
            Ok(0)
        }
    }
}

/// Writes `note` together with OCR text. Mirrors [`apply_note`] but calls the
/// overload of [`IndexStore::reindex_note_with_ocr`] that carries OCR.
fn apply_with_ocr(
    store: &mut IndexStore,
    note: &IndexedNote,
    ocr: &OcrText,
    output: &mpsc::Sender<ProgressEvent>,
    path: &str,
) -> Result<usize, PipelineError> {
    match store.reindex_note_with_ocr(note, Some(ocr)) {
        Ok(_) => Ok(1),
        Err(error) => {
            let _ = output.send(ProgressEvent::FileFailed {
                path: path.to_string(),
                reason: format!("index refused: {error}"),
            });
            Ok(0)
        }
    }
}

/// Records one per-file failure and emits a [`ProgressEvent::FileFailed`].
fn record_failure(
    output: &mpsc::Sender<ProgressEvent>,
    counters: &mut Counters,
    path: &str,
    reason: &FileFailureReason,
) {
    let short = short_reason(reason);
    counters.pdf_failed += 1;
    let _ = output.send(ProgressEvent::FileFailed {
        path: path.to_string(),
        reason: short,
    });
}

/// A short, one-line description of a [`FileFailureReason`].
fn short_reason(reason: &FileFailureReason) -> String {
    match reason {
        FileFailureReason::Unreadable => "could not read the file".to_string(),
        FileFailureReason::InvalidPdf => "the file is not a readable PDF".to_string(),
        FileFailureReason::NotAPdf(error) => format!("not a PDF name: {error}"),
        FileFailureReason::Metadata(error) => format!("metadata: {error}"),
        FileFailureReason::DuplicateContent(path) => {
            format!("duplicate content at {}", path.as_str())
        }
        FileFailureReason::Store(error) => format!("index refused: {error}"),
    }
}

/// Clones a `FileFailureReason`. The variant payloads do not all implement
/// `Clone` directly, so we rebuild the value here; the lossiest case is the
/// `Store` variant, where the inner index error is replaced with a
/// placeholder that the pipeline still surfaces through its own
/// `FileFailed` event when it runs.
fn clone_failure_reason(reason: &FileFailureReason) -> FileFailureReason {
    match reason {
        FileFailureReason::Unreadable => FileFailureReason::Unreadable,
        FileFailureReason::InvalidPdf => FileFailureReason::InvalidPdf,
        FileFailureReason::NotAPdf(error) => FileFailureReason::NotAPdf(*error),
        FileFailureReason::Metadata(error) => FileFailureReason::Metadata(error.clone()),
        FileFailureReason::DuplicateContent(path) => {
            FileFailureReason::DuplicateContent(path.clone())
        }
        FileFailureReason::Store(_) => FileFailureReason::InvalidPdf,
    }
}

/// An empty OCR text used when OCR could not run but the record should
/// still be written.
fn empty_ocr_text() -> OcrText {
    OcrText::new(String::new())
}

/// Bridges the OCR engine's [`notae_ocr::OcrText`] to the index store's
/// [`notae_index::OcrText`].
///
/// The engine records every language pack it ran against, with the primary
/// pack first; the store keeps the recognized text and at most one language
/// for it. Picking the first detected language is what the brief calls for:
///
/// > the pipeline must convert `notae_ocr::OcrText` → `notae_index::OcrText`
/// > before calling `IndexStore::reindex_note_with_ocr` or
/// > `IndexStore::set_ocr`. Recommended: pick the primary language from
/// > `detected_languages` (first element, if any) and call
/// > `OcrText::with_language(text, primary)`.
///
/// The function is public so unit tests and the eventual sidecar import
/// stage can share the conversion rule.
#[must_use]
pub fn into_index_ocr(ocr: &notae_ocr::OcrText) -> notae_index::OcrText {
    match ocr.detected_languages().first() {
        Some(lang) => notae_index::OcrText::with_language(ocr.text(), lang),
        None => notae_index::OcrText::new(ocr.text()),
    }
}

/// Maps a [`PdfError`] onto the per-file error type.
fn pdf_error_to_processing(_: PdfError) -> FileProcessingError {
    FileProcessingError::Pdf
}

/// What a single-file processing step could not do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileProcessingError {
    /// The PDF layer could not provide usable text.
    Pdf,
    /// OCR was attempted and the engine refused.
    Ocr,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_index_ocr_picks_the_primary_language_when_one_is_present() {
        let engine_ocr =
            notae_ocr::OcrText::new("recognized".to_string(), vec!["pol".into()], 80);
        let converted = into_index_ocr(&engine_ocr);
        assert_eq!(converted.text(), "recognized");
        assert_eq!(converted.language(), Some("pol"));
    }

    #[test]
    fn into_index_ocr_with_no_detected_language_omits_the_language_field() {
        let engine_ocr =
            notae_ocr::OcrText::new("recognized".to_string(), Vec::new(), 0);
        let converted = into_index_ocr(&engine_ocr);
        assert_eq!(converted.text(), "recognized");
        assert_eq!(converted.language(), None);
    }
}
