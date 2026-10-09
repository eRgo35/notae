//! Previewing and applying a PC sidecar against the local library.
//!
//! Importing a sidecar is two steps, and the split is the whole point:
//! [`SidecarImporter::preview`] reads the local index and a
//! [`SidecarManifest`](crate::SidecarManifest) and produces an
//! [`ImportPreview`] without touching a single row, and
//! [`SidecarImporter::apply`] performs that preview afterwards. The Android app
//! renders the preview, the user confirms, and only then is anything written —
//! a sidecar is never applied automatically.
//!
//! # Validation happens before mutation
//!
//! The manifest is validated in full before any classification runs: its schema
//! version must be one this build understands, unknown fields are refused
//! outright, no path and no content hash may appear twice, and every value is a
//! validated domain type by construction. A manifest that fails validation stops
//! the import at [`SidecarImporter::preview`], so there is no state in which the
//! local library has been partly written from a manifest that was then
//! rejected.
//!
//! # Hashes are verified against the files, not against the index
//!
//! The store only knows what was fingerprinted at the last rescan. A file that
//! changed since then still looks like a match, so comparing a sidecar entry
//! against the store would write the PC's metadata and OCR text over bytes that
//! no longer hold them. [`SidecarImporter::preview`] therefore takes a
//! [`PdfCatalog`] — the same abstraction [`crate::Reconciler`] uses — and checks
//! every entry against the file actually in the library root, which on Android is
//! a Storage Access Framework tree walk.
//!
//! Three things can come out of that check, and all three are reported and
//! applied to nothing:
//!
//! - The file is not in the library at all ([`EntryChange::Missing`]) — no
//!   record is created for bytes nobody has.
//! - The catalog could not read or parse it ([`EntryChange::Unverified`]) — no
//!   hash means nothing was verified, and unverified is not unchanged.
//! - The file hashes differently ([`EntryChange::HashMismatch`]) — the file
//!   changed after the PC wrote the sidecar, so the next manual rescan
//!   re-fingerprints it and the two sides agree again.
//!
//! `preview` itself stays read-only: the catalog is enumerated once, and the
//! result is kept in the snapshot so an override re-derives without re-reading
//! either the index or the tree.
//!
//! # Manual metadata wins by default
//!
//! [`NoteMetadata`] tracks the origin of every field, and that is what an import
//! consults. A field the user edited by hand is reported in
//! [`MetadataImport::preserved`] and keeps its value; a field that was not
//! edited takes the sidecar's value and is reported in
//! [`MetadataImport::imported`]. The user can override this per entry and per
//! field with [`ImportPreview::with_override`], which produces a *new* preview —
//! the original stays exactly as it was computed, so declining to override is
//! the default and overriding is always an explicit act.
//!
//! # OCR
//!
//! OCR text is replaced only where it is missing or out of date. An entry whose
//! text equals the stored text is [`OcrAssessment::Current`] and costs no write;
//! one whose text differs replaces the stored text ([`OcrAssessment::Importing`]);
//! one that carries no text leaves the stored text alone
//! ([`OcrAssessment::NotOffered`]) and never blanks it.
//!
//! # Duplicate IDs
//!
//! Conflicts are resolved with the same ownership rule reconciliation uses: the
//! record that already holds an ID in the index keeps it, and a newcomer
//! claiming a taken ID is written without one. A manual ID is never withheld —
//! that is a decision the user made — so the store refuses the write and it is
//! reported as an [`ImportFailure`] rather than silently dropped. Every conflict
//! is reported with both paths in [`ImportPreview::conflicts`], and both records
//! take on [`IndexStatus::DuplicateIdConflict`], so neither looks healthy. That
//! includes an incumbent the sidecar does not mention: it is not one of
//! [`ImportPreview::entries`], so it is listed in
//! [`ImportPreview::contested_records`] and written by `apply`, and a library
//! can never end up with one side of a collision still marked healthy. It also
//! includes an incumbent whose sidecar entry could not be verified: the entry
//! contributes nothing, but the record is still in the library and still holds
//! its number, so it competes for it and the collision is resolved rather than
//! surfacing as a store refusal on the newcomer.
//!
//! A record the catalog could not use is the one exception to the marker. Its
//! status says what was observed of the file — [`IndexStatus::Inaccessible`] or
//! [`IndexStatus::InvalidPdf`] — and that observation is not overwritten. Both
//! facts are then true at once: the row carries the condition, and the
//! collision is reported through [`ImportPreview::conflicts`] and
//! [`ImportPreview::conflict_for`], which name the record as one of the two
//! paths. Reconciliation applies the same rule, so a record cannot be marked by
//! one path and left unmarked by the other.
//!
//! Import never deletes: a record the sidecar does not mention is reported in
//! [`ImportPreview::missing`] and left exactly as it is.
//!
//! See [`crate::sidecar`] for the manifest format itself.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use notae_domain::{
    IndexStatus, IndexedNote, MetadataField, MetadataOrigin, NoteId, NoteMetadata, RelativePdfPath,
    Sha256Digest, validate_unique_ids,
};

use crate::reconcile::{FileCondition, PdfCatalog};
use crate::sidecar::{SidecarEntry, SidecarError, SidecarManifest};
use crate::store::{IndexError, IndexStore, OcrText};

/// How one sidecar entry relates to what the local library already holds.
///
/// The variants are the sections the import preview shows. Each entry is in
/// exactly one of them, and the four write categories — new, changed, unchanged,
/// and mismatch — correspond to [`ImportPreview::new_entries`],
/// [`ImportPreview::changed_entries`], [`ImportPreview::unchanged_entries`], and
/// [`ImportPreview::mismatches`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryChange {
    /// The library does not index this content; the record is inserted.
    New,
    /// The entry matches the stored record and changes nothing at all.
    ///
    /// Nothing is written: the record's OCR text, its status, and its row all
    /// stay as they are.
    Unchanged,
    /// The record will be written because its metadata, its OCR text, or its
    /// pipeline status changes.
    Changed,
    /// The same content is indexed under a different path.
    ///
    /// File identity is the SHA-256 of the bytes, so a rename moves the one
    /// existing record instead of creating a second note. The record keeps its
    /// row identifier and its OCR text.
    Moved {
        /// Path the record was indexed under before this import.
        from: RelativePdfPath,
    },
    /// The file at this path hashes differently from the entry.
    ///
    /// The entry is reported and applied to nothing: its fingerprint describes
    /// bytes the phone no longer has. The stored record keeps its identity, its
    /// metadata, and its OCR text.
    HashMismatch {
        /// Hash the local file at this path actually has.
        local: Sha256Digest,
    },
    /// The library root does not contain the file this entry names.
    ///
    /// The entry is reported and applied to nothing. Creating a record for a
    /// file that is not there would index bytes nobody can open, so the
    /// difference is surfaced instead: copy the file across, or drop the entry.
    Missing,
    /// The catalog could not read or parse the file this entry names.
    ///
    /// Verification needs a hash of the local bytes, and an unreadable file
    /// yields none, so the entry is reported and applied to nothing. Treating
    /// "no hash" as "no difference" would import the PC's metadata and OCR text
    /// for a file whose contents nobody has confirmed.
    Unverified {
        /// What the catalog observed about the file.
        condition: FileCondition,
    },
}

impl EntryChange {
    /// Whether applying this entry writes to the index.
    #[must_use]
    pub const fn writes(&self) -> bool {
        matches!(
            self,
            Self::New | Self::Changed | Self::Moved { .. }
        )
    }
}

/// What will happen to one entry's OCR text.
///
/// OCR text is only ever replaced, never blanked: a sidecar that carries no text
/// says nothing about whether the file has one, so the stored text stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrAssessment {
    /// The sidecar's text will be written, replacing missing or stale local
    /// text.
    Importing,
    /// The stored text already equals the sidecar's; nothing is written.
    Current,
    /// The entry carries no OCR text; the stored text is kept untouched.
    NotOffered,
    /// The entry is a hash mismatch, so it contributes nothing at all.
    NotApplied,
}

/// Which metadata fields an import takes from the sidecar and which it keeps.
///
/// The lists are what the import screen shows per entry: the values the user
/// edited and that will survive, and the values the sidecar proposes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetadataImport {
    imported: Vec<MetadataField>,
    preserved: Vec<MetadataField>,
    overridden: Vec<MetadataField>,
}

impl MetadataImport {
    /// The fields whose value will be replaced with the sidecar's.
    #[must_use]
    pub fn imported(&self) -> &[MetadataField] {
        &self.imported
    }

    /// The fields that keep their local value because the user edited them.
    ///
    /// A field is listed whether or not the sidecar proposes a different value:
    /// the point of the section is to show which fields are the user's, so an
    /// import cannot quietly claim them later.
    #[must_use]
    pub fn preserved(&self) -> &[MetadataField] {
        &self.preserved
    }

    /// The fields the user explicitly chose to replace anyway.
    ///
    /// A field listed here was manually edited locally and the sidecar's value
    /// is being applied over it on purpose.
    #[must_use]
    pub fn overridden(&self) -> &[MetadataField] {
        &self.overridden
    }
}

/// One sidecar entry as the import will treat it.
#[derive(Debug, Clone)]
pub struct ImportEntry {
    entry: SidecarEntry,
    change: EntryChange,
    /// The record this entry matches locally, if any.
    local: Option<IndexedNote>,
    /// The record as it will be stored, when the entry is applied.
    resolved: Option<IndexedNote>,
    metadata: MetadataImport,
    ocr: OcrAssessment,
    conflict: Option<ImportConflict>,
}

impl ImportEntry {
    /// Returns the entry as the sidecar describes it.
    #[must_use]
    pub const fn sidecar(&self) -> &SidecarEntry {
        &self.entry
    }

    /// Returns the path the entry is filed under.
    #[must_use]
    pub const fn path(&self) -> &RelativePdfPath {
        self.entry.path()
    }

    /// Returns how this entry relates to the local library.
    #[must_use]
    pub const fn change(&self) -> &EntryChange {
        &self.change
    }

    /// Returns the record this entry matches locally, when it matches one.
    #[must_use]
    pub const fn local(&self) -> Option<&IndexedNote> {
        self.local.as_ref()
    }

    /// Returns the record as it will be stored, or `None` for a hash mismatch,
    /// which is applied to nothing.
    #[must_use]
    pub const fn resolved(&self) -> Option<&IndexedNote> {
        self.resolved.as_ref()
    }

    /// Returns what will happen to the metadata.
    #[must_use]
    pub const fn metadata(&self) -> &MetadataImport {
        &self.metadata
    }

    /// Returns what will happen to the OCR text.
    #[must_use]
    pub const fn ocr(&self) -> OcrAssessment {
        self.ocr
    }

    /// Returns the duplicate-ID conflict this entry takes part in, if any.
    #[must_use]
    pub const fn conflict(&self) -> Option<&ImportConflict> {
        self.conflict.as_ref()
    }
}

/// Two records claim one note ID.
///
/// The conflict is reported with both paths and left unresolved. Both records
/// stay in the library and both carry [`IndexStatus::DuplicateIdConflict`];
/// `existing` keeps the ID and `conflicting` is written without one until the
/// user decides. Keeping the indexed record, editing the newcomer's number, and
/// deleting or renaming one of the files are all the user's decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportConflict {
    id: NoteId,
    existing: RelativePdfPath,
    conflicting: RelativePdfPath,
}

impl ImportConflict {
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

    /// The path whose record is indexed without the contested ID.
    #[must_use]
    pub const fn conflicting(&self) -> &RelativePdfPath {
        &self.conflicting
    }
}

/// One record the index refused to write.
///
/// A refusal is per record, never per import: one record the database rejects
/// must not strand the rest of the sidecar.
#[derive(Debug, thiserror::Error)]
#[error("{path}: {reason}")]
pub struct ImportFailure {
    path: RelativePdfPath,
    reason: IndexError,
}

impl ImportFailure {
    /// The record that was refused.
    #[must_use]
    pub const fn path(&self) -> &RelativePdfPath {
        &self.path
    }

    /// Why the index refused it.
    #[must_use]
    pub const fn reason(&self) -> &IndexError {
        &self.reason
    }
}

/// What importing a sidecar would do, computed without writing anything.
///
/// The preview owns the snapshot it was computed from — the local records and
/// their stored OCR text — so an override can be applied
/// ([`ImportPreview::with_override`]) by re-deriving everything from that
/// snapshot rather than by reading the index again. What `apply` writes is
/// therefore exactly what the preview showed.
#[derive(Debug, Clone)]
pub struct ImportPreview {
    manifest: SidecarManifest,
    local: Vec<IndexedNote>,
    ocr: HashMap<Sha256Digest, OcrText>,
    /// The catalog's observation of each file, so an override re-verifies the
    /// same bytes rather than re-reading the library root.
    files: HashMap<RelativePdfPath, FileCondition>,
    overrides: BTreeMap<RelativePdfPath, BTreeSet<MetadataField>>,
    entries: Vec<ImportEntry>,
    missing: Vec<IndexedNote>,
    conflicts: Vec<ImportConflict>,
    /// Local records the sidecar omits that take part in a conflict, in the
    /// form they will be stored.
    contested: Vec<IndexedNote>,
}

impl ImportPreview {
    /// Returns every entry, in ascending path order.
    #[must_use]
    pub fn entries(&self) -> &[ImportEntry] {
        &self.entries
    }

    /// Returns the entries that will be inserted.
    #[must_use]
    pub fn new_entries(&self) -> Vec<&ImportEntry> {
        self.filtered(|change| matches!(change, EntryChange::New))
    }

    /// Returns the entries that will be written: metadata and OCR imports, and
    /// moves.
    #[must_use]
    pub fn changed_entries(&self) -> Vec<&ImportEntry> {
        self.filtered(|change| match change {
            EntryChange::Changed | EntryChange::Moved { .. } => true,
            EntryChange::New
            | EntryChange::Unchanged
            | EntryChange::HashMismatch { .. }
            | EntryChange::Missing
            | EntryChange::Unverified { .. } => false,
        })
    }

    /// Returns the entries that match the stored record exactly.
    #[must_use]
    pub fn unchanged_entries(&self) -> Vec<&ImportEntry> {
        self.filtered(|change| matches!(change, EntryChange::Unchanged))
    }

    /// Returns the entries whose hash does not match the local file.
    #[must_use]
    pub fn mismatches(&self) -> Vec<&ImportEntry> {
        self.filtered(|change| matches!(change, EntryChange::HashMismatch { .. }))
    }

    /// Returns the entries the library root does not contain.
    ///
    /// These are reported rather than imported: a sidecar describes files the PC
    /// had, and the phone only owns the ones it actually has. Nothing is
    /// created for a file that is not in the library.
    #[must_use]
    pub fn absent_entries(&self) -> Vec<&ImportEntry> {
        self.filtered(|change| matches!(change, EntryChange::Missing))
    }

    /// Returns the entries whose file the catalog could not read.
    ///
    /// An entry is only as trustworthy as the hash it was checked against. A
    /// file the catalog could not open or parse yields no hash, so the entry
    /// cannot be verified and is reported here instead of being imported on the
    /// strength of a hash nobody confirmed.
    #[must_use]
    pub fn unreadable_entries(&self) -> Vec<&ImportEntry> {
        self.filtered(|change| matches!(change, EntryChange::Unverified { .. }))
    }

    /// Returns the records the sidecar does not mention.
    ///
    /// Import never deletes. A record the sidecar omits was probably added on
    /// the phone after the PC ran, so it is listed for the user to see and left
    /// exactly as it is.
    #[must_use]
    pub fn missing(&self) -> &[IndexedNote] {
        &self.missing
    }

    /// Returns every note ID more than one record claims.
    #[must_use]
    pub fn conflicts(&self) -> &[ImportConflict] {
        &self.conflicts
    }

    /// Returns the records the sidecar omits that are taking part in a conflict.
    ///
    /// A record the manifest does not mention still holds whatever note ID it
    /// holds, so it still competes for that number. When a newcomer claims it,
    /// the incumbent takes on [`IndexStatus::DuplicateIdConflict`] too — and
    /// because it is not an entry, it would otherwise be reported as a conflict
    /// and then silently left healthy. This is what shows the user both sides of
    /// a collision before anything is written.
    ///
    /// A record the catalog could not read or parse is not in this list even
    /// when it takes part in a conflict: its status keeps the observed
    /// condition and needs no write, so it is reported through
    /// [`conflicts`](Self::conflicts) instead of being rewritten.
    #[must_use]
    pub fn contested_records(&self) -> &[IndexedNote] {
        &self.contested
    }

    /// Returns the conflict a given path takes part in.
    #[must_use]
    pub fn conflict_for(&self, path: &RelativePdfPath) -> Option<&ImportConflict> {
        self.conflicts
            .iter()
            .find(|conflict| conflict.existing() == path || conflict.conflicting() == path)
    }

    /// Returns a preview in which the sidecar replaces `fields` on one entry,
    /// even where the user edited them.
    ///
    /// The replacement is recorded as [`MetadataOrigin::Manual`], because an
    /// explicit choice made in Notae is a manual edit and must survive the next
    /// import exactly like any other.
    ///
    /// The receiver is a new preview: the original stays as it was computed, so
    /// an override is always something the user asked for and never a side
    /// effect of showing the preview.
    ///
    /// # Errors
    /// Returns [`SidecarError::UnknownEntry`] when the manifest has no entry at
    /// `path`.
    pub fn with_override(
        &self,
        path: &RelativePdfPath,
        fields: impl IntoIterator<Item = MetadataField>,
    ) -> Result<Self, SidecarError> {
        if self.manifest.entry(path).is_none() {
            return Err(SidecarError::UnknownEntry(path.clone()));
        }
        let mut overrides = self.overrides.clone();
        overrides.entry(path.clone()).or_default().extend(fields);
        derive(
            self.manifest.clone(),
            self.files.clone(),
            self.local.clone(),
            self.ocr.clone(),
            overrides,
        )
    }

    /// Whether applying this preview would write anything.
    #[must_use]
    pub fn writes(&self) -> bool {
        self.entries.iter().any(|entry| entry.change.writes())
    }

    fn filtered(&self, keep: impl Fn(&EntryChange) -> bool) -> Vec<&ImportEntry> {
        self.entries
            .iter()
            .filter(|entry| keep(entry.change()))
            .collect()
    }
}

/// What applying an import did.
///
/// The categories are the preview's, so a caller can show what it promised and
/// what the index actually accepted, plus [`failures`](Self::failures) for the
/// records it refused.
#[derive(Debug, Default)]
pub struct ImportReport {
    imported: Vec<ImportEntry>,
    unchanged: Vec<ImportEntry>,
    mismatches: Vec<ImportEntry>,
    absent: Vec<ImportEntry>,
    unreadable: Vec<ImportEntry>,
    missing: Vec<IndexedNote>,
    conflicts: Vec<ImportConflict>,
    contested: Vec<IndexedNote>,
    failures: Vec<ImportFailure>,
}

impl ImportReport {
    /// The records that were written.
    #[must_use]
    pub fn imported(&self) -> &[ImportEntry] {
        &self.imported
    }

    /// The entries that needed no write.
    #[must_use]
    pub fn unchanged(&self) -> &[ImportEntry] {
        &self.unchanged
    }

    /// The entries reported as hash mismatches.
    #[must_use]
    pub fn mismatches(&self) -> &[ImportEntry] {
        &self.mismatches
    }

    /// Returns the entries the library root did not contain.
    #[must_use]
    pub fn absent_entries(&self) -> &[ImportEntry] {
        &self.absent
    }

    /// Returns the entries whose local file the catalog could not read.
    #[must_use]
    pub fn unreadable_entries(&self) -> &[ImportEntry] {
        &self.unreadable
    }

    /// The records the sidecar did not mention, left untouched.
    #[must_use]
    pub fn missing(&self) -> &[IndexedNote] {
        &self.missing
    }

    /// The note IDs that stayed contested.
    #[must_use]
    pub fn conflicts(&self) -> &[ImportConflict] {
        &self.conflicts
    }

    /// Returns the records the sidecar omits that were written with
    /// [`IndexStatus::DuplicateIdConflict`].
    #[must_use]
    pub fn contested_records(&self) -> &[IndexedNote] {
        &self.contested
    }

    /// The records the index refused.
    #[must_use]
    pub fn failures(&self) -> &[ImportFailure] {
        &self.failures
    }

    /// Whether nothing was written.
    ///
    /// Both write categories count. An entry written through
    /// [`imported`](Self::imported) is one the preview called a write, and a
    /// contested incumbent written through
    /// [`contested_records`](Self::contested_records) is a status mutation the
    /// entries do not account for: the sidecar omits that record, so there is
    /// no entry for it to be counted in. Leaving it out would report an import
    /// that marked both sides of a collision as having done nothing, and hide
    /// any failure the same import refused along the way.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.imported.is_empty() && self.contested.is_empty()
    }
}

/// Previews and applies PC sidecars against a local index.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SidecarImporter;

impl SidecarImporter {
    /// Compares a manifest against the local index without writing anything.
    ///
    /// `catalog` is how the importer learns the hashes of the files actually in
    /// the library. The store only knows what was fingerprinted at the last
    /// rescan, which is a snapshot: a file that changed since then still looks
    /// like a match, and importing the sidecar would write the PC's metadata and
    /// OCR text over bytes that no longer hold it. The [`PdfCatalog`] is the same
    /// abstraction reconciliation uses, so on Android this is a Storage Access
    /// Framework tree walk and on the desktop a directory walk, and this crate
    /// still knows nothing about how files are read.
    ///
    /// The index and the catalog are each read once; every classification,
    /// metadata merge, conflict resolution, and OCR decision is computed from
    /// those two snapshots, which is also what lets
    /// [`ImportPreview::with_override`] re-derive a preview without touching
    /// either again. Entries are processed in ascending path order, so which of
    /// two records keeps a contested ID never depends on the order the manifest
    /// happened to be written in.
    ///
    /// # Errors
    /// Returns [`SidecarError::Catalog`] when the library root cannot be
    /// enumerated at all, [`SidecarError::Store`] when the index cannot be read,
    /// and whatever [`SidecarManifest::validate`] reports for a manifest that
    /// must be rejected before any mutation.
    pub fn preview(
        local: &IndexStore,
        sidecar: &SidecarManifest,
        catalog: &dyn PdfCatalog,
    ) -> Result<ImportPreview, SidecarError> {
        sidecar.validate()?;
        let records = local.list_all()?;
        let ocr = records
            .iter()
            .filter_map(|note| {
                local
                    .ocr_text(&note.fingerprint().sha256())
                    .ok()
                    .flatten()
                    .map(|text| (note.fingerprint().sha256(), text))
            })
            .collect();
        let files = catalog
            .enumerate()?
            .into_iter()
            .map(|file| (file.path().clone(), file.condition().clone()))
            .collect();
        derive(sidecar.clone(), files, records, ocr, BTreeMap::new())
    }

    /// Performs the changes a preview describes.
    ///
    /// Writes go in ascending path order, one record per call, and a record the
    /// index refuses becomes an [`ImportFailure`] in the report rather than
    /// aborting the rest — a single manual-ID collision must not strand the
    /// whole import. The contested records in
    /// [`ImportReport::contested_records`] are written too: an incumbent the
    /// sidecar omits is not one of `preview.entries`, so both sides of a
    /// conflict carrying the marker depends on this. Nothing is deleted: the
    /// records in [`ImportReport::missing`] are reported and left alone.
    ///
    /// # Errors
    /// Returns [`SidecarError::Store`] when the index cannot be written. Every
    /// per-record problem is reported in [`ImportReport::failures`].
    pub fn apply(preview: ImportPreview, local: &IndexStore) -> Result<ImportReport, SidecarError> {
        let mut report = ImportReport {
            conflicts: preview.conflicts.clone(),
            missing: preview.missing.clone(),
            ..ImportReport::default()
        };
        for entry in &preview.entries {
            match entry.change() {
                EntryChange::HashMismatch { .. } => {
                    report.mismatches.push(entry.clone());
                }
                EntryChange::Missing => report.absent.push(entry.clone()),
                EntryChange::Unverified { .. } => report.unreadable.push(entry.clone()),
                EntryChange::Unchanged => report.unchanged.push(entry.clone()),
                EntryChange::New | EntryChange::Changed | EntryChange::Moved { .. } => {
                    // A store refusal is per record, so one contested manual ID
                    // leaves the rest of the sidecar applied.
                    let ocr = match entry.ocr() {
                        OcrAssessment::Importing => entry.entry.ocr_text(),
                        OcrAssessment::Current
                        | OcrAssessment::NotOffered
                        | OcrAssessment::NotApplied => None,
                    };
                    let Some(note) = entry.resolved() else {
                        continue;
                    };
                    match local.reindex_note_with_ocr(note, ocr.as_ref()) {
                        Ok(_) => report.imported.push(entry.clone()),
                        Err(error) => report.failures.push(ImportFailure {
                            path: entry.path().clone(),
                            reason: error,
                        }),
                    }
                }
            }
        }
        for note in &preview.contested {
            // Only the status changes, so the write carries no OCR text and the
            // stored text stands.
            match local.reindex_note(note) {
                Ok(_) => report.contested.push(note.clone()),
                Err(error) => report.failures.push(ImportFailure {
                    path: note.path().clone(),
                    reason: error,
                }),
            }
        }
        Ok(report)
    }
}

/// The index's ID ownership before an import, keyed by note ID.
///
/// Ownership decides a contested ID exactly as it does in reconciliation: the
/// record already filed under an ID keeps it, matched by path *or* by content
/// hash so a rename or a content change is still recognized as the same record.
#[derive(Debug, Clone)]
struct HeldClaim {
    path: RelativePdfPath,
    fingerprint: Sha256Digest,
}

/// One record that will exist once the import is applied.
///
/// A candidate is either a record an entry writes or a local record the sidecar
/// does not touch. Both take part in duplicate-ID resolution: a record the
/// sidecar omits still holds its number, so excluding it would let a newcomer
/// claim an ID that is already taken.
#[derive(Debug, Clone)]
struct Candidate {
    note: IndexedNote,
    /// Index into the entry list, or `None` for a record the sidecar omits or
    /// cannot import.
    entry: Option<usize>,
    /// Whether the catalog could not use this record's file on this run.
    ///
    /// Such a record's status says what was observed of the file, so it is not
    /// overwritten with a conflict marker; the collision it takes part in is
    /// reported through [`ImportConflict`] instead. This is the same rule
    /// reconciliation applies to a record it could not re-index.
    conditioned: bool,
}

/// Ranks a candidate for a contested ID: 0 if the index already filed this very
/// record under it, 1 otherwise.
///
/// The rank is total on any set of candidates — at most one record can be the
/// stored owner of an ID, and the rest tie-break on path — which is what makes
/// the ownership decision the same on every import.
fn rank(note: &IndexedNote, held: &BTreeMap<NoteId, HeldClaim>) -> u8 {
    let Some(claim) = note.id().and_then(|id| held.get(&id)) else {
        return 1;
    };
    let same_record =
        &claim.path == note.path() || claim.fingerprint == note.fingerprint().sha256();
    u8::from(!same_record)
}

/// Recomputes the whole preview from a manifest, the catalog's observation of
/// the library root, the local snapshot, the stored OCR text, and the fields
/// the user has chosen to override.
///
/// Verification is decided from `files`, the bytes on the phone, and not from
/// `local`, the index as it was last rescan. The two disagree exactly when a
/// file changed since the last scan, which is the case that must not be
/// imported.
fn derive(
    manifest: SidecarManifest,
    files: HashMap<RelativePdfPath, FileCondition>,
    local: Vec<IndexedNote>,
    ocr: HashMap<Sha256Digest, OcrText>,
    overrides: BTreeMap<RelativePdfPath, BTreeSet<MetadataField>>,
) -> Result<ImportPreview, SidecarError> {
    let mut by_path: HashMap<&RelativePdfPath, &IndexedNote> = HashMap::new();
    let mut by_hash: HashMap<Sha256Digest, &IndexedNote> = HashMap::new();
    let mut held: BTreeMap<NoteId, HeldClaim> = BTreeMap::new();
    for note in &local {
        by_path.insert(note.path(), note);
        by_hash.insert(note.fingerprint().sha256(), note);
        if let Some(id) = note.id() {
            held.insert(
                id,
                HeldClaim {
                    path: note.path().clone(),
                    fingerprint: note.fingerprint().sha256(),
                },
            );
        }
    }

    let mut ordered: Vec<&SidecarEntry> = manifest.entries().iter().collect();
    ordered.sort_by(|left, right| left.path().cmp(right.path()));

    let mut entries: Vec<ImportEntry> = Vec::with_capacity(ordered.len());
    let mut candidates: Vec<Candidate> = Vec::new();
    // Paths and content hashes the import takes over, so a record the sidecar
    // absorbs is neither reported as missing nor weighed into conflicts twice.
    let mut superseded: BTreeSet<RelativePdfPath> = BTreeSet::new();

    for entry in ordered {
        let overrides = overrides.get(entry.path()).cloned().unwrap_or_default();
        let stored = by_path.get(entry.path()).copied();

        // Verification is against the file in the library, before anything else
        // is considered. A file the phone does not have cannot be imported at
        // all; one it could not read yields no hash to check the entry against;
        // and one whose bytes differ from the entry's describes a file that
        // changed after the PC wrote the sidecar. All three contribute nothing —
        // no metadata, no OCR text, no record — and are reported instead.
        if let Err(change) = verify(entry, files.get(entry.path())) {
            superseded.insert(entry.path().clone());
            // The entry contributes no candidate, but the record the index
            // already holds under this path is still there and still holds
            // whatever number it holds. Dropping it would let a newcomer claim
            // that number and be refused by the store's unique-ID constraint
            // instead of being resolved by the conflict resolution below.
            if let Some(stored) = stored {
                candidates.push(Candidate {
                    note: stored.clone(),
                    entry: None,
                    conditioned: matches!(change, EntryChange::Unverified { .. }),
                });
            }
            entries.push(ImportEntry {
                entry: entry.clone(),
                change,
                local: stored.cloned(),
                resolved: None,
                metadata: MetadataImport {
                    preserved: stored
                        .map_or_else(Vec::new, |local| manual_fields(local.metadata())),
                    ..MetadataImport::default()
                },
                ocr: OcrAssessment::NotApplied,
                conflict: None,
            });
            continue;
        }

        // The same content indexed elsewhere is a rename: the record follows
        // the file rather than becoming a second note.
        let moved_from = match stored {
            Some(_) => None,
            None => by_hash
                .get(&entry.fingerprint().sha256())
                .map(|local| local.path().clone()),
        };
        let local = stored.or_else(|| by_hash.get(&entry.fingerprint().sha256()).copied());

        let (metadata, ocr_assessment, status) = match local {
            Some(local) => {
                let metadata = merge_metadata(local.metadata(), entry.metadata(), &overrides);
                let assessment = match entry.ocr() {
                    Some(incoming) => {
                        let stored = ocr.get(&entry.fingerprint().sha256());
                        if text_is_current(stored, incoming.text(), incoming.language()) {
                            OcrAssessment::Current
                        } else {
                            OcrAssessment::Importing
                        }
                    }
                    None => OcrAssessment::NotOffered,
                };
                (metadata, assessment, local.status())
            }
            None => (
                entry.metadata().clone(),
                match entry.ocr() {
                    Some(_) => OcrAssessment::Importing,
                    None => OcrAssessment::NotOffered,
                },
                // The PC's own status, so a record it left in
                // `DuplicateIdConflict` — one whose note ID was withheld —
                // arrives as contested rather than as a free unnumbered entry,
                // and a record it could not read is not resurrected as healthy.
                entry.status(),
            ),
        };

        let note =
            IndexedNote::new(entry.path().clone(), entry.fingerprint().clone(), metadata.clone())
                .with_status(status);

        let change = match &moved_from {
            Some(from) => {
                superseded.insert(from.clone());
                EntryChange::Moved { from: from.clone() }
            }
            None => match local {
                // An existing record the import would not change in any way is
                // left alone: its row, its OCR text, and its status survive.
                Some(stored) if *stored == note && ocr_assessment != OcrAssessment::Importing => {
                    EntryChange::Unchanged
                }
                Some(_) => EntryChange::Changed,
                None => EntryChange::New,
            },
        };
        superseded.insert(entry.path().clone());
        if let Some(local) = local {
            superseded.insert(local.path().clone());
        }

        candidates.push(Candidate {
            note: note.clone(),
            entry: Some(entries.len()),
            // Verified: the file is present, readable, and hashes to what the
            // entry claims, so its status is this import's to write.
            conditioned: false,
        });
        entries.push(ImportEntry {
            entry: entry.clone(),
            change,
            local: local.cloned(),
            resolved: Some(note),
            metadata: describe_metadata(local, entry, &overrides),
            ocr: ocr_assessment,
            conflict: None,
        });
    }

    // A record the sidecar does not mention keeps its place in the library, and
    // still holds whatever number it holds.
    for note in &local {
        if !superseded.contains(note.path()) {
            candidates.push(Candidate {
                note: note.clone(),
                entry: None,
                // The catalog's observation stands wherever it came from, not
                // only where a sidecar entry names the path: a file it could
                // not read keeps its condition rather than taking a marker.
                conditioned: matches!(
                    files.get(note.path()),
                    Some(FileCondition::Unreadable | FileCondition::InvalidPdf)
                ),
            });
        }
    }

    let conflicts = resolve_conflicts(&mut candidates, &mut entries, &held);

    // Conflicts are reported per entry too, so the preview screen can mark both
    // files of a collision without joining two lists itself.
    for entry in &mut entries {
        entry.conflict = conflicts
            .iter()
            .find(|conflict| {
                conflict.existing() == entry.path() || conflict.conflicting() == entry.path()
            })
            .cloned();
    }

    let missing: Vec<IndexedNote> = local
        .iter()
        .filter(|note| !superseded.contains(note.path()))
        .cloned()
        .collect();

    // A record the sidecar omits that took part in a conflict is written only
    // when its status actually changes; an incumbent that was already marked
    // costs nothing.
    let contested: Vec<IndexedNote> = candidates
        .iter()
        .filter(|candidate| candidate.entry.is_none())
        .map(|candidate| &candidate.note)
        .filter(|note| {
            note.status() == IndexStatus::DuplicateIdConflict
                && local
                    .iter()
                    .find(|stored| stored.path() == note.path())
                    .is_none_or(|stored| stored.status() != IndexStatus::DuplicateIdConflict)
        })
        .cloned()
        .collect();

    Ok(ImportPreview {
        manifest,
        local,
        ocr,
        files,
        overrides,
        entries,
        missing,
        conflicts,
        contested,
    })
}

/// Checks one entry against the file the library actually has.
///
/// The catalog's observation is the only evidence about the bytes on the phone,
/// so this is where an entry earns the right to be applied at all. `Ok(())`
/// means the file is present, readable, and hashes to what the sidecar claims.
fn verify(entry: &SidecarEntry, condition: Option<&FileCondition>) -> Result<(), EntryChange> {
    let Some(condition) = condition else {
        return Err(EntryChange::Missing);
    };
    match condition {
        FileCondition::Readable(fingerprint) => {
            if fingerprint.sha256() == entry.fingerprint().sha256() {
                Ok(())
            } else {
                Err(EntryChange::HashMismatch {
                    local: fingerprint.sha256(),
                })
            }
        }
        FileCondition::Unreadable | FileCondition::InvalidPdf => Err(EntryChange::Unverified {
            condition: condition.clone(),
        }),
    }
}

/// Resolves every note ID more than one surviving record claims.
///
/// Ownership is decided from `held` — the index as it was before the import —
/// and then by path, so the outcome does not depend on the order the entries
/// were processed in. Each round takes one ID out of contention, which is what
/// makes the loop terminate.
fn resolve_conflicts(
    candidates: &mut [Candidate],
    entries: &mut [ImportEntry],
    held: &BTreeMap<NoteId, HeldClaim>,
) -> Vec<ImportConflict> {
    let mut conflicts = Vec::new();
    // Records whose contested ID may not be withheld. Their write is refused by
    // the store and reported as an `ImportFailure`, so the collision is real
    // and is left in place; what they must not do is be re-detected forever,
    // which would make this loop spin without making progress.
    let mut unresolvable: BTreeSet<RelativePdfPath> = BTreeSet::new();
    loop {
        let mut ordered: Vec<usize> = (0..candidates.len())
            .filter(|index| !unresolvable.contains(candidates[*index].note.path()))
            .collect();
        ordered.sort_by(|left, right| {
            let left = &candidates[*left].note;
            let right = &candidates[*right].note;
            rank(left, held)
                .cmp(&rank(right, held))
                .then_with(|| left.path().cmp(right.path()))
        });
        let notes: Vec<&IndexedNote> = ordered
            .iter()
            .map(|index| &candidates[*index].note)
            .collect();
        let Err(duplicate) = validate_unique_ids(notes.into_iter()) else {
            return conflicts;
        };
        let id = duplicate.id();
        let existing = duplicate.existing_path().clone();
        let conflicting = duplicate.conflicting_path().clone();
        conflicts.push(ImportConflict {
            id,
            existing: existing.clone(),
            conflicting: conflicting.clone(),
        });

        let keeper = position(candidates, &existing);
        let loser = position(candidates, &conflicting);

        // Both records carry the marker so neither looks healthy, and the loser
        // is written without the contested number — unless the user set that
        // number by hand, which an import may not undo.
        //
        // A record whose file the catalog could not use keeps the condition it
        // was observed in: that condition is what was actually seen of the
        // file, and the collision is reported through `ImportConflict` and
        // `ImportEntry::conflict` instead. Reconciliation applies the same rule,
        // so a record cannot be marked by one path and left unmarked by the
        // other.
        if !candidates[keeper].conditioned {
            candidates[keeper].note =
                candidates[keeper].note.with_status(IndexStatus::DuplicateIdConflict);
        }
        if !candidates[loser].conditioned {
            candidates[loser].note =
                candidates[loser].note.with_status(IndexStatus::DuplicateIdConflict);
        }
        if candidates[loser]
            .note
            .metadata()
            .provenance()
            .is_manual(MetadataField::Id)
        {
            // A manual ID is the user's decision and is never withheld, so the
            // store refuses this write and the import reports it. The collision
            // stays exactly as it is: resolving it is the user's, by renumbering
            // or renaming one of the two files.
            unresolvable.insert(candidates[loser].note.path().clone());
        } else {
            let note = &candidates[loser].note;
            let mut metadata = note.metadata().clone();
            metadata.assign_id(None, note.metadata().origin_of(MetadataField::Id));
            candidates[loser].note =
                IndexedNote::new(note.path().clone(), note.fingerprint().clone(), metadata)
                    .with_status(note.status());
        }

        // The decision about the contested ID is part of what gets stored, so it
        // is written back onto the entry: an entry that was otherwise untouched
        // now differs from its record, and an entry that was already a write
        // keeps the category the user reads it under.
        for candidate in candidates.iter() {
            let Some(index) = candidate.entry else {
                continue;
            };
            if entries[index].resolved.as_ref() == Some(&candidate.note) {
                continue;
            }
            entries[index].resolved = Some(candidate.note.clone());
            if matches!(entries[index].change, EntryChange::Unchanged) {
                entries[index].change = EntryChange::Changed;
            }
        }
    }
}

/// Finds a candidate by path.
fn position(candidates: &[Candidate], path: &RelativePdfPath) -> usize {
    candidates
        .iter()
        .position(|candidate| candidate.note.path() == path)
        .expect("a candidate named by the conflict is one of the candidates")
}

/// The fields of `metadata` the user edited by hand.
fn manual_fields(metadata: &NoteMetadata) -> Vec<MetadataField> {
    MetadataField::ALL
        .into_iter()
        .filter(|field| metadata.provenance().is_manual(*field))
        .collect()
}

/// Reports which fields an import takes and which it leaves alone.
fn describe_metadata(
    local: Option<&IndexedNote>,
    entry: &SidecarEntry,
    overrides: &BTreeSet<MetadataField>,
) -> MetadataImport {
    let Some(local) = local else {
        return MetadataImport {
            imported: imported_for_new_entry(entry),
            preserved: Vec::new(),
            overridden: Vec::new(),
        };
    };
    let (imported, preserved) = merge_fields(local.metadata(), entry.metadata(), overrides);
    MetadataImport {
        imported,
        preserved,
        overridden: MetadataField::ALL
            .into_iter()
            .filter(|field| overrides.contains(field))
            .collect(),
    }
}

/// The fields an entry contributes when the library has no record for it.
fn imported_for_new_entry(entry: &SidecarEntry) -> Vec<MetadataField> {
    let metadata = entry.metadata();
    MetadataField::ALL
        .into_iter()
        .filter(|field| match field {
            MetadataField::Id => metadata.id().is_some(),
            MetadataField::Title => true,
            MetadataField::ComposerOrArtist => metadata.composer_or_artist().is_some(),
            MetadataField::Tags => !metadata.tags().is_empty(),
        })
        .collect()
}

/// Merges the sidecar's metadata into the stored one.
///
/// A field the user edited is kept unless `overrides` names it, in which case the
/// sidecar's value replaces it and the field becomes [`MetadataOrigin::Manual`]
/// — an explicit replacement is the user's decision. A field whose value is
/// already equal is left exactly as it is, provenance included: there is nothing
/// to attribute, and rewriting it would turn every import into a write.
fn merge_metadata(
    local: &NoteMetadata,
    incoming: &NoteMetadata,
    overrides: &BTreeSet<MetadataField>,
) -> NoteMetadata {
    let (imported, _) = merge_fields(local, incoming, overrides);
    let mut metadata = local.clone();
    for field in imported {
        let origin = if overrides.contains(&field) {
            MetadataOrigin::Manual
        } else {
            MetadataOrigin::PcImported
        };
        copy_field(&mut metadata, field, incoming, origin);
    }
    metadata
}

/// Decides, per field, whether the sidecar's value replaces the stored one.
fn merge_fields(
    local: &NoteMetadata,
    incoming: &NoteMetadata,
    overrides: &BTreeSet<MetadataField>,
) -> (Vec<MetadataField>, Vec<MetadataField>) {
    let mut imported = Vec::new();
    let mut preserved = Vec::new();
    for field in MetadataField::ALL {
        if same_value(local, incoming, field) {
            continue;
        }
        if local.provenance().is_manual(field) && !overrides.contains(&field) {
            preserved.push(field);
        } else {
            imported.push(field);
        }
    }
    (imported, preserved)
}

/// Whether one field carries the same value in both records.
fn same_value(left: &NoteMetadata, right: &NoteMetadata, field: MetadataField) -> bool {
    match field {
        MetadataField::Id => left.id() == right.id(),
        MetadataField::Title => left.title() == right.title(),
        MetadataField::ComposerOrArtist => left.composer_or_artist() == right.composer_or_artist(),
        MetadataField::Tags => left.tags() == right.tags(),
    }
}

/// Copies one field's value out of `incoming` with an explicit origin.
fn copy_field(
    target: &mut NoteMetadata,
    field: MetadataField,
    incoming: &NoteMetadata,
    origin: MetadataOrigin,
) {
    match field {
        MetadataField::Id => target.assign_id(incoming.id(), origin),
        MetadataField::Title => {
            // Both records went through `NoteMetadata::new`, which rejects an
            // empty title, so there is always a value to copy.
            let _ = target.assign_title(incoming.title(), origin);
        }
        MetadataField::ComposerOrArtist => {
            target.assign_composer_or_artist(incoming.composer_or_artist().map(str::to_owned), origin);
        }
        MetadataField::Tags => target.assign_tags(incoming.tags().iter().cloned(), origin),
    }
}

/// Whether the stored OCR text already says what the sidecar says.
///
/// Absent local text is never current: a record with no OCR yet is exactly the
/// case an import is meant to fill.
fn text_is_current(stored: Option<&OcrText>, text: &str, language: Option<&str>) -> bool {
    let Some(stored) = stored else {
        return false;
    };
    stored.text() == text && stored.language() == language
}