//! The portable sidecar manifest written by the PC indexer.
//!
//! A sidecar is the cross-platform half of the Notae index: the Linux CLI reads
//! its own library, writes `.notae/index.sqlite` beside `.notae/manifest.json`,
//! and the Android app validates that manifest against its own library before
//! importing anything. The manifest is therefore a *contract*, not an internal
//! dump: its shape is stable snake_case JSON, its schema version is explicit,
//! and every value in it is a validated domain type.
//!
//! # What crosses the boundary
//!
//! Each [`SidecarEntry`] carries one indexed PDF: where it lives, the SHA-256
//! fingerprint of its bytes, the note metadata with its per-field provenance,
//! the pipeline [`IndexStatus`] the PC left the record in, and the OCR text the
//! PC recognized, if any. Nothing else is exported — no row identifiers — because
//! the receiving app owns its own database and re-derives everything else it
//! needs.
//!
//! # What the manifest deliberately does not carry
//!
//! The manifest never claims that the receiving library matches it. It is a
//! proposal: the receiving app compares each entry against the files it actually
//! has and reports the difference, which is what [`crate::SidecarImporter`]
//! does. The manifest also carries no evidence about the receiving library's
//! bytes, so the comparison has to be made against a
//! [`PdfCatalog`](crate::PdfCatalog) rather than against the manifest alone.
//!
//! # Strictness
//!
//! The wire structs reject unknown fields and require `schema_version`. A
//! producer and a consumer that do not agree must find out before anything is
//! imported, not by silently dropping the fields the other side invented.
//!
//! That rule reaches every level of the manifest, not just its top. The
//! domain types are deliberately lenient about what may be *absent* — a record
//! may omit optional fields, and [`NoteMetadata`] migrates the legacy scalar
//! `origin` — but `deny_unknown_fields` only guards the level it is written
//! on, so reading through them would have left the fingerprint, the metadata
//! object, the provenance object, and the timestamp free to carry fields this
//! build silently drops. The sidecar therefore owns private wire types for
//! those four shapes and converts them into domain values at the boundary. The
//! compatibility migration stays in [`NoteMetadata`], where it belongs, and the
//! wire contract is written once.

use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::Path;

use notae_domain::{
    DomainError, EpochTimestamp, FieldProvenance, IndexStatus, IndexedNote, MetadataField,
    MetadataOrigin, NoteId, NoteMetadata, PdfFingerprint, RelativePdfPath, Sha256Digest,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::reconcile::CatalogError;
use crate::{IndexError, IndexStore, OcrText};

/// Version of the sidecar manifest this build writes and accepts.
///
/// The version is part of the contract: a manifest written by another build
/// with a different version is rejected rather than guessed at, because an
/// unrecognized layout would otherwise import as silently wrong data.
///
/// Version 2 added the required per-entry `status`, made `schema_version`
/// required, and started rejecting unknown fields. Version 1 is not accepted:
/// its entries carry no status, so a record the PC left in
/// [`IndexStatus::DuplicateIdConflict`] would arrive on the phone
/// indistinguishable from a free unnumbered one.
pub const SIDECAR_SCHEMA_VERSION: u32 = 2;

/// Failures produced by reading, writing, or importing a sidecar.
///
/// Every variant stops the whole operation. A sidecar is validated end to end
/// before anything is written, so a malformed or unsupported manifest cannot
/// leave the local library half-imported.
#[derive(Debug, Error)]
pub enum SidecarError {
    /// The manifest file could not be read or written.
    #[error("sidecar file could not be accessed: {0}")]
    Io(#[from] std::io::Error),
    /// The manifest is not valid JSON in the sidecar shape.
    #[error("sidecar manifest is malformed: {0}")]
    Json(#[from] serde_json::Error),
    /// The manifest declares a version this build does not understand.
    #[error("sidecar schema version {found} is not supported; this build reads version {supported}")]
    UnsupportedSchemaVersion {
        /// Version found in the manifest.
        found: u32,
        /// Version this build understands.
        supported: u32,
    },
    /// Two entries name the same path.
    ///
    /// A path identifies one file, so a manifest that lists it twice describes
    /// two records for one file and cannot be applied without guessing which
    /// one wins.
    #[error("sidecar lists the path {0} more than once")]
    DuplicatePath(RelativePdfPath),
    /// Two entries claim the same content hash.
    ///
    /// File identity is the SHA-256 of the bytes: one file's content is indexed
    /// once. A manifest listing the same hash twice asks for two records that
    /// cannot both exist.
    #[error("sidecar lists the same content hash {0} more than once")]
    DuplicateContent(Sha256Digest),
    /// An import plan referred to an entry the manifest does not contain.
    #[error("no sidecar entry names the path {0}")]
    UnknownEntry(RelativePdfPath),
    /// The local index could not be read or written.
    #[error(transparent)]
    Store(#[from] IndexError),
    /// The library root could not be enumerated at all.
    ///
    /// Without a catalog there are no hashes to verify the sidecar against, so
    /// this stops the import rather than letting an unverified entry through.
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

/// OCR text as it crosses the sidecar boundary.
///
/// This is [`OcrText`] with a serde representation: the normalized text and the
/// detected language, both optional in the language's case. Keeping it a
/// separate wire type means the sidecar format cannot change just because the
/// in-memory representation does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidecarOcr {
    /// Normalized recognized text.
    text: String,
    /// Language detected for the text, when one was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    language: Option<String>,
}

impl SidecarOcr {
    /// Creates sidecar OCR text from stored OCR text.
    #[must_use]
    pub fn from_ocr(ocr: &OcrText) -> Self {
        Self {
            text: ocr.text().to_string(),
            language: ocr.language().map(str::to_owned),
        }
    }

    /// Returns the normalized recognized text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the detected language, when one was recorded.
    #[must_use]
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }
}

impl From<&OcrText> for SidecarOcr {
    fn from(ocr: &OcrText) -> Self {
        Self::from_ocr(ocr)
    }
}

impl From<&SidecarOcr> for OcrText {
    fn from(ocr: &SidecarOcr) -> Self {
        match &ocr.language {
            Some(language) => OcrText::with_language(&ocr.text, language),
            None => OcrText::new(&ocr.text),
        }
    }
}

/// One indexed PDF as the sidecar describes it.
///
/// An entry is a *proposal* about one file. It names the file by its
/// library-relative path and identifies its content by the SHA-256 of its
/// bytes, so the receiving app can tell a renamed file from a rewritten one
/// without trusting either.
///
/// # Wire form
///
/// ```json
/// {
///   "relative_path": "12 - Autumn Leaves.pdf",
///   "fingerprint": {"sha256": "…", "byte_len": 1024},
///   "metadata": {"id": 12, "title": "Autumn Leaves", "provenance": {…}},
///   "status": "duplicate_id_conflict",
///   "ocr": {"text": "…", "language": "eng"}
/// }
/// ```
///
/// `metadata` carries its own `provenance` object, which is not decoration:
/// [`NoteMetadata`] refuses to deserialize without one, so a manifest cannot
/// smuggle in a record whose manually edited fields are indistinguishable from
/// fields that were never set.
///
/// `status` is the PC's own [`IndexStatus`] for the record, carried verbatim.
/// It is what makes a *withheld* ID legible: a record the PC left in
/// [`DuplicateIdConflict`](IndexStatus::DuplicateIdConflict) was written with
/// no note ID at all, because two records claimed that number. Without this
/// field the receiving app invents a status from whether OCR text came with
/// the entry, and a contested record with no number becomes indistinguishable
/// from a free unnumbered one. It also round-trips the condition a record was
/// left in by a rescan that could not read or parse it
/// ([`Inaccessible`](IndexStatus::Inaccessible) /
/// [`InvalidPdf`](IndexStatus::InvalidPdf)), which the conflict marker must not
/// replace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "EntryWire")]
pub struct SidecarEntry {
    /// Path relative to the library root.
    relative_path: RelativePdfPath,
    /// Content identity of the file.
    fingerprint: PdfFingerprint,
    /// Metadata, including per-field provenance.
    metadata: NoteMetadata,
    /// Where the record sat in the PC's pipeline.
    status: IndexStatus,
    /// OCR text recognized on the PC, when there is any.
    #[serde(skip_serializing_if = "Option::is_none")]
    ocr: Option<SidecarOcr>,
}

impl SidecarEntry {
    /// Builds an entry for one indexed record and its OCR text.
    ///
    /// The record's own metadata is carried verbatim, provenance included, so an
    /// edit the user made on the PC arrives on the phone as the manual edit it
    /// is and is not silently re-imported over later.
    ///
    /// Empty OCR text is stored as no OCR text at all. A record that was
    /// fingerprinted but never recognized has nothing to offer, and writing an
    /// empty string would say "this file has no text", which a later import
    /// would act on by replacing local text with nothing.
    #[must_use]
    pub fn new(note: &IndexedNote, ocr: Option<&OcrText>) -> Self {
        Self {
            relative_path: note.path().clone(),
            fingerprint: note.fingerprint().clone(),
            metadata: note.metadata().clone(),
            status: note.status(),
            ocr: ocr
                .filter(|ocr| !ocr.text().is_empty())
                .map(SidecarOcr::from_ocr),
        }
    }

    /// Returns the library-relative path.
    #[must_use]
    pub const fn path(&self) -> &RelativePdfPath {
        &self.relative_path
    }

    /// Returns the content fingerprint.
    #[must_use]
    pub const fn fingerprint(&self) -> &PdfFingerprint {
        &self.fingerprint
    }

    /// Returns the metadata, with its per-field provenance.
    #[must_use]
    pub const fn metadata(&self) -> &NoteMetadata {
        &self.metadata
    }

    /// Returns the per-field provenance.
    #[must_use]
    pub const fn provenance(&self) -> &FieldProvenance {
        self.metadata.provenance()
    }

    /// Returns the OCR text, when the PC recognized any.
    #[must_use]
    pub const fn ocr(&self) -> Option<&SidecarOcr> {
        self.ocr.as_ref()
    }

    /// Returns where the record sat in the PC's pipeline.
    ///
    /// This is the PC's own status, not the receiving app's: a record the PC
    /// could not read keeps [`IndexStatus::InvalidPdf`] here, and a record it
    /// left in [`IndexStatus::DuplicateIdConflict`] is one whose note ID was
    /// withheld because two records claimed it.
    #[must_use]
    pub const fn status(&self) -> IndexStatus {
        self.status
    }

    /// Returns the OCR text as stored text, when the entry carries any.
    #[must_use]
    pub fn ocr_text(&self) -> Option<OcrText> {
        self.ocr.as_ref().map(OcrText::from)
    }

    /// Rebuilds the indexed record this entry describes.
    ///
    /// The record takes the entry's [`status`](Self::status), so a conflict or
    /// a condition the PC recorded survives the crossing instead of being
    /// rebuilt as [`IndexStatus::Discovered`].
    #[must_use]
    pub fn to_note(&self) -> IndexedNote {
        IndexedNote::new(
            self.relative_path.clone(),
            self.fingerprint.clone(),
            self.metadata.clone(),
        )
        .with_status(self.status)
    }
}

/// Why a strictly-read wire value could not become a domain value.
///
/// This is deliberately not a [`SidecarError`]: it is reached from inside
/// `Deserialize`, where a serde error is what the caller sees, and every variant
/// is a malformed manifest rather than a failed import.
#[derive(Debug, Error)]
enum WireError {
    /// A value the domain constructors reject.
    #[error(transparent)]
    Domain(#[from] DomainError),
    /// A timestamp whose nanoseconds are not below one billion.
    #[error("sidecar timestamp nanoseconds are out of range")]
    TimestampNanos,
}

/// The strict wire shape of the whole sidecar.
///
/// Strict for the same reason as [`EntryWire`]: the timestamp is read through a
/// strict type, so a field added to it is refused rather than dropped.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestWire {
    schema_version: u32,
    generated_at: TimestampWire,
    tool_version: String,
    entries: Vec<EntryWire>,
}

/// The strict wire shape of one entry.
///
/// # Why the sidecar has its own nested types
///
/// The domain types are deliberately permissive about *absence* — a record may
/// omit optional fields, and [`NoteMetadata`] accepts the legacy scalar `origin`
/// so a manifest written before per-field provenance existed still migrates.
/// That leniency is about what a *reader* will accept from an older producer;
/// it is not permission to accept fields this build does not know.
///
/// `deny_unknown_fields` only guards the level it is written on, so a struct
/// deriving `Deserialize` still takes its nested domain values through their own
/// permissive deserializers: a fingerprint, a metadata object, a provenance
/// object, and a timestamp were all read leniently, and a producer could add a
/// field to any of them and have it silently dropped. These wire structs close
/// that hole. They are private: the boundary converts in and out of the domain
/// types, so the lenient compatibility migration stays where it belongs, in
/// [`NoteMetadata`], and never becomes the sidecar's contract.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryWire {
    relative_path: RelativePdfPath,
    fingerprint: FingerprintWire,
    metadata: MetadataWire,
    status: IndexStatus,
    #[serde(default)]
    ocr: Option<SidecarOcr>,
}

/// The strict wire shape of a content fingerprint.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FingerprintWire {
    sha256: Sha256Digest,
    byte_len: u64,
    #[serde(default)]
    modified: Option<TimestampWire>,
}

/// The strict wire shape of a point in time.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TimestampWire {
    secs: i64,
    #[serde(default)]
    nanos: u32,
}

/// The strict wire shape of a note's metadata.
///
/// `origin` is the legacy scalar [`NoteMetadata`] migrates from, accepted here
/// for the same reason it is accepted there: a manifest written by an older
/// producer still reads. The migration is unchanged — a scalar applies to every
/// field — and a record carrying neither `provenance` nor `origin` is refused.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MetadataWire {
    #[serde(default)]
    id: Option<NoteId>,
    title: String,
    #[serde(default)]
    composer_or_artist: Option<String>,
    #[serde(default)]
    tags: BTreeSet<String>,
    provenance: Option<ProvenanceWire>,
    origin: Option<MetadataOrigin>,
}

/// The strict wire shape of per-field provenance.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProvenanceWire {
    #[serde(default)]
    id: MetadataOrigin,
    #[serde(default)]
    title: MetadataOrigin,
    #[serde(default)]
    composer_or_artist: MetadataOrigin,
    #[serde(default)]
    tags: MetadataOrigin,
}

impl TryFrom<ManifestWire> for SidecarManifest {
    type Error = WireError;

    fn try_from(wire: ManifestWire) -> Result<Self, Self::Error> {
        Ok(Self {
            schema_version: wire.schema_version,
            generated_at: wire.generated_at.try_into()?,
            tool_version: wire.tool_version,
            entries: wire
                .entries
                .into_iter()
                .map(SidecarEntry::try_from)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl TryFrom<EntryWire> for SidecarEntry {
    type Error = WireError;

    fn try_from(wire: EntryWire) -> Result<Self, Self::Error> {
        Ok(Self {
            relative_path: wire.relative_path,
            fingerprint: wire.fingerprint.try_into()?,
            metadata: wire.metadata.try_into()?,
            status: wire.status,
            ocr: wire.ocr,
        })
    }
}

impl TryFrom<FingerprintWire> for PdfFingerprint {
    type Error = WireError;

    fn try_from(wire: FingerprintWire) -> Result<Self, Self::Error> {
        Ok(Self::new(
            wire.sha256,
            wire.byte_len,
            wire.modified.map(TimestampWire::try_into).transpose()?,
        ))
    }
}

impl TryFrom<TimestampWire> for EpochTimestamp {
    type Error = WireError;

    fn try_from(wire: TimestampWire) -> Result<Self, Self::Error> {
        Self::new(wire.secs, wire.nanos).ok_or(WireError::TimestampNanos)
    }
}

impl TryFrom<MetadataWire> for NoteMetadata {
    type Error = WireError;

    fn try_from(wire: MetadataWire) -> Result<Self, Self::Error> {
        let provenance = match (wire.provenance, wire.origin) {
            (Some(provenance), _) => provenance.into(),
            (None, Some(origin)) => FieldProvenance::with_every_field(origin),
            (None, None) => return Err(WireError::from(DomainError::MissingProvenance)),
        };
        let mut metadata = Self::new(&wire.title, wire.composer_or_artist, wire.tags)?;
        metadata.assign_id(wire.id, provenance.origin_of(MetadataField::Id));
        metadata.set_origin(MetadataField::Title, provenance.origin_of(MetadataField::Title));
        metadata.set_origin(
            MetadataField::ComposerOrArtist,
            provenance.origin_of(MetadataField::ComposerOrArtist),
        );
        metadata.set_origin(MetadataField::Tags, provenance.origin_of(MetadataField::Tags));
        Ok(metadata)
    }
}

impl From<ProvenanceWire> for FieldProvenance {
    fn from(wire: ProvenanceWire) -> Self {
        let mut provenance = Self::new();
        provenance.set_origin(MetadataField::Id, wire.id);
        provenance.set_origin(MetadataField::Title, wire.title);
        provenance.set_origin(MetadataField::ComposerOrArtist, wire.composer_or_artist);
        provenance.set_origin(MetadataField::Tags, wire.tags);
        provenance
    }
}

/// The whole sidecar: a versioned list of entries plus provenance about the
/// run that produced it.
///
/// # Wire form
///
/// ```json
/// {
///   "schema_version": 2,
///   "generated_at": {"secs": 1700000000, "nanos": 0},
///   "tool_version": "notae-index 0.1.0",
///   "entries": [ … ]
/// }
/// ```
///
/// Entries are written in ascending path order, so two runs over an unchanged
/// library produce byte-identical manifests apart from `generated_at`. That is
/// what makes a sidecar reviewable as a diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ManifestWire")]
pub struct SidecarManifest {
    /// Version of this manifest's shape.
    ///
    /// Required. A manifest that omits it has not declared which contract it
    /// was written to, and reading it as the current version would be a guess:
    /// the field is the one thing that tells a producer and a consumer whether
    /// they agree, so a manifest without it is refused rather than assumed.
    schema_version: u32,
    /// When the PC run that produced this manifest finished.
    generated_at: EpochTimestamp,
    /// Version of the tool that wrote it.
    tool_version: String,
    /// One entry per indexed PDF.
    entries: Vec<SidecarEntry>,
}

impl SidecarManifest {
    /// Builds a manifest from entries.
    ///
    /// The schema version is set to [`SIDECAR_SCHEMA_VERSION`]; a manifest of
    /// another version is produced by editing the field afterwards, which is
    /// how a caller constructs the input its rejection path must handle.
    #[must_use]
    pub fn new(
        generated_at: EpochTimestamp,
        tool_version: impl Into<String>,
        entries: Vec<SidecarEntry>,
    ) -> Self {
        Self {
            schema_version: SIDECAR_SCHEMA_VERSION,
            generated_at,
            tool_version: tool_version.into(),
            entries,
        }
    }

    /// Returns the manifest's schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Overrides the schema version, for constructing a manifest a build is
    /// expected to reject.
    pub fn set_schema_version(&mut self, version: u32) {
        self.schema_version = version;
    }

    /// Returns when the run that produced this manifest finished.
    #[must_use]
    pub const fn generated_at(&self) -> EpochTimestamp {
        self.generated_at
    }

    /// Returns the version of the tool that wrote the manifest.
    #[must_use]
    pub fn tool_version(&self) -> &str {
        &self.tool_version
    }

    /// Returns the entries, in the order they will be written.
    #[must_use]
    pub fn entries(&self) -> &[SidecarEntry] {
        &self.entries
    }

    /// Returns the entry filed under `path`, if there is one.
    #[must_use]
    pub fn entry(&self, path: &RelativePdfPath) -> Option<&SidecarEntry> {
        self.entries.iter().find(|entry| entry.path() == path)
    }

    /// Checks everything that must hold before any import is attempted.
    ///
    /// This is the gate the whole import contract rests on: the schema version
    /// must be one this build understands, and the manifest must not describe
    /// one path or one piece of content twice. Running it before a single
    /// write is what makes a rejected sidecar leave the library untouched
    /// rather than half-imported.
    ///
    /// # Errors
    /// Returns [`SidecarError::UnsupportedSchemaVersion`],
    /// [`SidecarError::DuplicatePath`], or [`SidecarError::DuplicateContent`].
    pub fn validate(&self) -> Result<(), SidecarError> {
        if self.schema_version != SIDECAR_SCHEMA_VERSION {
            return Err(SidecarError::UnsupportedSchemaVersion {
                found: self.schema_version,
                supported: SIDECAR_SCHEMA_VERSION,
            });
        }
        let mut paths = BTreeSet::new();
        let mut hashes = BTreeSet::new();
        for entry in &self.entries {
            if !paths.insert(entry.path().clone()) {
                return Err(SidecarError::DuplicatePath(entry.path().clone()));
            }
            if !hashes.insert(entry.fingerprint().sha256()) {
                return Err(SidecarError::DuplicateContent(entry.fingerprint().sha256()));
            }
        }
        Ok(())
    }

    /// Serializes the manifest to JSON.
    ///
    /// # Errors
    /// Returns [`SidecarError::Json`] when a value cannot be serialized.
    pub fn to_json(&self) -> Result<String, SidecarError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Writes the manifest to `destination`, creating parent directories.
    ///
    /// Missing parent directories are created, which is what makes
    /// `<library>/.notae/manifest.json` a one-call operation for the CLI.
    ///
    /// # Errors
    /// Returns [`SidecarError::Io`] when the file cannot be created or
    /// written, and [`SidecarError::Json`] on a serialization failure.
    pub fn write(&self, destination: impl AsRef<Path>) -> Result<(), SidecarError> {
        let destination = destination.as_ref();
        if let Some(parent) = destination.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::File::create(destination)?;
        file.write_all(self.to_json()?.as_bytes())?;
        file.sync_all()?;
        Ok(())
    }

    /// Reads and validates a manifest from `source`.
    ///
    /// Reading validates too, so a manifest that reached the filesystem
    /// malformed is refused here rather than at the point of import.
    ///
    /// # Errors
    /// Returns [`SidecarError::Io`] or [`SidecarError::Json`] on a read or
    /// parse failure, and whatever [`SidecarManifest::validate`] reports for a
    /// structurally valid but unusable manifest.
    pub fn read(source: impl AsRef<Path>) -> Result<Self, SidecarError> {
        let text = std::fs::read_to_string(source)?;
        let manifest: Self = serde_json::from_str(&text)?;
        manifest.validate()?;
        Ok(manifest)
    }
}

/// Writes the sidecar manifest for an index the PC produced.
///
/// The writer is the only place that decides what a sidecar contains, so the
/// Linux CLI and any other producer cannot drift: it reads one store and emits
/// one versioned manifest in the stable wire form.
///
/// # What it exports
///
/// Every record in the store becomes an entry, carrying its fingerprint, its
/// metadata with per-field provenance, and its OCR text when the record has
/// any. Nothing is filtered: a record the PC could not index is not in its
/// store, so the manifest describes exactly the records that exist, and the
/// receiving app decides what to do about a library that differs from it.
///
/// # Ordering
///
/// Entries are sorted by path, so re-running the indexer over an unchanged
/// library rewrites a byte-identical manifest apart from its timestamp. That
/// makes a sidecar reviewable as a diff rather than as noise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarWriter {
    tool_version: String,
    generated_at: EpochTimestamp,
}

impl SidecarWriter {
    /// Builds a writer stamping manifests with `tool_version` and
    /// `generated_at`.
    #[must_use]
    pub fn new(tool_version: impl Into<String>, generated_at: EpochTimestamp) -> Self {
        Self {
            tool_version: tool_version.into(),
            generated_at,
        }
    }

    /// Reads `root` and writes its manifest to `destination`.
    ///
    /// The destination is created together with its parent directories, which
    /// is what makes `<library>/.notae/manifest.json` a one-call operation for
    /// the CLI.
    ///
    /// # Errors
    /// Returns [`SidecarError::Store`] when the index cannot be read, and
    /// [`SidecarError::Io`] or [`SidecarError::Json`] when the manifest cannot
    /// be written.
    pub fn write(&self, root: &IndexStore, destination: impl AsRef<Path>) -> Result<(), SidecarError> {
        let mut notes = root.list_all()?;
        notes.sort_by(|left, right| left.path().cmp(right.path()));
        let entries = notes
            .iter()
            .map(|note| {
                let ocr = root.ocr_text(&note.fingerprint().sha256())?;
                Ok(SidecarEntry::new(note, ocr.as_ref()))
            })
            .collect::<Result<Vec<_>, SidecarError>>()?;
        let manifest = SidecarManifest::new(self.generated_at, &self.tool_version, entries);
        manifest.write(destination)
    }
}