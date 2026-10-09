//! Reconciling an index against the PDFs a catalog enumerates.
//!
//! A rescan is two steps. [`Reconciler::plan`] reads the existing index and the
//! enumerated files and produces a [`RescanPlan`] without writing anything, so
//! the caller can show exactly what would change. [`Reconciler::apply`] then
//! performs that plan and returns a [`RescanReport`] describing what it did.
//!
//! # Identity
//!
//! A file is identified by the SHA-256 of its contents, never by its path. A
//! rename therefore *moves* the one existing record rather than creating a new
//! one, while new bytes at a known path are an *update* that keeps the record —
//! its note ID, its row, and every manually edited field. Paths are matched too,
//! but only to classify the difference between a move and a change, never to
//! decide identity.
//!
//! # Failure isolation
//!
//! One unreadable or unnameable PDF must not abort a rescan. A file whose
//! metadata cannot be derived becomes a [`FileFailure`] in the plan and is
//! otherwise ignored; every other file is still classified and applied.
//!
//! # Conflicts
//!
//! Duplicate note IDs are surfaced, never resolved. When two files would claim
//! one ID, the record that is already indexed keeps it, the newcomer is left
//! out of the plan, and a [`DuplicateIdConflict`] naming both paths is reported
//! so the user can decide. The surviving record is marked
//! [`IndexStatus::DuplicateIdConflict`] so the library shows the collision too.
//!
//! See [`crate::provenance`] for how manual edits are preserved.

use std::collections::HashMap;

use notae_domain::{
    FilenameError, IndexStatus, IndexedNote, NoteId, PdfFingerprint, RelativePdfPath, Sha256Digest,
    parse_note_filename, validate_unique_ids,
};
use thiserror::Error;

use crate::provenance::{MetadataRefreshError, metadata_from_filename, refresh_from_parsed};
use crate::store::{IndexError, IndexStore};

/// Failures produced by enumerating a library.
///
/// A catalog reports only what stopped it from listing the whole tree. A single
/// file that cannot be fingerprinted is not a catalog failure: enumeration
/// skips it and the rescan reports it, so one bad PDF cannot abort the run.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CatalogError {
    /// The library root could not be opened or traversed.
    #[error("library root is not readable: {0}")]
    Unreadable(String),
    /// Traversal started but failed part way through the tree.
    #[error("library enumeration failed: {0}")]
    Failed(String),
}

/// One enumerated PDF, as the catalog saw it.
///
/// The [`filename`](PdfFile::filename) is the file's own name, kept beside the
/// library-relative [`path`](PdfFile::path) because a provider such as the
/// Storage Access Framework knows a document's display name independently of
/// where Notae files it, and the name is what filename-derived metadata is
/// parsed from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfFile {
    path: RelativePdfPath,
    fingerprint: PdfFingerprint,
    filename: String,
}

impl PdfFile {
    /// Describes an enumerated PDF.
    #[must_use]
    pub const fn new(
        path: RelativePdfPath,
        fingerprint: PdfFingerprint,
        filename: String,
    ) -> Self {
        Self {
            path,
            fingerprint,
            filename,
        }
    }

    /// Describes an enumerated PDF whose display name is the last segment of
    /// its library-relative path.
    #[must_use]
    pub fn from_path(path: RelativePdfPath, fingerprint: PdfFingerprint) -> Self {
        let filename = path
            .as_str()
            .rsplit('/')
            .next()
            .unwrap_or(path.as_str())
            .to_owned();
        Self::new(path, fingerprint, filename)
    }

    /// The library-relative path the record is filed under.
    #[must_use]
    pub const fn path(&self) -> &RelativePdfPath {
        &self.path
    }

    /// The observed content fingerprint.
    #[must_use]
    pub const fn fingerprint(&self) -> &PdfFingerprint {
        &self.fingerprint
    }

    /// The file's own name.
    #[must_use]
    pub fn filename(&self) -> &str {
        &self.filename
    }
}

/// A source of the PDFs under the selected library root.
///
/// The trait is object safe and knows nothing about `std::fs`: on Android an
/// implementation walks a Storage Access Framework tree and reads content
/// URIs, on a desktop it walks a directory. Either way it yields [`PdfFile`]
/// values, and nothing above this trait learns how they were produced.
pub trait PdfCatalog {
    /// Lists every PDF under the library root.
    ///
    /// # Errors
    /// Returns [`CatalogError`] when the tree cannot be read at all. A file
    /// that cannot be read individually must be skipped, not fail the whole
    /// enumeration.
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError>;
}

/// Why one file could not be reconciled.
#[derive(Debug, Error)]
pub enum FileFailureReason {
    /// The name is not a PDF name.
    #[error("not a PDF filename: {0}")]
    NotAPdf(#[from] FilenameError),
    /// The name parsed, but the metadata it carries is not valid.
    #[error(transparent)]
    Metadata(#[from] MetadataRefreshError),
    /// The index refused the record. The rest of the rescan continues.
    #[error("index write failed: {0}")]
    Store(#[from] IndexError),
}

/// One file that could not be reconciled.
///
/// A failure is per file, never per run: the plan and report still account for
/// every other file.
#[derive(Debug, Error)]
#[error("{path}: {reason}")]
pub struct FileFailure {
    path: RelativePdfPath,
    #[source]
    reason: FileFailureReason,
}

impl FileFailure {
    /// The file that failed.
    #[must_use]
    pub const fn path(&self) -> &RelativePdfPath {
        &self.path
    }

    /// Why it failed.
    #[must_use]
    pub const fn reason(&self) -> &FileFailureReason {
        &self.reason
    }
}

/// Two files claim one note ID.
///
/// The conflict is reported with both paths and left unresolved: keeping the
/// indexed record, replacing it, and editing the newcomer's ID are all the
/// user's decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateIdConflict {
    id: NoteId,
    existing: RelativePdfPath,
    conflicting: RelativePdfPath,
}

impl DuplicateIdConflict {
    /// The contested ID.
    #[must_use]
    pub const fn id(&self) -> NoteId {
        self.id
    }

    /// The path that keeps the ID.
    #[must_use]
    pub const fn existing(&self) -> &RelativePdfPath {
        &self.existing
    }

    /// The path that was refused the ID.
    #[must_use]
    pub const fn conflicting(&self) -> &RelativePdfPath {
        &self.conflicting
    }
}

/// A file that is not indexed yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedAddition {
    note: IndexedNote,
}

impl PlannedAddition {
    /// The record that will be inserted.
    #[must_use]
    pub const fn note(&self) -> &IndexedNote {
        &self.note
    }
}

/// A file whose bytes changed at a path already indexed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedUpdate {
    note: IndexedNote,
    previous: PdfFingerprint,
}

impl PlannedUpdate {
    /// The record as it will be after the write.
    #[must_use]
    pub const fn note(&self) -> &IndexedNote {
        &self.note
    }

    /// The fingerprint the record held before this rescan.
    #[must_use]
    pub const fn previous(&self) -> &PdfFingerprint {
        &self.previous
    }
}

/// A file that kept its contents but changed location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedMove {
    note: IndexedNote,
    from: RelativePdfPath,
}

impl PlannedMove {
    /// The record as it will be after the move.
    #[must_use]
    pub const fn note(&self) -> &IndexedNote {
        &self.note
    }

    /// The path the record is leaving.
    #[must_use]
    pub const fn from(&self) -> &RelativePdfPath {
        &self.from
    }
}

/// A record whose file is gone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedDeletion {
    path: RelativePdfPath,
}

impl PlannedDeletion {
    /// The path whose record will be removed.
    #[must_use]
    pub const fn path(&self) -> &RelativePdfPath {
        &self.path
    }
}

/// A file whose record is reused exactly as stored.
///
/// The record is listed so a preview can show what the rescan deliberately left
/// alone. Nothing is written for it, so its OCR text and status survive
/// untouched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnchangedFile {
    note: IndexedNote,
}

impl UnchangedFile {
    /// The record as it is stored.
    #[must_use]
    pub const fn note(&self) -> &IndexedNote {
        &self.note
    }

    /// The path of the untouched record.
    #[must_use]
    pub const fn path(&self) -> &RelativePdfPath {
        self.note.path()
    }

    /// The fingerprint that matched.
    #[must_use]
    pub const fn fingerprint(&self) -> &PdfFingerprint {
        self.note.fingerprint()
    }
}

/// What a rescan would do, computed without writing anything.
///
/// Every enumerated file appears in exactly one of the write categories,
/// [`unchanged`](Self::unchanged), or [`failures`](Self::failures). A file whose
/// note ID is refused appears in [`conflicts`](Self::conflicts) instead of the
/// write categories, and the record that keeps the ID is written so it can show
/// that it is contested.
#[derive(Debug, Default)]
pub struct RescanPlan {
    additions: Vec<PlannedAddition>,
    updates: Vec<PlannedUpdate>,
    moves: Vec<PlannedMove>,
    deletions: Vec<PlannedDeletion>,
    conflicts: Vec<DuplicateIdConflict>,
    unchanged: Vec<UnchangedFile>,
    failures: Vec<FileFailure>,
}

impl RescanPlan {
    /// Files that will be indexed for the first time.
    #[must_use]
    pub fn additions(&self) -> &[PlannedAddition] {
        &self.additions
    }

    /// Files whose content changed at a known path.
    #[must_use]
    pub fn updates(&self) -> &[PlannedUpdate] {
        &self.updates
    }

    /// Files whose content is unchanged but whose path changed.
    #[must_use]
    pub fn moves(&self) -> &[PlannedMove] {
        &self.moves
    }

    /// Records whose file is gone.
    #[must_use]
    pub fn deletions(&self) -> &[PlannedDeletion] {
        &self.deletions
    }

    /// Note IDs claimed by more than one file.
    #[must_use]
    pub fn conflicts(&self) -> &[DuplicateIdConflict] {
        &self.conflicts
    }

    /// Files whose record is reused untouched.
    #[must_use]
    pub fn unchanged(&self) -> &[UnchangedFile] {
        &self.unchanged
    }

    /// Files that could not be reconciled.
    #[must_use]
    pub fn failures(&self) -> &[FileFailure] {
        &self.failures
    }

    /// Whether applying this plan would write anything at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.additions.is_empty()
            && self.updates.is_empty()
            && self.moves.is_empty()
            && self.deletions.is_empty()
    }

    /// Drops every planned write for `path`, leaving the record as stored.
    ///
    /// Used for the loser of a duplicate-ID conflict: it is not indexed at all,
    /// rather than indexed under an ID another record owns.
    fn drop_path(&mut self, path: &RelativePdfPath) {
        self.additions.retain(|item| item.note.path() != path);
        self.updates.retain(|item| item.note.path() != path);
        self.moves.retain(|item| item.note.path() != path);
        self.unchanged.retain(|item| item.note.path() != path);
    }

    /// Marks the record at `path` as contested by a duplicate ID.
    ///
    /// The status is part of the stored record, so a record that would
    /// otherwise have been reused untouched is promoted to a write here:
    /// showing the conflict costs one write, and the alternative — a library
    /// that silently hides a contested ID — is worse.
    fn mark_conflicted(&mut self, path: &RelativePdfPath) {
        for note in self
            .additions
            .iter_mut()
            .map(|item| &mut item.note)
            .chain(self.updates.iter_mut().map(|item| &mut item.note))
            .chain(self.moves.iter_mut().map(|item| &mut item.note))
        {
            if note.path() == path {
                note.set_status(IndexStatus::DuplicateIdConflict);
            }
        }
        if let Some(index) = self
            .unchanged
            .iter()
            .position(|item| item.note.path() == path)
        {
            let mut note = self.unchanged.remove(index).note;
            note.set_status(IndexStatus::DuplicateIdConflict);
            let previous = note.fingerprint().clone();
            self.updates.push(PlannedUpdate { note, previous });
        }
    }
}

/// What applying a plan did.
///
/// The categories match the plan's, so a caller can show both, plus
/// [`failures`](Self::failures) for anything the store refused.
#[derive(Debug, Default)]
pub struct RescanReport {
    additions: Vec<PlannedAddition>,
    updates: Vec<PlannedUpdate>,
    moves: Vec<PlannedMove>,
    deletions: Vec<PlannedDeletion>,
    conflicts: Vec<DuplicateIdConflict>,
    unchanged: Vec<UnchangedFile>,
    failures: Vec<FileFailure>,
}

impl RescanReport {
    /// Records inserted for files seen for the first time.
    #[must_use]
    pub fn additions(&self) -> &[PlannedAddition] {
        &self.additions
    }

    /// Records whose fingerprint was replaced.
    #[must_use]
    pub fn updates(&self) -> &[PlannedUpdate] {
        &self.updates
    }

    /// Records that followed a file to a new path.
    #[must_use]
    pub fn moves(&self) -> &[PlannedMove] {
        &self.moves
    }

    /// Records removed because their file is gone.
    #[must_use]
    pub fn deletions(&self) -> &[PlannedDeletion] {
        &self.deletions
    }

    /// Note IDs that stayed contested.
    #[must_use]
    pub fn conflicts(&self) -> &[DuplicateIdConflict] {
        &self.conflicts
    }

    /// Files reused without a write.
    #[must_use]
    pub fn unchanged(&self) -> &[UnchangedFile] {
        &self.unchanged
    }

    /// Files that could not be indexed.
    #[must_use]
    pub fn failures(&self) -> &[FileFailure] {
        &self.failures
    }

    /// Whether nothing was written.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.additions.is_empty()
            && self.updates.is_empty()
            && self.moves.is_empty()
            && self.deletions.is_empty()
    }
}

/// Failures that stop a whole reconciliation rather than one file.
///
/// A file-level problem is a [`FileFailure`] in the plan or report, never an
/// error here: only the store failing as a whole is.
#[derive(Debug, Error)]
pub enum ReconcileError {
    /// The index could not be read or written.
    #[error(transparent)]
    Index(#[from] IndexError),
    /// The catalog could not be enumerated.
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

/// Plans and applies rescans.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Reconciler;

impl Reconciler {
    /// Compares the indexed records against the enumerated files.
    ///
    /// The store is read but never written: every classification is computed
    /// first, so a caller can display the plan and abandon it without having
    /// touched the index.
    ///
    /// # Errors
    /// Returns [`ReconcileError`] when the index cannot be read. A file whose
    /// metadata cannot be derived becomes a [`FileFailure`] instead.
    pub fn plan(existing: &IndexStore, files: &[PdfFile]) -> Result<RescanPlan, ReconcileError> {
        let mut by_path: HashMap<RelativePdfPath, IndexedNote> = HashMap::new();
        let mut by_hash: HashMap<Sha256Digest, IndexedNote> = HashMap::new();
        for note in existing.list_all()? {
            by_path.insert(note.path().clone(), note.clone());
            by_hash.insert(note.fingerprint().sha256(), note.clone());
        }

        let mut plan = RescanPlan::default();
        for file in files {
            match classify(file, &by_path, &by_hash) {
                Ok(Classification::Addition(note)) => plan.additions.push(PlannedAddition { note }),
                Ok(Classification::Update(note, previous)) => {
                    plan.updates.push(PlannedUpdate { note, previous });
                }
                Ok(Classification::Move(note, from)) => {
                    plan.moves.push(PlannedMove { note, from });
                }
                Ok(Classification::Unchanged(note)) => {
                    plan.unchanged.push(UnchangedFile { note });
                }
                Err(failure) => plan.failures.push(failure),
            }
        }

        // A record whose file was seen keeps its row even when the file moved
        // away from the path it was filed under, so a moved path is claimed and
        // never planned for deletion.
        let mut moved: Vec<&RelativePdfPath> = plan.moves.iter().map(|item| &item.from).collect();
        moved.extend(files.iter().map(PdfFile::path));
        let mut deleted: Vec<RelativePdfPath> = by_path
            .keys()
            .filter(|path| !moved.contains(path))
            .cloned()
            .collect();
        deleted.sort();
        plan.deletions = deleted
            .into_iter()
            .map(|path| PlannedDeletion { path })
            .collect();

        plan.resolve_conflicts();
        Ok(plan)
    }

    /// Performs a plan against the store.
    ///
    /// Writes happen in one order — additions, then updates, then moves — and a
    /// write the store refuses becomes a [`FileFailure`] in the report rather
    /// than aborting the rest, so one record the database rejects cannot
    /// strand the remainder of the rescan. Deletions come last, against the
    /// set of paths that survived, so nothing still on disk is removed.
    ///
    /// # Errors
    /// Returns [`ReconcileError`] when the index cannot be read for the
    /// deletion pass. Every other per-record problem is reported in
    /// [`RescanReport::failures`].
    pub fn apply(plan: RescanPlan, store: &mut IndexStore) -> Result<RescanReport, ReconcileError> {
        let RescanPlan {
            additions,
            updates,
            moves,
            deletions,
            conflicts,
            unchanged,
            failures,
        } = plan;

        let mut report = RescanReport {
            conflicts,
            unchanged,
            failures,
            ..RescanReport::default()
        };

        for addition in &additions {
            if write(store, addition.note(), &mut report.failures) {
                report.additions.push(PlannedAddition {
                    note: addition.note.clone(),
                });
            }
        }
        for update in &updates {
            if write(store, update.note(), &mut report.failures) {
                report.updates.push(PlannedUpdate {
                    note: update.note.clone(),
                    previous: update.previous.clone(),
                });
            }
        }
        let mut moved_from: Vec<RelativePdfPath> = Vec::new();
        for moved in &moves {
            if write(store, moved.note(), &mut report.failures) {
                report.moves.push(PlannedMove {
                    note: moved.note.clone(),
                    from: moved.from.clone(),
                });
            } else {
                // The record did not follow the file, so it is still filed
                // under the path it left and must survive the deletion pass.
                moved_from.push(moved.from.clone());
            }
        }

        if !deletions.is_empty() {
            let present: Vec<RelativePdfPath> = report
                .additions
                .iter()
                .map(|item| item.note.path().clone())
                .chain(report.updates.iter().map(|item| item.note.path().clone()))
                .chain(report.moves.iter().map(|item| item.note.path().clone()))
                .chain(report.unchanged.iter().map(|item| item.path().clone()))
                .chain(moved_from)
                .collect();
            store.delete_missing(&present.into_iter().collect())?;
            report.deletions = deletions;
        }

        Ok(report)
    }
}

impl RescanPlan {
    /// Refuses every newcomer whose note ID an indexed record already claims.
    ///
    /// [`validate_unique_ids`] is the authority on what counts as a duplicate;
    /// this only decides who keeps the contested ID. The record that was
    /// already indexed wins, which is what the ID invariant says happens when a
    /// renamed or imported file collides, and the loser is removed from the
    /// plan so applying it cannot fail on the uniqueness constraint. Each round
    /// removes one candidate, so the loop terminates.
    fn resolve_conflicts(&mut self) {
        loop {
            // Every record the index will hold once the plan is applied: the
            // ones already stored, then the newcomers. A stored record is
            // always listed first, so it is the one the validator names as
            // keeping the ID.
            let mut stored: Vec<&IndexedNote> = self
                .unchanged
                .iter()
                .map(UnchangedFile::note)
                .chain(self.updates.iter().map(PlannedUpdate::note))
                .chain(self.moves.iter().map(PlannedMove::note))
                .collect();
            let mut newcomers: Vec<&IndexedNote> =
                self.additions.iter().map(PlannedAddition::note).collect();
            stored.sort_by(|left, right| left.path().cmp(right.path()));
            newcomers.sort_by(|left, right| left.path().cmp(right.path()));
            stored.append(&mut newcomers);

            let Err(duplicate) = validate_unique_ids(stored.into_iter()) else {
                return;
            };
            let loser = duplicate.conflicting_path().clone();
            self.conflicts.push(DuplicateIdConflict {
                id: duplicate.id(),
                existing: duplicate.existing_path().clone(),
                conflicting: loser.clone(),
            });
            self.mark_conflicted(duplicate.existing_path());
            self.drop_path(&loser);
        }
    }
}

/// Writes one planned record, recording a refusal instead of propagating it.
///
/// Returns whether the record was written, so a caller reports only what
/// actually reached the index.
fn write(store: &IndexStore, note: &IndexedNote, failures: &mut Vec<FileFailure>) -> bool {
    match store.reindex_note(note) {
        Ok(_) => true,
        Err(error) => {
            failures.push(FileFailure {
                path: note.path().clone(),
                reason: FileFailureReason::Store(error),
            });
            false
        }
    }
}

/// The category one enumerated file falls into.
enum Classification {
    Addition(IndexedNote),
    Update(IndexedNote, PdfFingerprint),
    Move(IndexedNote, RelativePdfPath),
    Unchanged(IndexedNote),
}

/// Decides what one file means relative to the index.
///
/// The hash is consulted first because it, not the path, is the file's
/// identity; the path is read only to tell a move from a content change.
fn classify(
    file: &PdfFile,
    by_path: &HashMap<RelativePdfPath, IndexedNote>,
    by_hash: &HashMap<Sha256Digest, IndexedNote>,
) -> Result<Classification, FileFailure> {
    let fail = |reason| FileFailure {
        path: file.path.clone(),
        reason,
    };

    let parsed =
        parse_note_filename(&file.filename).map_err(|error| fail(FileFailureReason::NotAPdf(error)))?;

    // The same bytes are already indexed: this is the same file, wherever it
    // now lives.
    if let Some(record) = by_hash.get(&file.fingerprint.sha256()) {
        if record.path() == &file.path {
            return Ok(Classification::Unchanged(record.clone()));
        }
        let (metadata, _) = refresh_from_parsed(record.metadata(), &parsed)
            .map_err(FileFailureReason::Metadata)
            .map_err(fail)?;
        let mut note = IndexedNote::new(file.path.clone(), file.fingerprint.clone(), metadata);
        // The contents are identical, so the record keeps its pipeline status
        // and, through the store, its OCR text.
        note.set_status(record.status());
        return Ok(Classification::Move(note, record.path().clone()));
    }

    // Known path, new bytes: the record is kept and only its fingerprint and
    // its non-manual metadata are replaced. OCR text read from the previous
    // bytes is dropped by the store, so the record goes back to
    // `Discovered` until the pipeline reads the new content.
    if let Some(record) = by_path.get(&file.path) {
        let (metadata, _) = refresh_from_parsed(record.metadata(), &parsed)
            .map_err(FileFailureReason::Metadata)
            .map_err(fail)?;
        let mut note = IndexedNote::new(file.path.clone(), file.fingerprint.clone(), metadata);
        note.set_status(IndexStatus::Discovered);
        return Ok(Classification::Update(
            note,
            record.fingerprint().clone(),
        ));
    }

    let metadata = metadata_from_filename(parsed)
        .map_err(FileFailureReason::Metadata)
        .map_err(fail)?;
    Ok(Classification::Addition(IndexedNote::new(
        file.path.clone(),
        file.fingerprint.clone(),
        metadata,
    )))
}

