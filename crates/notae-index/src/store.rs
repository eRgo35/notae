//! SQLite-backed repository for indexed notes.
//!
//! [`IndexStore`] owns one synchronous connection. All writes go through
//! [`IndexStore::upsert_note`], [`IndexStore::upsert_note_with_ocr`], or
//! [`IndexStore::set_ocr`], which update the canonical `notes` row and rewrite
//! that row's entry in the derived FTS5 index in the same transaction.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;

use notae_domain::{
    EpochTimestamp, FieldProvenance, IndexedNote, IndexStatus, MetadataField, MetadataOrigin,
    NoteId, NoteMetadata, PdfFingerprint, RelativePdfPath, Sha256Digest, compare_library_order,
};
use rusqlite::{Connection, OptionalExtension, Row, ToSql, params, params_from_iter};
use thiserror::Error;

use crate::schema;
use crate::search::SearchHit;

/// Failures produced by the index repository.
#[derive(Debug, Error)]
pub enum IndexError {
    /// The database rejected a statement or returned an unreadable row.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// The note ID is already claimed by a different record.
    ///
    /// The ID invariant allows at most one indexed PDF per numeric ID, so the
    /// write is refused and neither record is touched: the caller decides how
    /// to report the conflict.
    #[error("note id {0} is already used by another indexed record")]
    DuplicateNoteId(NoteId),
    /// The database was written by a newer build of Notae.
    #[error("index schema version {found} is newer than the supported version {supported}")]
    UnsupportedSchemaVersion {
        /// Version found in the database.
        found: u32,
        /// Version this build understands.
        supported: u32,
    },
    /// No record matches the content hash the caller asked about.
    #[error("no indexed record has content hash {hash}")]
    UnknownNote {
        /// The hash that matched no row.
        hash: Sha256Digest,
    },
}

/// Text recognized from the printed pages of one PDF.
///
/// OCR text is stored beside, not inside, [`IndexedNote`]: a record is
/// fingerprinted long before OCR runs, and re-running OCR must be able to
/// replace the text without touching metadata or provenance. Constructing an
/// `OcrText` normalizes it — whitespace runs collapse to single spaces and the
/// text is trimmed — so a value read back from the database always equals the
/// value a freshly constructed `OcrText` produces.
///
/// Case folding is deliberately not applied: the FTS5 `unicode61` tokenizer
/// already matches case-insensitively, so storing the text as recognized keeps
/// it readable and directly comparable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrText {
    text: String,
    language: Option<String>,
}

impl OcrText {
    /// Creates OCR text without a detected language.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: normalize_ocr_text(&text.into()),
            language: None,
        }
    }

    /// Creates OCR text with the language detected for it.
    #[must_use]
    pub fn with_language(text: impl Into<String>, language: impl Into<String>) -> Self {
        Self {
            text: normalize_ocr_text(&text.into()),
            language: Some(language.into()),
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

/// Columns of `notes` read by [`note_from_row`], in a fixed order.
///
/// Every read uses this one list so the row reader and the query clauses stay
/// in step.
const NOTE_COLUMNS: &str = "row_id, path, sha256, byte_len, modified_secs, modified_nanos, \
     note_id, title, composer_or_artist, tags, provenance_id, provenance_title, \
     provenance_composer, provenance_tags, status, ocr_text, ocr_language";

/// Insert or update of the canonical note row, keyed by content hash.
const UPSERT_NOTE: &str = "\
INSERT INTO notes (
    path, sha256, byte_len, modified_secs, modified_nanos, note_id, title,
    composer_or_artist, tags, provenance_id, provenance_title, provenance_composer,
    provenance_tags, status, ocr_text, ocr_language, indexed_at
) VALUES (
    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
)
ON CONFLICT (sha256) DO UPDATE SET
    path = excluded.path,
    byte_len = excluded.byte_len,
    modified_secs = excluded.modified_secs,
    modified_nanos = excluded.modified_nanos,
    note_id = excluded.note_id,
    title = excluded.title,
    composer_or_artist = excluded.composer_or_artist,
    tags = excluded.tags,
    provenance_id = excluded.provenance_id,
    provenance_title = excluded.provenance_title,
    provenance_composer = excluded.provenance_composer,
    provenance_tags = excluded.provenance_tags,
    status = excluded.status,
    ocr_text = excluded.ocr_text,
    ocr_language = excluded.ocr_language,
    indexed_at = excluded.indexed_at
RETURNING row_id";

/// One indexed-note database.
#[derive(Debug)]
pub struct IndexStore {
    conn: Connection,
}

impl IndexStore {
    /// Opens `path`, creating the file if needed, and applies pending
    /// migrations.
    ///
    /// `":memory:"` opens a private in-memory database.
    ///
    /// # Errors
    ///
    /// Returns [`IndexError::UnsupportedSchemaVersion`] when the file was
    /// written by a newer build.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, IndexError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Wraps an already-open connection, applying pending migrations.
    ///
    /// # Errors
    ///
    /// Fails as [`IndexStore::open`] does.
    pub fn from_connection(conn: Connection) -> Result<Self, IndexError> {
        schema::migrate(&conn)?;
        Ok(Self { conn })
    }

    /// Returns the schema version recorded in the database.
    ///
    /// # Errors
    ///
    /// Fails when the database cannot be read.
    pub fn schema_version(&self) -> Result<u32, IndexError> {
        schema::applied_version(&self.conn)
    }

    /// Inserts or updates `note`, keyed by its content hash.
    ///
    /// A record that already has the same SHA-256 is updated in place, so a
    /// renamed file keeps one record and its OCR text. A note ID already
    /// claimed by another record makes the whole write fail with
    /// [`IndexError::DuplicateNoteId`].
    ///
    /// # Errors
    ///
    /// Fails on any database error, including a duplicate note ID.
    pub fn upsert_note(&self, note: &IndexedNote) -> Result<(), IndexError> {
        self.upsert_note_with_ocr(note, None)
    }

    /// Inserts or updates `note` and optionally replaces its OCR text.
    ///
    /// Passing `None` for `ocr` keeps whatever OCR text the record with that
    /// content hash already has, so a metadata write never silently drops OCR.
    ///
    /// # Errors
    ///
    /// Fails as [`IndexStore::upsert_note`] does.
    pub fn upsert_note_with_ocr(
        &self,
        note: &IndexedNote,
        ocr: Option<&OcrText>,
    ) -> Result<(), IndexError> {
        let ocr = match ocr {
            Some(text) => text.clone(),
            None => self
                .ocr_text(&note.fingerprint().sha256())?
                .unwrap_or_else(|| OcrText::new(String::new())),
        };

        let transaction = self.conn.unchecked_transaction()?;
        let row_id = transaction
            .query_row(
                UPSERT_NOTE,
                note_params(note, &ocr),
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| classify_write_error(error, note))?;
        write_fts_row(&transaction, row_id, note, ocr.text())?;
        transaction.commit()?;
        Ok(())
    }

    /// Replaces the OCR text of the record with the given content hash.
    ///
    /// # Errors
    ///
    /// Returns [`IndexError::UnknownNote`] when no record has that hash.
    pub fn set_ocr(&self, hash: &Sha256Digest, ocr: &OcrText) -> Result<(), IndexError> {
        let transaction = self.conn.unchecked_transaction()?;
        let (row_id, note) = read_note_with_row_id(&transaction, "sha256 = ?1", [hash.to_string()])?
            .ok_or(IndexError::UnknownNote { hash: *hash })?;
        transaction.execute(
            "UPDATE notes SET ocr_text = ?1, ocr_language = ?2 WHERE row_id = ?3",
            params![ocr.text(), ocr.language(), row_id],
        )?;
        write_fts_row(&transaction, row_id, &note, ocr.text())?;
        transaction.commit()?;
        Ok(())
    }

    /// Searches the indexed notes.
    ///
    /// The query matches note ID, title, composer/artist, tags, filename, and
    /// OCR text, and at most `limit` hits come back in rank order with the
    /// default browsing order as the tie-break. See [`crate::SearchHit`].
    ///
    /// # Errors
    ///
    /// Fails when the database cannot be read or holds a value that no longer
    /// parses.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, IndexError> {
        crate::search::search(&self.conn, query, limit)
    }

    /// Returns the record with the given content hash.
    ///
    /// # Errors
    ///
    /// Fails when the database cannot be read or a stored value is invalid.
    pub fn get_by_hash(&self, hash: &Sha256Digest) -> Result<Option<IndexedNote>, IndexError> {
        read_note(&self.conn, "sha256 = ?1", [hash.to_string()])
    }

    /// Returns the record with the given note ID.
    ///
    /// # Errors
    ///
    /// Fails when the database cannot be read or a stored value is invalid.
    pub fn get_by_id(&self, id: NoteId) -> Result<Option<IndexedNote>, IndexError> {
        read_note(&self.conn, "note_id = ?1", [i64::from(id.get())])
    }

    /// Returns the normalized OCR text stored for the given content hash.
    ///
    /// # Errors
    ///
    /// Fails when the database cannot be read.
    pub fn ocr_text(&self, hash: &Sha256Digest) -> Result<Option<OcrText>, IndexError> {
        let row: Option<(String, Option<String>)> = self
            .conn
            .query_row(
                "SELECT ocr_text, ocr_language FROM notes WHERE sha256 = ?1",
                [hash.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(row.map(|(text, language)| match language {
            Some(language) => OcrText::with_language(text, language),
            None => OcrText::new(text),
        }))
    }

    /// Returns every record in the default browsing order.
    ///
    /// The order is [`compare_library_order`]: records with IDs by ascending
    /// numeric ID, then records without IDs by title.
    ///
    /// # Errors
    ///
    /// Fails when the database cannot be read or a stored value is invalid.
    pub fn list_all(&self) -> Result<Vec<IndexedNote>, IndexError> {
        let mut notes = self
            .conn
            .prepare(&format!("SELECT {NOTE_COLUMNS} FROM notes"))?
            .query_map([], note_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        notes.sort_by(compare_library_order);
        Ok(notes)
    }

    /// Deletes every record whose path is not in `paths`.
    ///
    /// The pruned rows leave the FTS index in the same transaction, so a
    /// deleted file cannot still match a search.
    ///
    /// # Errors
    ///
    /// Fails when the database cannot be read.
    pub fn delete_missing(&self, paths: &HashSet<RelativePdfPath>) -> Result<(), IndexError> {
        let known: Vec<Box<dyn ToSql>> = paths
            .iter()
            .map(|path| Box::new(path.to_string()) as Box<dyn ToSql>)
            .collect();
        // An empty library has no paths to keep, and `NOT IN ()` is not valid
        // SQL, so the criterion degrades to "delete everything".
        let mut placeholders = String::new();
        for index in 1..=known.len() {
            let _ = write!(placeholders, ", ?{index}");
        }
        let kept = if placeholders.is_empty() {
            "TRUE".to_owned()
        } else {
            format!("path NOT IN ({})", placeholders.trim_start_matches(", "))
        };

        let transaction = self.conn.unchecked_transaction()?;
        transaction.execute(
            &format!(
                "DELETE FROM notes_fts WHERE rowid IN
                     (SELECT row_id FROM notes WHERE {kept})"
            ),
            params_from_iter(known.iter().map(|path| path.as_ref())),
        )?;
        transaction.execute(
            &format!("DELETE FROM notes WHERE {kept}"),
            params_from_iter(known.iter().map(|path| path.as_ref())),
        )?;
        transaction.commit()?;
        Ok(())
    }
}

/// Reads the single note matching a `WHERE` clause.
fn read_note(
    conn: &Connection,
    criterion: &str,
    parameter: impl rusqlite::Params,
) -> Result<Option<IndexedNote>, IndexError> {
    Ok(read_note_with_row_id(conn, criterion, parameter)?.map(|(_, note)| note))
}

/// Reads the single note matching a `WHERE` clause, with its row identifier.
///
/// The row identifier is what the derived FTS index is keyed by, so every write
/// needs it alongside the note it is rewriting.
fn read_note_with_row_id(
    conn: &Connection,
    criterion: &str,
    parameter: impl rusqlite::Params,
) -> Result<Option<(i64, IndexedNote)>, IndexError> {
    // The alias keeps the two `row_id` columns distinguishable by name.
    let query = format!("SELECT row_id AS note_row_id, {NOTE_COLUMNS} FROM notes WHERE {criterion}");
    Ok(conn
        .query_row(&query, parameter, |row| Ok((row.get("note_row_id")?, note_from_row(row)?)))
        .optional()?)
}

/// Rewrites the FTS5 entry of one note row.
fn write_fts_row(
    conn: &Connection,
    row_id: i64,
    note: &IndexedNote,
    ocr_text: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM notes_fts WHERE rowid = ?1", [row_id])?;
    conn.execute(
        "INSERT INTO notes_fts (rowid, note_id, title, composer, tags, filename, ocr_text)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            row_id,
            note.id().map(|id| i64::from(id.get())),
            note.title(),
            note.metadata().composer_or_artist(),
            tags_text(note),
            file_name(note.path()),
            ocr_text,
        ],
    )?;
    Ok(())
}

/// Maps a write failure onto the typed ID conflict when that is what happened.
///
/// The uniqueness violation is detected by SQLite; only the rejected note
/// identifies which ID was contested, so it is carried here.
fn classify_write_error(error: rusqlite::Error, note: &IndexedNote) -> IndexError {
    if let rusqlite::Error::SqliteFailure(failure, _) = &error
        && failure.code == rusqlite::ErrorCode::ConstraintViolation
        && failure.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
        && let Some(id) = note.id()
    {
        return IndexError::DuplicateNoteId(id);
    }
    IndexError::from(error)
}

/// Builds the bound parameters of [`UPSERT_NOTE`] from a note and its OCR text.
fn note_params(note: &IndexedNote, ocr: &OcrText) -> impl rusqlite::Params {
    let metadata = note.metadata();
    let modified = note.fingerprint().modified();
    // Owned values so the parameter list can outlive the borrow of `note`.
    params_from_iter(
        [
            Box::new(note.path().to_string()) as Box<dyn ToSql>,
            Box::new(note.fingerprint().sha256().to_string()),
            Box::new(
                note.fingerprint()
                    .byte_len()
                    .try_into()
                    .unwrap_or(i64::MAX),
            ),
            Box::new(modified.map(EpochTimestamp::seconds)),
            Box::new(modified.map(EpochTimestamp::subsec_nanos).map(i64::from)),
            Box::new(note.id().map(|id| i64::from(id.get()))),
            Box::new(metadata.title().to_owned()),
            Box::new(metadata.composer_or_artist().map(str::to_owned)),
            Box::new(tags_text(note)),
            Box::new(origin_text(metadata.provenance(), MetadataField::Id)),
            Box::new(origin_text(metadata.provenance(), MetadataField::Title)),
            Box::new(origin_text(metadata.provenance(), MetadataField::ComposerOrArtist)),
            Box::new(origin_text(metadata.provenance(), MetadataField::Tags)),
            Box::new(status_text(note.status())),
            Box::new(ocr.text().to_owned()),
            Box::new(ocr.language().map(str::to_owned)),
        ],
    )
}

/// Reads one `notes` row.
///
/// Columns are read by name so the same reader serves both the plain note
/// queries and the search query, which prepends its relevance column.
pub(crate) fn note_from_row(row: &Row<'_>) -> rusqlite::Result<IndexedNote> {
    let path_text: String = row.get("path")?;
    let sha256_text: String = row.get("sha256")?;
    let byte_len: i64 = row.get("byte_len")?;
    let modified_secs: Option<i64> = row.get("modified_secs")?;
    let modified_nanos: Option<i64> = row.get("modified_nanos")?;
    let note_id: Option<i64> = row.get("note_id")?;
    let title: String = row.get("title")?;
    let composer: Option<String> = row.get("composer_or_artist")?;
    let tags: String = row.get("tags")?;
    let provenance_id: String = row.get("provenance_id")?;
    let provenance_title: String = row.get("provenance_title")?;
    let provenance_composer: String = row.get("provenance_composer")?;
    let provenance_tags: String = row.get("provenance_tags")?;
    let status: String = row.get("status")?;

    let path = RelativePdfPath::new(path_text).map_err(|error| conversion("path", error))?;
    let sha256 = sha256_text
        .parse::<Sha256Digest>()
        .map_err(|error| conversion("sha256", error))?;
    let byte_len =
        u64::try_from(byte_len).map_err(|_| conversion("byte_len", "negative byte length"))?;
    let modified = match (modified_secs, modified_nanos) {
        (Some(secs), Some(nanos)) => Some(
            EpochTimestamp::new(secs, u32::try_from(nanos).unwrap_or_default())
                .ok_or_else(|| conversion("modified_nanos", "invalid timestamp"))?,
        ),
        (None, None) => None,
        _ => return Err(conversion("modified_secs", "half a timestamp")),
    };
    let id = match note_id {
        Some(value) => Some(NoteId::new(
            u32::try_from(value).map_err(|_| conversion("note_id", "negative note id"))?,
        )),
        None => None,
    };
    let tag_list = || {
        tags.split('\n')
            .filter(|tag| !tag.is_empty())
            .map(str::to_owned)
    };
    let mut metadata = NoteMetadata::new(&title, composer.clone(), tag_list())
        .map_err(|error| conversion("title", error))?;
    metadata.assign_id(
        id,
        origin_from_text(&provenance_id).ok_or_else(|| conversion("provenance_id", &provenance_id))?,
    );
    metadata
        .assign_title(
            &title,
            origin_from_text(&provenance_title)
                .ok_or_else(|| conversion("provenance_title", &provenance_title))?,
        )
        .map_err(|error| conversion("title", error))?;
    metadata.assign_composer_or_artist(
        composer,
        origin_from_text(&provenance_composer)
            .ok_or_else(|| conversion("provenance_composer", &provenance_composer))?,
    );
    metadata.assign_tags(
        tag_list(),
        origin_from_text(&provenance_tags)
            .ok_or_else(|| conversion("provenance_tags", &provenance_tags))?,
    );
    let mut note = IndexedNote::new(
        path,
        PdfFingerprint::new(sha256, byte_len, modified),
        metadata,
    );
    note.set_status(
        status_from_text(&status).ok_or_else(|| conversion("status", &status))?,
    );
    Ok(note)
}

/// Wraps a decoding failure as a row conversion error naming the column.
fn conversion(
    column: &str,
    reason: impl std::fmt::Display,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(StoreCorruption(format!("{column}: {reason}"))),
    )
}

/// Wraps a stored value that no longer parses.
#[derive(Debug, thiserror::Error)]
#[error("stored value is not valid: {0}")]
struct StoreCorruption(String);

/// Joins the normalized tags of a note for storage.
fn tags_text(note: &IndexedNote) -> String {
    note.metadata().tags().iter().cloned().collect::<Vec<_>>().join("\n")
}

/// Returns the last path segment, which is what a user types as a filename.
fn file_name(path: &RelativePdfPath) -> String {
    let path = path.to_string();
    match path.rsplit_once('/') {
        Some((_, name)) => name.to_owned(),
        None => path,
    }
}

/// Renders one field's provenance as stored.
fn origin_text(provenance: &FieldProvenance, field: MetadataField) -> &'static str {
    match provenance.origin_of(field) {
        MetadataOrigin::FilenameDerived => "filename_derived",
        MetadataOrigin::PcImported => "pc_imported",
        MetadataOrigin::Manual => "manual",
        MetadataOrigin::Unset => "unset",
    }
}

/// Parses a stored provenance marker.
fn origin_from_text(text: &str) -> Option<MetadataOrigin> {
    match text {
        "filename_derived" => Some(MetadataOrigin::FilenameDerived),
        "pc_imported" => Some(MetadataOrigin::PcImported),
        "manual" => Some(MetadataOrigin::Manual),
        "unset" => Some(MetadataOrigin::Unset),
        _ => None,
    }
}

/// Renders a status as stored.
fn status_text(status: IndexStatus) -> &'static str {
    match status {
        IndexStatus::Discovered => "discovered",
        IndexStatus::Indexing => "indexing",
        IndexStatus::Ready => "ready",
        IndexStatus::OcrPending => "ocr_pending",
        IndexStatus::OcrIncomplete => "ocr_incomplete",
        IndexStatus::Stale => "stale",
        IndexStatus::DuplicateIdConflict => "duplicate_id_conflict",
        IndexStatus::Inaccessible => "inaccessible",
        IndexStatus::InvalidPdf => "invalid_pdf",
        IndexStatus::ImportMismatch => "import_mismatch",
    }
}

/// Parses a stored status.
fn status_from_text(text: &str) -> Option<IndexStatus> {
    match text {
        "discovered" => Some(IndexStatus::Discovered),
        "indexing" => Some(IndexStatus::Indexing),
        "ready" => Some(IndexStatus::Ready),
        "ocr_pending" => Some(IndexStatus::OcrPending),
        "ocr_incomplete" => Some(IndexStatus::OcrIncomplete),
        "stale" => Some(IndexStatus::Stale),
        "duplicate_id_conflict" => Some(IndexStatus::DuplicateIdConflict),
        "inaccessible" => Some(IndexStatus::Inaccessible),
        "invalid_pdf" => Some(IndexStatus::InvalidPdf),
        "import_mismatch" => Some(IndexStatus::ImportMismatch),
        _ => None,
    }
}

/// Collapses whitespace runs so the stored OCR text is comparable and the
/// FTS5 tokenizer sees clean token boundaries.
fn normalize_ocr_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}