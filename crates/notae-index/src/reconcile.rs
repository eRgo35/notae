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
//! One unreadable, corrupt, or unnameable PDF must not abort a rescan. A file
//! the catalog could not read, whose bytes are not a PDF, or whose metadata
//! cannot be derived becomes a [`FileFailure`] in the plan and is otherwise
//! ignored; every other file is still classified and applied. A failed file
//! that already has a record keeps it, because the file is still on disk — but
//! a record whose file the catalog reported as unreadable, or as not a
//! readable PDF, takes on [`IndexStatus::Inaccessible`] or
//! [`IndexStatus::InvalidPdf`] in the same rescan, because a stored row must
//! not contradict what the catalog just saw.
//!
//! A file that fails produces no record, and therefore reserves nothing: its
//! content hash is only claimed once a record exists for it, so a later file
//! holding the same bytes is indexed on its own merit instead of being
//! reported as a copy of something that was never indexed.
//!
//! # Conflicts
//!
//! Duplicate note IDs are surfaced, never resolved. When two files would claim
//! one ID, the record that is already indexed keeps it, the newcomer keeps its
//! record but is written *without* an ID, and a [`DuplicateIdConflict`] naming
//! both paths is reported so the user can decide. Both records are marked
//! [`IndexStatus::DuplicateIdConflict`], so neither looks healthy, and neither
//! PDF disappears from the library. The marker is dropped again on the first
//! rescan in which no collision remains.
//!
//! A record whose file the catalog *could not use* keeps the status that
//! describes the condition the catalog observed —
//! [`IndexStatus::Inaccessible`] or [`IndexStatus::InvalidPdf`] — instead of
//! having it replaced by the conflict marker. The stored row has one status
//! field, and the condition is the fact the catalog just established; a
//! collision is a fact about the library, not about the file, and it is
//! reported per rescan in [`RescanPlan::conflicts`] (and per file in
//! [`RescanPlan::failures`], which names the condition). Task 5 therefore
//! joins the two by path: a contested path that also appears as
//! [`FileFailureReason::Unreadable`] or [`FileFailureReason::InvalidPdf`] is a
//! record that is both contested and unusable, and must be shown as both.
//!
//! Ownership still decides the ID: a record that already holds an ID keeps it
//! whether or not its file could be read, so "at most one indexed record per
//! numeric ID" holds even while both facts are true at once. Because the
//! conflict is re-derived from the index and the catalog on every rescan
//! rather than read back out of a status, and because a rescan that would
//! write a record identical to the stored one writes nothing at all, an
//! unresolved conflict *and* an observed condition both stay stable.
//!
//! Ownership is decided from the index as it was before the rescan, never from
//! the order paths happen to sort in, so an unresolved conflict reaches the
//! same state on every rescan. `RescanPlan::resolve_conflicts` states the rule
//! in full.
//!
//! See [`crate::provenance`] for how manual edits are preserved.

use std::collections::{HashMap, HashSet};

use notae_domain::{
    FilenameError, IndexStatus, IndexedNote, MetadataField, MetadataOrigin, NoteId, ParsedFilename,
    PdfFingerprint, RelativePdfPath, Sha256Digest, parse_note_filename, validate_unique_ids,
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

/// What a catalog observed when it tried to read one file.
///
/// A catalog enumerates every `.pdf` it can see, whether or not it can read
/// the bytes. What it found is one of these three conditions, so a file that
/// cannot be opened or is not a PDF is *reported* rather than silently absent
/// from the rescan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileCondition {
    /// The file was read end to end and hashed; it can be reconciled.
    Readable(PdfFingerprint),
    /// The file exists but could not be opened or read — denied permission, a
    /// vanished document, or an I/O error. No fingerprint exists, so nothing
    /// about the record's identity changes: it keeps its note ID, its content,
    /// and its OCR text, and takes on [`IndexStatus::Inaccessible`] so the
    /// stored row does not claim a file the catalog could not read. A
    /// [`FileFailure`] is reported either way.
    Unreadable,
    /// The bytes were read but do not form a readable PDF. As with
    /// [`Unreadable`](Self::Unreadable), the record keeps its identity and takes
    /// on [`IndexStatus::InvalidPdf`], and the file is reported.
    InvalidPdf,
}

impl FileCondition {
    /// The fingerprint, when the file was readable.
    #[must_use]
    pub const fn fingerprint(&self) -> Option<&PdfFingerprint> {
        match self {
            Self::Readable(fingerprint) => Some(fingerprint),
            Self::Unreadable | Self::InvalidPdf => None,
        }
    }

    /// Whether the file was read and hashed.
    #[must_use]
    pub const fn is_readable(&self) -> bool {
        matches!(self, Self::Readable(_))
    }
}

/// One enumerated PDF, as the catalog saw it.
///
/// The [`filename`](PdfFile::filename) is the file's own name, kept beside the
/// library-relative [`path`](PdfFile::path) because a provider such as the
/// Storage Access Framework knows a document's display name independently of
/// where Notae files it, and the name is what filename-derived metadata is
/// parsed from. The two can differ, and a refresh compares what the name says
/// against what is stored, so a display-name change is picked up even when the
/// path and the bytes have not moved.
///
/// The [`condition`](PdfFile::condition) carries what the catalog could do with
/// the file. A file that could not be read, or whose bytes are not a PDF, is
/// still yielded here — as [`FileCondition::Unreadable`] or
/// [`FileCondition::InvalidPdf`] — so that reconciliation reports it per file
/// instead of treating it as absent and deleting its record, and so that an
/// existing record's status can be corrected to match what was observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfFile {
    path: RelativePdfPath,
    condition: FileCondition,
    filename: String,
}

impl PdfFile {
    /// Describes an enumerated, readable PDF.
    #[must_use]
    pub const fn new(path: RelativePdfPath, fingerprint: PdfFingerprint, filename: String) -> Self {
        Self::with_condition(path, filename, FileCondition::Readable(fingerprint))
    }

    /// Describes an enumerated PDF whose display name is the last segment of
    /// its library-relative path.
    #[must_use]
    pub fn from_path(path: RelativePdfPath, fingerprint: PdfFingerprint) -> Self {
        Self::from_path_with_condition(path, FileCondition::Readable(fingerprint))
    }

    /// Describes an enumerated PDF in any of the conditions a catalog can
    /// observe, with an explicit display name.
    #[must_use]
    pub const fn with_condition(
        path: RelativePdfPath,
        filename: String,
        condition: FileCondition,
    ) -> Self {
        Self {
            path,
            condition,
            filename,
        }
    }

    /// Describes an enumerated PDF in any of the conditions a catalog can
    /// observe, using the last segment of its library-relative path as its
    /// display name.
    #[must_use]
    pub fn from_path_with_condition(path: RelativePdfPath, condition: FileCondition) -> Self {
        let filename = path
            .as_str()
            .rsplit('/')
            .next()
            .unwrap_or(path.as_str())
            .to_owned();
        Self::with_condition(path, filename, condition)
    }

    /// The library-relative path the record is filed under.
    #[must_use]
    pub const fn path(&self) -> &RelativePdfPath {
        &self.path
    }

    /// The content fingerprint, when the catalog could read the file.
    #[must_use]
    pub const fn fingerprint(&self) -> Option<&PdfFingerprint> {
        self.condition.fingerprint()
    }

    /// What the catalog observed when it tried to read the file.
    #[must_use]
    pub const fn condition(&self) -> &FileCondition {
        &self.condition
    }

    /// Whether the catalog read and hashed the file.
    #[must_use]
    pub const fn is_readable(&self) -> bool {
        self.condition.is_readable()
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
    /// Every PDF the catalog can see must appear in the result, including one
    /// it could not read or could not parse: such a file is yielded with a
    /// non-readable [`FileCondition`] rather than omitted, so reconciliation
    /// reports it as a per-file [`FileFailure`] and leaves any record it
    /// already has untouched. Omitting a file entirely would be
    /// indistinguishable from the file having been deleted.
    ///
    /// # Errors
    /// Returns [`CatalogError`] when the tree cannot be read at all. A file
    /// that cannot be read individually must not fail the whole enumeration.
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError>;
}

/// Why one file could not be reconciled.
///
/// Every variant names a condition the user can act on, and every one of them
/// is scoped to a single file: the rest of the rescan continues.
#[derive(Debug, Error)]
pub enum FileFailureReason {
    /// The catalog could not open or read the file.
    #[error("the file could not be read: grant access to it, or move it out of the library folder")]
    Unreadable,
    /// The bytes were read but are not a readable PDF.
    #[error("the file is not a readable PDF: open it in a PDF viewer, then rescan")]
    InvalidPdf,
    /// The name is not a PDF name.
    #[error("not a PDF filename: {0}")]
    NotAPdf(#[from] FilenameError),
    /// The name parsed, but the metadata it carries is not valid.
    #[error(transparent)]
    Metadata(#[from] MetadataRefreshError),
    /// Another file in this rescan already holds these exact bytes, so there
    /// is only one record for them.
    #[error("identical content is already indexed at {0}; delete one of the two copies")]
    DuplicateContent(RelativePdfPath),
    /// The index refused the record. The rest of the rescan continues.
    #[error("index write failed: {0}")]
    Store(#[from] IndexError),
}

impl FileFailureReason {
    /// The reason a catalog's non-readable condition is reported under.
    ///
    /// Reconciliation maps each non-readable [`FileCondition`] onto the reason
    /// the file is reported under, so a catalog and the rescan report name the
    /// same condition identically. [`FileCondition::Readable`] is
    /// `None`: there is nothing to report about a file that was read.
    #[must_use]
    pub const fn for_condition(condition: &FileCondition) -> Option<Self> {
        match condition {
            FileCondition::Readable(_) => None,
            FileCondition::Unreadable => Some(Self::Unreadable),
            FileCondition::InvalidPdf => Some(Self::InvalidPdf),
        }
    }
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
/// The conflict is reported with both paths and left unresolved. Both records
/// stay in the library and both carry [`IndexStatus::DuplicateIdConflict`];
/// [`existing`](Self::existing) is the one that holds the ID, and
/// [`conflicting`](Self::conflicting) is indexed with no ID at all until the
/// user decides. Keeping the indexed record, editing the newcomer's number, and
/// deleting or renaming one of the files are all the user's decisions.
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

    /// The path whose record was indexed without the contested ID.
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
/// note ID is contested still appears in a write category — it is indexed, but
/// with no ID — and additionally in [`conflicts`](Self::conflicts), so a
/// preview can show the collision without hiding either file.
#[derive(Debug, Default)]
pub struct RescanPlan {
    additions: Vec<PlannedAddition>,
    updates: Vec<PlannedUpdate>,
    moves: Vec<PlannedMove>,
    deletions: Vec<PlannedDeletion>,
    conflicts: Vec<DuplicateIdConflict>,
    unchanged: Vec<UnchangedFile>,
    failures: Vec<FileFailure>,
    /// Records the catalog could not use, which need no write of their own.
    ///
    /// A file that could not be read, or whose bytes are not a PDF, is still a
    /// *record* holding a note ID, so it takes part in duplicate-ID resolution
    /// even when its stored status already says what the catalog observed.
    /// Leaving it out would let a second file claim its number: the newcomer
    /// would be planned as a plain addition and the collision would surface
    /// only as a refused write, hiding the conflict and leaving the second PDF
    /// unindexed.
    observed: Vec<IndexedNote>,
    /// The paths whose file the catalog could not use.
    ///
    /// Their records carry the status that describes the condition the catalog
    /// observed, so [`RescanPlan::mark_contested`] leaves them alone. The
    /// condition stays in the status and the collision is reported through
    /// [`conflicts`](Self::conflicts) — see the module documentation on how the
    /// two facts are represented together.
    conditioned: HashSet<RelativePdfPath>,
    /// Every path the catalog saw on disk, including the ones that failed.
    ///
    /// This is what the deletion pass runs against, not the set of records
    /// that were written: a file the catalog could not read, or whose metadata
    /// could not be derived, is still on disk and must keep its record. It is
    /// also how a contested record — which is indexed, but whose ID was
    /// withheld — survives.
    present: Vec<RelativePdfPath>,
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

    /// Marks the record at `path` as contested by a duplicate ID.
    ///
    /// The status is part of the stored record, so a record that would
    /// otherwise have been reused untouched is promoted to a write here:
    /// showing the conflict costs one write, and the alternative — a library
    /// that silently hides a contested ID — is worse.
    ///
    /// A record whose file the catalog could not use is *not* marked: its
    /// status says what the catalog observed, and the collision is reported
    /// through [`conflicts`](Self::conflicts) and named there by path. Both
    /// facts are then true at once and neither is discarded.
    fn mark_contested(&mut self, path: &RelativePdfPath) {
        if self.conditioned.contains(path) {
            return;
        }
        self.edit_note_at(path, |note| {
            note.with_status(IndexStatus::DuplicateIdConflict)
        });
    }

    /// Withholds the contested note ID from the record at `path`.
    ///
    /// The record stays in the library and keeps its title, its content, and
    /// its fingerprint; only the note number is held back, because two records
    /// cannot carry one ID. The user resolves the collision by editing the
    /// number — which marks the field [`MetadataOrigin::Manual`] and so
    /// survives later rescans — or by renaming or deleting a file. Renaming is
    /// enough on its own: the ID is [`MetadataOrigin::FilenameDerived`], so a
    /// later rescan derives it from the new name and the conflict either
    /// disappears or moves to whichever file still claims it.
    ///
    /// A record whose ID the user set by hand is left exactly as it is: the
    /// write that follows is refused by the store's unique-ID constraint and
    /// reported as a [`FileFailureReason::Store`], which is the honest outcome
    /// for an edit Notae must not discard.
    fn withhold_id(&mut self, path: &RelativePdfPath) {
        self.edit_note_at(path, |note| {
            if note.metadata().provenance().is_manual(MetadataField::Id) {
                return note;
            }
            let mut metadata = note.metadata().clone();
            metadata.assign_id(None, MetadataOrigin::FilenameDerived);
            IndexedNote::new(note.path().clone(), note.fingerprint().clone(), metadata)
                .with_status(note.status())
        });
    }

    /// Replaces the planned record filed at `path` with `edit`'s result.
    ///
    /// A record that was going to be reused untouched — or, for a file the
    /// catalog could not use, a record that needed no write at all — is
    /// promoted to a write first, so an edit is never silently discarded.
    fn edit_note_at(&mut self, path: &RelativePdfPath, edit: impl Fn(IndexedNote) -> IndexedNote) {
        if let Some(item) = self
            .additions
            .iter_mut()
            .find(|item| item.note.path() == path)
        {
            item.note = edit(item.note.clone());
            return;
        }
        if let Some(item) = self
            .updates
            .iter_mut()
            .find(|item| item.note.path() == path)
        {
            item.note = edit(item.note.clone());
            return;
        }
        if let Some(item) = self.moves.iter_mut().find(|item| item.note.path() == path) {
            item.note = edit(item.note.clone());
            return;
        }
        if let Some(index) = self.observed.iter().position(|item| item.path() == path) {
            let note = edit(self.observed.remove(index));
            let previous = note.fingerprint().clone();
            self.updates.push(PlannedUpdate { note, previous });
            return;
        }
        if let Some(index) = self.unchanged.iter().position(|item| item.path() == path) {
            let note = edit(self.unchanged.remove(index).note);
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
        // Which record each note ID belongs to *today*. Read before anything is
        // planned, because it is the state conflict resolution must preserve:
        // a rescan may not hand an ID to a different record than the one the
        // index already filed it under.
        let mut held: HashMap<NoteId, StoredClaim> = HashMap::new();
        for note in existing.list_all()? {
            by_path.insert(note.path().clone(), note.clone());
            by_hash.insert(note.fingerprint().sha256(), note.clone());
            if let Some(id) = note.id() {
                held.insert(
                    id,
                    StoredClaim {
                        path: note.path().clone(),
                        fingerprint: note.fingerprint().sha256(),
                    },
                );
            }
        }

        let mut plan = RescanPlan::default();
        // Files are classified in path order, so which of two identical files
        // claims the content — and therefore which one is reported — is the
        // same on every rescan instead of following the catalog's traversal.
        let mut ordered: Vec<&PdfFile> = files.iter().collect();
        ordered.sort_by(|left, right| left.path.cmp(&right.path));

        let mut claimed: HashMap<Sha256Digest, RelativePdfPath> = HashMap::new();
        for file in ordered {
            if FileFailureReason::for_condition(&file.condition).is_some() {
                // What the catalog saw for this path, so nothing downstream may
                // replace the condition it produced with a conflict marker.
                plan.conditioned.insert(file.path.clone());
            }
            let Verdict {
                write,
                record,
                failure,
            } = classify(file, &by_path, &by_hash, &mut claimed);
            let written = match write {
                Some(Classification::Addition(note)) => {
                    plan.additions.push(PlannedAddition { note });
                    true
                }
                Some(Classification::Update(note, previous)) => {
                    plan.updates.push(PlannedUpdate { note, previous });
                    true
                }
                Some(Classification::Move(note, from)) => {
                    plan.moves.push(PlannedMove { note, from });
                    true
                }
                Some(Classification::Unchanged(note)) => {
                    plan.unchanged.push(UnchangedFile { note });
                    true
                }
                None => false,
            };
            if !written && let Some(record) = record {
                // No write was planned for this record, but it still holds a
                // note ID and must take part in duplicate-ID resolution.
                plan.observed.push(record);
            }
            if let Some(failure) = failure {
                plan.failures.push(failure);
            }
        }

        // Every path the catalog saw is present on disk, whatever happened to its
        // metadata: a file that could not be read or parsed, and a copy whose
        // content another path already claims, are all still there.
        plan.present = files.iter().map(|file| file.path.clone()).collect();
        plan.present.sort();

        // A record whose file was seen keeps its row even when the file moved
        // away from the path it was filed under, so a moved path is claimed and
        // never planned for deletion.
        let mut moved: Vec<&RelativePdfPath> = plan.moves.iter().map(|item| &item.from).collect();
        moved.extend(&plan.present);
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

        plan.resolve_conflicts(&held);
        plan.discard_writes_that_change_nothing(&by_path);
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
            observed: _,
            conditioned: _,
            present,
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
            // Everything the catalog saw is still on disk, whatever happened
            // to its record, so the deletion pass keeps all of it. Only a
            // record whose *file* is gone is removed.
            let mut present: HashSet<RelativePdfPath> = present.into_iter().collect();
            // A record whose file moved but whose write was refused is still
            // filed under the path it left, so that path must survive too.
            present.extend(moved_from);
            store.delete_missing(&present)?;
            report.deletions = deletions;
        }

        Ok(report)
    }
}

/// The record the index filed under one note ID before this rescan.
///
/// Both the path and the content hash are kept, because either one identifies
/// the same record across a rescan: a rename changes the path and keeps the
/// hash, and a content change keeps the path and changes the hash. Matching a
/// candidate on either therefore recognises "this is the record that already
/// had the ID" in both cases.
#[derive(Debug, Clone)]
struct StoredClaim {
    path: RelativePdfPath,
    fingerprint: Sha256Digest,
}

/// Ranks one candidate for a contested ID: 0 if the index already filed this
/// very record under it, 1 otherwise.
///
/// The rank is total on any set of candidates — at most one record can be the
/// stored owner of an ID, and the rest tie-break on path — which is what makes
/// the ownership decision the same on every rescan.
fn rank(note: &IndexedNote, held: &HashMap<NoteId, StoredClaim>) -> u8 {
    let Some(claim) = note.id().and_then(|id| held.get(&id)) else {
        return 1;
    };
    let same_record =
        &claim.path == note.path() || claim.fingerprint == note.fingerprint().sha256();
    u8::from(!same_record)
}

impl RescanPlan {
    /// Reports every note ID that more than one file claims, and withholds the
    /// ID from all but one of them.
    ///
    /// [`validate_unique_ids`] is the authority on what counts as a duplicate;
    /// this only decides who keeps the contested ID, and `held` — the ID
    /// ownership the index had *before* this rescan — is what decides it.
    ///
    /// # The ownership rule
    ///
    /// For a contested ID, exactly one record keeps it, chosen in this order:
    ///
    /// 1. **The record that already held the ID in the index.** If some
    ///    candidate is the very record the store filed under this ID — matched
    ///    by path or by content hash, so it is still recognised after a rename
    ///    or a content change — that record keeps it, whatever its path sorts
    ///    to. A newcomer, a duplicate copy, or a file whose bytes changed never
    ///    takes the ID away from the record that was indexed first.
    /// 2. **Otherwise the smallest library-relative path.** When no candidate
    ///    was indexed under this ID the ID is genuinely up for grabs, and the
    ///    two files that happen to parse to it are indistinguishable to the
    ///    index. Choosing by path makes the choice the same on every rescan
    ///    instead of following the catalog's traversal order, and it names the
    ///    same file in [`DuplicateIdConflict::existing`] every time.
    ///
    /// The rule reads only the index as it was and the candidates' paths, so
    /// it cannot change between two rescans of the same library. That is what
    /// makes an unresolved conflict *stable*: rescanning a file whose name
    /// still yields a taken number reaches the same state, instead of handing
    /// the ID to whichever file happened to sort first on that pass. It also
    /// makes both user-facing ways out of a collision work: rename or delete
    /// the newcomer and its ID leaves the contested number with nobody else
    /// claiming it, or give it a number by hand and its ID becomes
    /// [`MetadataOrigin::Manual`], which a filename refresh may not overwrite —
    /// and a record whose ID is manual is never withheld, so the write cannot
    /// be refused.
    ///
    /// The loser is *not* dropped: it keeps its record and is written with no
    /// ID at all, so the user's PDF stays visible and editable in the library
    /// while the collision is unresolved. Both sides are marked
    /// [`IndexStatus::DuplicateIdConflict`], so neither looks like a healthy
    /// record. Each round takes one ID out of contention, so the loop
    /// terminates.
    fn resolve_conflicts(&mut self, held: &HashMap<NoteId, StoredClaim>) {
        loop {
            // Every record the index will hold once the plan is applied. They
            // are ordered by the ownership rule, so the validator — which names
            // the first record it sees for an ID as the keeper and the next as
            // the loser — agrees with it. A record whose file the catalog could
            // not use is included even when it needs no write of its own: it
            // still holds an ID, so excluding it would let a newcomer claim
            // that ID and hide the collision.
            let mut candidates: Vec<&IndexedNote> = self
                .unchanged
                .iter()
                .map(UnchangedFile::note)
                .chain(self.updates.iter().map(PlannedUpdate::note))
                .chain(self.moves.iter().map(PlannedMove::note))
                .chain(self.additions.iter().map(PlannedAddition::note))
                .chain(self.observed.iter())
                .collect();
            candidates.sort_by(|left, right| {
                rank(left, held)
                    .cmp(&rank(right, held))
                    .then_with(|| left.path().cmp(right.path()))
            });

            let Err(duplicate) = validate_unique_ids(candidates.into_iter()) else {
                return;
            };
            let loser = duplicate.conflicting_path().clone();
            self.conflicts.push(DuplicateIdConflict {
                id: duplicate.id(),
                existing: duplicate.existing_path().clone(),
                conflicting: loser.clone(),
            });
            self.mark_contested(duplicate.existing_path());
            self.mark_contested(&loser);
            self.withhold_id(&loser);
        }
    }

    /// Turns a planned write that would store what is already stored back into
    /// an untouched record.
    ///
    /// Conflict resolution runs after classification, so a record that was
    /// planned as a write can end up identical to what the index already holds
    /// — a contested record whose ID is withheld again, for instance, which is
    /// exactly what the previous rescan stored. Writing it changes nothing but
    /// the row's timestamp, so every rescan of an unresolved conflict would
    /// touch every contested record. Discarding those writes is what makes an
    /// unchanged library cost no writes at all, and so what makes a stored
    /// condition — which a rescan must not contradict — reachable again only
    /// when the catalog observes something new.
    fn discard_writes_that_change_nothing(
        &mut self,
        stored: &HashMap<RelativePdfPath, IndexedNote>,
    ) {
        let mut reused = Vec::new();
        self.updates.retain(|update| {
            if stored.get(update.note().path()) == Some(update.note()) {
                reused.push(UnchangedFile {
                    note: update.note.clone(),
                });
                return false;
            }
            true
        });
        self.unchanged.extend(reused);
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

impl Classification {
    /// The record this classification leaves in the index.
    const fn note(&self) -> &IndexedNote {
        match self {
            Self::Addition(note)
            | Self::Update(note, _)
            | Self::Move(note, _)
            | Self::Unchanged(note) => note,
        }
    }
}

/// What one file means relative to the index, and what is reported about it.
///
/// These are independent: a record whose condition the catalog contradicts is
/// corrected *and* reported, because the file still needs the user's attention
/// and the row must not claim otherwise. A file with nothing to say about it
/// reports nothing, and a file with nothing to write is not written.
struct Verdict {
    /// The record this file contributes to the plan, if any.
    write: Option<Classification>,
    /// The record this file leaves in the index, when it leaves one at all.
    ///
    /// This is what duplicate-ID resolution ranges over, so it is set even when
    /// no write is planned: a record whose stored status already matches the
    /// condition the catalog observed still holds a note ID, and a file that
    /// claims that ID is in conflict with it whether or not the record needs
    /// correcting.
    record: Option<IndexedNote>,
    /// What is reported about this file, if anything.
    failure: Option<FileFailure>,
}

impl Verdict {
    /// A file that becomes a record and is not reported.
    fn written(classification: Classification) -> Self {
        Self {
            record: Some(classification.note().clone()),
            write: Some(classification),
            failure: None,
        }
    }

    /// A file that is reported and produces no record.
    fn failed(failure: FileFailure) -> Self {
        Self {
            write: None,
            record: None,
            failure: Some(failure),
        }
    }

    /// A record that is corrected and reported at the same time.
    fn corrected(classification: Classification, failure: FileFailure) -> Self {
        Self {
            record: Some(classification.note().clone()),
            write: Some(classification),
            failure: Some(failure),
        }
    }

    /// A record the catalog contradicts, which needs no write and is reported.
    ///
    /// The stored row already carries the observed condition, so there is
    /// nothing to correct — but the record is still returned, because it still
    /// holds the note ID another file may be claiming.
    fn observed(record: IndexedNote, failure: FileFailure) -> Self {
        Self {
            write: None,
            record: Some(record),
            failure: Some(failure),
        }
    }
}

/// The status a record must carry when the catalog could not use its file.
///
/// The record is still on disk, so it is not deleted; but a row that says
/// [`IndexStatus::Ready`] about a file the catalog could not read is a lie the
/// user would act on, so the observed condition becomes the status. Observing
/// the same condition twice writes nothing, because the row already says it.
fn observed_status(condition: &FileCondition) -> IndexStatus {
    match condition {
        FileCondition::Unreadable => IndexStatus::Inaccessible,
        FileCondition::InvalidPdf => IndexStatus::InvalidPdf,
        FileCondition::Readable(_) => IndexStatus::Discovered,
    }
}

/// Decides what one file means relative to the index.
///
/// The hash is consulted first because it, not the path, is the file's
/// identity; the path is read only to tell a move from a content change.
///
/// `claimed` holds the fingerprints already reserved *in this rescan*, so two
/// unseen files with identical bytes cannot both be planned as separate
/// records. A fingerprint is reserved only once this file has actually
/// produced a record: a file that fails before that reserves nothing, so a
/// later file holding the same bytes can still be indexed on its own merit.
/// Because the reservation follows the outcome, `claimed` does not depend on
/// the order files are handed over in for the *outcome*, only for which copy is
/// reported.
fn classify(
    file: &PdfFile,
    by_path: &HashMap<RelativePdfPath, IndexedNote>,
    by_hash: &HashMap<Sha256Digest, IndexedNote>,
    claimed: &mut HashMap<Sha256Digest, RelativePdfPath>,
) -> Verdict {
    let fail = |reason| FileFailure {
        path: file.path.clone(),
        reason,
    };

    // The catalog could not use this file. The record it already has keeps its
    // identity, its ID, and its OCR text — but its status must stop claiming a
    // file the catalog could not read — and the file is still reported, because
    // it is on disk and something is wrong with it.
    if let Some(reason) = FileFailureReason::for_condition(&file.condition) {
        return match by_path.get(&file.path) {
            Some(record) => {
                let status = observed_status(&file.condition);
                let failure = fail(reason);
                if record.status() == status {
                    Verdict::observed(record.clone(), failure)
                } else {
                    Verdict::corrected(
                        Classification::Update(
                            record.with_status(status),
                            record.fingerprint().clone(),
                        ),
                        failure,
                    )
                }
            }
            // Nothing is indexed for a file that could not be read, so there
            // is no row to correct and only the report says so.
            None => Verdict::failed(fail(reason)),
        };
    }
    let fingerprint = match &file.condition {
        FileCondition::Readable(fingerprint) => fingerprint.clone(),
        FileCondition::Unreadable | FileCondition::InvalidPdf => {
            unreachable!("the reason above covers every non-readable condition")
        }
    };

    let parsed = match parse_note_filename(&file.filename) {
        Ok(parsed) => parsed,
        Err(error) => return Verdict::failed(fail(FileFailureReason::NotAPdf(error))),
    };

    // These exact bytes are already spoken for in this rescan, by another path
    // that did produce a record. There is one record per content hash, so
    // however many copies exist they cannot be indexed separately.
    if let Some(owner) = claimed.get(&fingerprint.sha256())
        && owner != &file.path
    {
        return Verdict::failed(fail(FileFailureReason::DuplicateContent(owner.clone())));
    }

    let sha256 = fingerprint.sha256();
    match classify_bytes(file, fingerprint, &parsed, by_path, by_hash) {
        Ok(classification) => {
            // The record exists now, so — and only now — these bytes are
            // claimed: a file that produced nothing cannot deny another file
            // its own content.
            claimed.insert(sha256, file.path.clone());
            Verdict::written(classification)
        }
        Err(failure) => Verdict::failed(failure),
    }
}

/// Classifies one file that was read, hashed, and named.
///
/// Split from [`classify`] so that the content reservation happens once, on
/// the one path where a record was actually produced.
fn classify_bytes(
    file: &PdfFile,
    fingerprint: PdfFingerprint,
    parsed: &ParsedFilename,
    by_path: &HashMap<RelativePdfPath, IndexedNote>,
    by_hash: &HashMap<Sha256Digest, IndexedNote>,
) -> Result<Classification, FileFailure> {
    let fail = |reason| FileFailure {
        path: file.path.clone(),
        reason,
    };

    // The same bytes are already indexed: this is the same file, wherever it
    // now lives.
    if let Some(record) = by_hash.get(&fingerprint.sha256()) {
        let (metadata, _) = refresh_from_parsed(record.metadata(), parsed)
            .map_err(FileFailureReason::Metadata)
            .map_err(fail)?;
        if record.path() == &file.path {
            // Same path, same bytes. Only the display name can still have
            // changed what filename-derived metadata *should* be, so the
            // record is reused untouched unless a non-manual value actually
            // differs — a refresh that changes nothing costs no write, and the
            // record's status is only rewritten when it was stale.
            let status = unconflicted(record.status());
            if record.metadata().id() == metadata.id()
                && record.metadata().title() == metadata.title()
                && record.status() == status
            {
                return Ok(Classification::Unchanged(record.clone()));
            }
            let mut note = IndexedNote::new(file.path.clone(), fingerprint, metadata);
            note.set_status(status);
            return Ok(Classification::Update(note, record.fingerprint().clone()));
        }
        let mut note = IndexedNote::new(file.path.clone(), fingerprint, metadata);
        // The contents are identical, so the record keeps its pipeline status
        // and, through the store, its OCR text.
        note.set_status(unconflicted(record.status()));
        return Ok(Classification::Move(note, record.path().clone()));
    }

    // Known path, new bytes: the record is kept and only its fingerprint and
    // its non-manual metadata are replaced. OCR text read from the previous
    // bytes is dropped by the store, so the record goes back to
    // `Discovered` until the pipeline reads the new content.
    if let Some(record) = by_path.get(&file.path) {
        let (metadata, _) = refresh_from_parsed(record.metadata(), parsed)
            .map_err(FileFailureReason::Metadata)
            .map_err(fail)?;
        let mut note = IndexedNote::new(file.path.clone(), fingerprint, metadata);
        note.set_status(IndexStatus::Discovered);
        return Ok(Classification::Update(note, record.fingerprint().clone()));
    }

    let metadata = metadata_from_filename(parsed.clone())
        .map_err(FileFailureReason::Metadata)
        .map_err(fail)?;
    Ok(Classification::Addition(IndexedNote::new(
        file.path.clone(),
        fingerprint,
        metadata,
    )))
}

/// Returns the status a record should carry after this rescan.
///
/// A conflict marker describes the state of the library rather than the state
/// of the file, so it is only correct while a collision exists. Once the
/// competing file is gone the record no longer has a contested ID, and the
/// marker is dropped in favour of [`IndexStatus::Discovered`] — the record is
/// valid again and simply goes back through the pipeline. Its OCR text is
/// keyed by content hash and is untouched by this decision.
///
/// [`IndexStatus::Inaccessible`] and [`IndexStatus::InvalidPdf`] describe a
/// condition the catalog *observed*. A file that was read this time no longer
/// has that condition, so the record goes back to
/// [`IndexStatus::Discovered`] for the same reason — the stored row must not
/// contradict what the catalog just saw. The statuses that describe work in
/// progress or a completed run are left alone, because a rescan does not
/// observe them either way.
fn unconflicted(status: IndexStatus) -> IndexStatus {
    match status {
        IndexStatus::DuplicateIdConflict | IndexStatus::Inaccessible | IndexStatus::InvalidPdf => {
            IndexStatus::Discovered
        }
        IndexStatus::Discovered
        | IndexStatus::Indexing
        | IndexStatus::Ready
        | IndexStatus::OcrPending
        | IndexStatus::OcrIncomplete
        | IndexStatus::Stale
        | IndexStatus::ImportMismatch => status,
    }
}
