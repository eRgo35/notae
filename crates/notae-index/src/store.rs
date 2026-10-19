//! SQLite-backed repository for indexed notes.
//!
//! [`IndexStore`] owns one synchronous connection. All writes go through
//! [`IndexStore::reindex_note`], [`IndexStore::reindex_note_with_ocr`], or
//! [`IndexStore::set_ocr`], which update the canonical `notes` row and rewrite
//! that row's entry in the derived FTS5 index in the same transaction.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;

use notae_domain::{
    EpochTimestamp, FieldProvenance, IndexStatus, IndexedNote, MetadataField, MetadataOrigin,
    NoteId, NoteMetadata, PdfFingerprint, RelativePdfPath, Sha256Digest, compare_library_order,
    normalize_whitespace,
};
use rusqlite::{Connection, OptionalExtension, Row, ToSql, params, params_from_iter};
use thiserror::Error;

use crate::fold::fold_l_stroke;
use crate::schema;
use crate::search::{SearchHit, SearchScope};

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
    /// The bookkeeping table names a version whose objects are not all present.
    ///
    /// A version row is a claim, not a guarantee: a database that recorded a
    /// migration but does not carry everything that migration created is
    /// unusable, and every later query would fail with a missing-table error
    /// far from its cause. Refusing the open keeps the failure at the file that
    /// is actually damaged.
    #[error("index schema version {version} is incomplete: {missing}")]
    IncompleteSchema {
        /// Version the bookkeeping claims.
        version: u32,
        /// Comma-separated names of the objects that are absent.
        missing: String,
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
/// it readable and directly comparable. The same holds for the `ł`→`l` fold of
/// `crate::fold`: it is applied to the derived `notes_fts` row only, never to
/// the stored text.
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

/// What one call to [`IndexStore::reindex_note`] did to the store.
///
/// The variants are exactly the distinctions a rescan has to make between a
/// file that moved and a file whose bytes changed, so a caller can report them
/// without re-deriving them from the database.
///
/// A record is reached by the path it is filed under — that is the file being
/// reported — and the record of the same content, wherever it is filed, is
/// consulted to tell a move from a change:
///
/// - Nothing holds this path and nothing holds this content: a first sighting —
///   [`Inserted`](ReindexOutcome::Inserted).
/// - This content is filed under a *different* path and nothing holds this
///   path: the file was moved or renamed without its bytes changing. The single
///   record follows the file, keeps its OCR text, and reports where it came
///   from — [`Moved`](ReindexOutcome::Moved).
/// - This path is filed under this same content: nothing about the file's
///   identity changed, only its metadata was refreshed —
///   [`Unchanged`](ReindexOutcome::Unchanged).
/// - This path is filed under *different* content: the file's bytes changed.
///   The record at that path keeps its identity — its row, its note ID, and
///   every field the caller supplies — and only its fingerprint is replaced —
///   [`Replaced`](ReindexOutcome::Replaced). OCR text is *not* carried over,
///   because text recognized from the previous bytes says nothing about the new
///   ones, so the replacement starts with empty OCR text.
///
/// [`ReindexOutcome::Replaced`] also reports the path that previously held the *new* content,
/// if there was one. Two files with identical bytes cannot both be indexed,
/// because the content hash is unique, so the record of the other path is
/// dropped and that content is now indexed once, at this path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReindexOutcome {
    /// The file was not indexed under this content hash or at this path.
    Inserted,
    /// The file was already indexed with this content hash at this path.
    Unchanged,
    /// The content was already indexed at another path and now lives here.
    Moved {
        /// Path the record was stored under before this call.
        from: RelativePdfPath,
    },
    /// The path held a record of other content, which this call replaced.
    Replaced {
        /// Content hash the record at this path carried before this call.
        previous: Sha256Digest,
        /// Path that already held the new content, if any; its record was
        /// dropped because one file's content can only be indexed once.
        duplicate_at: Option<RelativePdfPath>,
    },
}

/// Columns of `notes` read by [`note_from_row`], in a fixed order.
///
/// Every read uses this one list so the row reader and the query clauses stay
/// in step.
const NOTE_COLUMNS: &str = "row_id, path, sha256, byte_len, modified_secs, modified_nanos, \
     note_id, title, composer_or_artist, tags, provenance_id, provenance_title, \
     provenance_composer, provenance_tags, status, ocr_text, ocr_language";

/// Inserts the canonical note row.
///
/// Used when neither the content hash nor the path is already indexed, so no
/// conflict clause is needed and every constraint violation that SQLite can
/// raise here is a genuine one.
const INSERT_NOTE: &str = "\
INSERT INTO notes (
    path, sha256, byte_len, modified_secs, modified_nanos, note_id, title,
    composer_or_artist, tags, provenance_id, provenance_title, provenance_composer,
    provenance_tags, status, ocr_text, ocr_language, indexed_at
) VALUES (
    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
)";

/// Rewrites the canonical note row identified by `row_id`.
///
/// Every column the caller supplies is replaced, including `sha256`: this is
/// how a same-path content change updates the fingerprint while keeping the
/// record, its note ID, and its row identifier — and therefore its place in
/// the derived FTS index. Addressing the row explicitly, rather than keying on
/// the hash, is what lets the write move a record from one path to another or
/// swap the content at a path without colliding with the unique indexes.
const UPDATE_NOTE: &str = "\
UPDATE notes SET
    path = ?1,
    sha256 = ?2,
    byte_len = ?3,
    modified_secs = ?4,
    modified_nanos = ?5,
    note_id = ?6,
    title = ?7,
    composer_or_artist = ?8,
    tags = ?9,
    provenance_id = ?10,
    provenance_title = ?11,
    provenance_composer = ?12,
    provenance_tags = ?13,
    status = ?14,
    ocr_text = ?15,
    ocr_language = ?16,
    indexed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE row_id = ?17";

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

    /// Records `note`, discarding which kind of write it turned out to be.
    ///
    /// This is [`IndexStore::reindex_note`] for callers that do not need the
    /// [`ReindexOutcome`], such as a test or a one-off metadata correction. It
    /// resolves content hash and path the same way, so it updates a record
    /// whose content changed instead of refusing the write.
    ///
    /// # Errors
    ///
    /// Fails on any database error, including a duplicate note ID.
    pub fn upsert_note(&self, note: &IndexedNote) -> Result<(), IndexError> {
        self.upsert_note_with_ocr(note, None)
    }

    /// Records `note` and optionally replaces its OCR text.
    ///
    /// This is [`IndexStore::reindex_note_with_ocr`] for callers that do not
    /// need the [`ReindexOutcome`].
    ///
    /// # Errors
    ///
    /// Fails as [`IndexStore::upsert_note`] does.
    pub fn upsert_note_with_ocr(
        &self,
        note: &IndexedNote,
        ocr: Option<&OcrText>,
    ) -> Result<(), IndexError> {
        self.reindex_note_with_ocr(note, ocr).map(|_| ())
    }

    /// Records `note` as the current state of the file at its path.
    ///
    /// This is the write a rescan uses. It reports what it did as a
    /// [`ReindexOutcome`] instead of leaving the caller to compare rows, and it
    /// resolves the two ways a stored record can be reached — by content hash
    /// and by path — explicitly, because `path` and `sha256` are both unique
    /// and the two can disagree.
    ///
    /// The distinctions it draws, and how a caller should read them:
    ///
    /// - **Same hash, same path** — the file is unchanged; only its metadata
    ///   was rewritten, and the record keeps its OCR text.
    ///   ([`Unchanged`](ReindexOutcome::Unchanged))
    /// - **Same hash, different path** — the file was moved or renamed; the
    ///   record follows the file, keeps its OCR text, and reports where it
    ///   came from. ([`Moved`](ReindexOutcome::Moved))
    /// - **Same path, different hash** — the file's bytes changed; the record
    ///   at that path is kept, its note ID and metadata are replaced with the
    ///   supplied ones, and only its fingerprint and OCR text change. The old
    ///   hash ceases to be indexed.
    ///   ([`Replaced`](ReindexOutcome::Replaced))
    /// - **Neither** — a first sighting. ([`Inserted`](ReindexOutcome::Inserted))
    ///
    /// If the supplied content hash is already indexed under a *different* path
    /// as well, that other record is dropped: one file's content is indexed
    /// once, and [`ReindexOutcome::Replaced`] reports the path it was dropped
    /// from.
    ///
    /// A note ID claimed by a different record makes the whole write fail with
    /// [`IndexError::DuplicateNoteId`] and changes nothing; every other write in
    /// this method and the FTS rewrite it implies share one transaction.
    ///
    /// # Errors
    ///
    /// Fails on any database error, including a duplicate note ID.
    pub fn reindex_note(&self, note: &IndexedNote) -> Result<ReindexOutcome, IndexError> {
        self.reindex_note_with_ocr(note, None)
    }

    /// Records `note` and optionally replaces its OCR text.
    ///
    /// Passing `None` for `ocr` keeps whatever OCR text is already stored for
    /// this content hash, so a metadata write never silently drops OCR. That
    /// only applies when the hash is unchanged: text recognized from content
    /// that is no longer the file's describes nothing, so a
    /// [`Replaced`](ReindexOutcome::Replaced) record starts with empty OCR text.
    ///
    /// # Errors
    ///
    /// Fails as [`IndexStore::reindex_note`] does.
    pub fn reindex_note_with_ocr(
        &self,
        note: &IndexedNote,
        ocr: Option<&OcrText>,
    ) -> Result<ReindexOutcome, IndexError> {
        let transaction = self.conn.unchecked_transaction()?;
        let outcome = write_note(&transaction, note, ocr)?;
        transaction.commit()?;
        Ok(outcome)
    }

    /// Replaces the OCR text of the record with the given content hash.
    ///
    /// # Errors
    ///
    /// Returns [`IndexError::UnknownNote`] when no record has that hash.
    pub fn set_ocr(&self, hash: &Sha256Digest, ocr: &OcrText) -> Result<(), IndexError> {
        let transaction = self.conn.unchecked_transaction()?;
        let (row_id, note) =
            read_note_with_row_id(&transaction, "sha256 = ?1", [hash.to_string()])?
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
        self.search_in(query, SearchScope::Everything, limit)
    }

    /// Like [`IndexStore::search`], restricted to the columns of `scope`.
    ///
    /// # Errors
    ///
    /// Fails when the database cannot be read or holds a value that no longer parses.
    pub fn search_in(
        &self,
        query: &str,
        scope: SearchScope,
        limit: usize,
    ) -> Result<Vec<SearchHit>, IndexError> {
        crate::search::search(&self.conn, query, scope, limit)
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

    /// Returns the number of indexed notes without loading them.
    ///
    /// # Errors
    ///
    /// Fails when the database cannot be read.
    pub fn note_count(&self) -> Result<usize, IndexError> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))?;
        Ok(usize::try_from(count).unwrap_or_default())
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

/// Records `note` inside an open transaction, rewriting its FTS row.
///
/// `path` and `sha256` are both unique, so a record cannot be found by one key
/// alone: the hash says *which content* this is, the path says *which file* it
/// is, and a rescan can supply either one that disagrees with the stored state.
/// The record filed under this path is therefore resolved first — that is the
/// file being reported — and the record of the same content, wherever it is
/// filed, is resolved second, because only it can say whether the file was
/// moved or rewritten.
///
/// Whichever record is written, it is addressed by its row identifier rather
/// than by a conflict clause, so one statement can change the hash, change the
/// path, or change both without tripping the unique indexes.
fn write_note(
    conn: &Connection,
    note: &IndexedNote,
    ocr: Option<&OcrText>,
) -> Result<ReindexOutcome, IndexError> {
    // Read before writing: the record of the same content may be deleted below,
    // and its OCR text is exactly what the incoming content inherits.
    let stored_ocr = read_ocr(conn, &note.fingerprint().sha256())?;
    let ocr = match ocr {
        Some(text) => text.clone(),
        None => stored_ocr.unwrap_or_else(|| OcrText::new(String::new())),
    };

    // The record of the file being reported, addressed by its path.
    let at_path: Option<(i64, String)> = conn
        .query_row(
            "SELECT row_id, sha256 FROM notes WHERE path = ?1",
            [note.path().to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    // The record of the incoming content, wherever it is filed.
    let at_hash: Option<(i64, String)> = conn
        .query_row(
            "SELECT row_id, path FROM notes WHERE sha256 = ?1",
            [note.fingerprint().sha256().to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    let (row_id, outcome) = match (at_path, at_hash) {
        // A first sighting: neither this file nor this content is indexed.
        (None, None) => {
            conn.execute(INSERT_NOTE, params_from_iter(note_params(note, &ocr)?))
                .map_err(|error| classify_write_error(error, note))?;
            let row_id = last_insert_row_id(conn)?;
            write_fts_row(conn, row_id, note, ocr.text())?;
            return Ok(ReindexOutcome::Inserted);
        }
        // The content is indexed elsewhere and nothing holds this path: the
        // record follows the file, keeping its OCR text.
        (None, Some((row_id, other_path))) => (
            row_id,
            ReindexOutcome::Moved {
                from: RelativePdfPath::new(other_path)
                    .map_err(|error| conversion("path", error))?,
            },
        ),
        (Some((row_id, previous_hash)), at_hash) => {
            // The same content is also filed under another path. It cannot
            // stay there: the content hash is unique. The record of the file
            // being written survives and takes the new fingerprint, because
            // the incoming note describes that file.
            let duplicate_at = match at_hash {
                Some((other_row, other_path)) if other_row != row_id => {
                    delete_row(conn, other_row)?;
                    Some(
                        RelativePdfPath::new(other_path)
                            .map_err(|error| conversion("path", error))?,
                    )
                }
                _ => None,
            };
            let previous = previous_hash
                .parse::<Sha256Digest>()
                .map_err(|error| conversion("sha256", error))?;
            let outcome = if previous == note.fingerprint().sha256() {
                ReindexOutcome::Unchanged
            } else {
                ReindexOutcome::Replaced {
                    previous,
                    duplicate_at,
                }
            };
            (row_id, outcome)
        }
    };

    conn.execute(UPDATE_NOTE, update_params(note, &ocr, row_id)?)
        .map_err(|error| classify_write_error(error, note))?;
    write_fts_row(conn, row_id, note, ocr.text())?;
    Ok(outcome)
}

/// Deletes one note row and its FTS entry.
fn delete_row(conn: &Connection, row_id: i64) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM notes_fts WHERE rowid = ?1", [row_id])?;
    conn.execute("DELETE FROM notes WHERE row_id = ?1", [row_id])?;
    Ok(())
}

/// Returns the row identifier of the most recent insert on this connection.
fn last_insert_row_id(conn: &Connection) -> Result<i64, rusqlite::Error> {
    conn.query_row("SELECT last_insert_rowid()", [], |row| row.get(0))
}

/// Reads the normalized OCR text stored for a content hash.
fn read_ocr(conn: &Connection, hash: &Sha256Digest) -> Result<Option<OcrText>, IndexError> {
    let row: Option<(String, Option<String>)> = conn
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
    let query =
        format!("SELECT row_id AS note_row_id, {NOTE_COLUMNS} FROM notes WHERE {criterion}");
    Ok(conn
        .query_row(&query, parameter, |row| {
            Ok((row.get("note_row_id")?, note_from_row(row)?))
        })
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
    // Every indexed column goes through the same fold the query uses; see
    // `crate::fold`. The canonical `notes` row keeps the original spelling.
    conn.execute(
        "INSERT INTO notes_fts (rowid, note_id, title, composer, tags, filename, ocr_text)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            row_id,
            note.id().map(|id| i64::from(id.get())),
            fold_l_stroke(note.title()),
            note.metadata().composer_or_artist().map(fold_l_stroke),
            fold_l_stroke(&tags_text(note)),
            fold_l_stroke(&file_name(note.path())),
            fold_l_stroke(ocr_text),
        ],
    )?;
    Ok(())
}

/// Maps a write failure onto the typed ID conflict when that is what happened.
///
/// The uniqueness violation is detected by SQLite; only the rejected note
/// identifies which ID was contested, so it is carried here.
///
/// [`write_note`] resolves the `path` and `sha256` constraints before it
/// writes, so the only unique constraint that can still fire is the note ID.
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

/// Builds the bound parameters of [`UPDATE_NOTE`] from a note, its OCR text,
/// and the row to write.
fn update_params(
    note: &IndexedNote,
    ocr: &OcrText,
    row_id: i64,
) -> Result<impl rusqlite::Params, IndexError> {
    Ok(params_from_iter(note_params(note, ocr)?.into_iter().chain(
        std::iter::once(Box::new(row_id) as Box<dyn ToSql>),
    )))
}

/// Builds the bound parameters of [`INSERT_NOTE`] from a note and its OCR text.
fn note_params(note: &IndexedNote, ocr: &OcrText) -> Result<Vec<Box<dyn ToSql>>, IndexError> {
    let metadata = note.metadata();
    let modified = note.fingerprint().modified();
    Ok(vec![
        Box::new(note.path().to_string()) as Box<dyn ToSql>,
        Box::new(note.fingerprint().sha256().to_string()),
        Box::new(
            i64::try_from(note.fingerprint().byte_len())
                .map_err(|_| conversion("byte_len", "byte length exceeds the stored range"))?,
        ),
        Box::new(modified.map(EpochTimestamp::seconds)),
        Box::new(modified.map(EpochTimestamp::subsec_nanos).map(i64::from)),
        Box::new(note.id().map(|id| i64::from(id.get()))),
        Box::new(metadata.title().to_owned()),
        Box::new(metadata.composer_or_artist().map(str::to_owned)),
        Box::new(tags_text(note)),
        Box::new(origin_text(metadata.provenance(), MetadataField::Id)),
        Box::new(origin_text(metadata.provenance(), MetadataField::Title)),
        Box::new(origin_text(
            metadata.provenance(),
            MetadataField::ComposerOrArtist,
        )),
        Box::new(origin_text(metadata.provenance(), MetadataField::Tags)),
        Box::new(status_text(note.status())),
        Box::new(ocr.text().to_owned()),
        Box::new(ocr.language().map(str::to_owned)),
    ])
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
        origin_from_text(&provenance_id)
            .ok_or_else(|| conversion("provenance_id", &provenance_id))?,
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
    note.set_status(status_from_text(&status).ok_or_else(|| conversion("status", &status))?);
    Ok(note)
}

/// Wraps a decoding failure as a row conversion error naming the column.
fn conversion(column: &str, reason: impl std::fmt::Display) -> rusqlite::Error {
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
    note.metadata()
        .tags()
        .iter()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
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
    normalize_whitespace(text)
}
