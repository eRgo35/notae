//! Schema versioning and migration application.
//!
//! Every database carries the version of the schema it was built with in the
//! `schema_migrations` table. Opening a database applies only the migrations
//! newer than that version, and the bookkeeping table is created inside the
//! same transaction as the migrations it records, so a failed migration leaves
//! the file exactly as it was.
//!
//! A recorded version is a claim, not a guarantee, so opening also checks that
//! every object the recorded version is supposed to have created is present. A
//! database whose bookkeeping survived but whose schema did not is refused at
//! open instead of failing later with a missing-table error.

use rusqlite::{Connection, OptionalExtension, params};

use crate::store::IndexError;

/// Version of the schema this build creates and expects.
pub const SCHEMA_VERSION: u32 = 1;

/// The bookkeeping table recording applied migrations.
const CREATE_MIGRATIONS_TABLE: &str = "\
CREATE TABLE IF NOT EXISTS schema_migrations (
    version    INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT";

/// Objects the schema at version 1 is made of, as `sqlite_master` type and
/// name.
///
/// Checked when opening, so a database that recorded version 1 without
/// actually having it is rejected. The FTS5 shadow tables are deliberately not
/// listed: they are SQLite's own bookkeeping for `notes_fts` and are dropped
/// and recreated with it.
const V1_OBJECTS: &[(&str, &str)] = &[
    ("table", "schema_migrations"),
    ("table", "notes"),
    ("table", "notes_fts"),
    ("index", "notes_note_id_unique"),
    ("index", "notes_status_idx"),
];

/// One forward-only schema change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    /// Version this migration produces; must be greater than the previous one.
    pub version: u32,
    /// Short identifier used in logs and error messages.
    pub name: &'static str,
    /// SQL executed inside the migration transaction.
    pub sql: &'static str,
}

/// Every migration, ordered by ascending version.
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "initial",
    sql: include_str!("../migrations/0001_initial.sql"),
}];

/// Returns the newest migration version recorded in `conn`.
///
/// A database with no `schema_migrations` table has applied nothing yet and
/// reports [`SCHEMA_VERSION`] minus one is not assumed: it reports `0`.
pub fn applied_version(conn: &Connection) -> Result<u32, IndexError> {
    let exists = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .is_some();
    if !exists {
        return Ok(0);
    }
    let version = conn.query_row(
        "SELECT max(version) FROM schema_migrations",
        [],
        |row| row.get::<_, Option<i64>>(0),
    )?;
    Ok(version.map_or(0, |value| u32::try_from(value).unwrap_or(u32::MAX)))
}

/// Applies every migration newer than the recorded version.
///
/// # Errors
///
/// Returns [`IndexError::UnsupportedSchemaVersion`] when the database was
/// written by a newer build: downgrading would corrupt data the caller cannot
/// interpret, so opening is refused instead. Returns
/// [`IndexError::IncompleteSchema`] when the recorded version names objects the
/// database does not have.
pub fn migrate(conn: &Connection) -> Result<u32, IndexError> {
    // Read the bookkeeping before creating it: a database that has never been
    // migrated has no table, which is version zero, not an error.
    let current = applied_version(conn)?;
    if current > SCHEMA_VERSION {
        return Err(IndexError::UnsupportedSchemaVersion {
            found: current,
            supported: SCHEMA_VERSION,
        });
    }

    let pending: Vec<&Migration> = MIGRATIONS
        .iter()
        .filter(|migration| migration.version > current)
        .collect();

    // One transaction for the whole batch, bookkeeping included. SQLite has
    // transactional DDL, so a failure anywhere — including the creation of the
    // bookkeeping table itself — leaves no trace of the attempt.
    let transaction = conn.unchecked_transaction()?;
    transaction.execute_batch(CREATE_MIGRATIONS_TABLE)?;
    for migration in pending {
        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version) VALUES (?1)",
            [i64::from(migration.version)],
        )?;
    }
    transaction.commit()?;

    verify_objects(conn, SCHEMA_VERSION)
}

/// Checks that the database really has the schema its version claims.
///
/// The bookkeeping table can outlive the schema it describes — a truncated
/// file, a partial copy, a hand-made file — and nothing in the version number
/// alone would notice. Every object the version is defined by is looked up in
/// `sqlite_master`, and the ones that are absent are named in the error.
fn verify_objects(conn: &Connection, version: u32) -> Result<u32, IndexError> {
    let mut missing: Vec<&str> = Vec::new();
    for (kind, name) in V1_OBJECTS {
        if !object_exists(conn, kind, name)? {
            missing.push(name);
        }
    }
    if missing.is_empty() {
        return Ok(version);
    }
    Err(IndexError::IncompleteSchema {
        version,
        missing: missing.join(", "),
    })
}

/// Returns whether one `sqlite_master` entry of the given type and name exists.
fn object_exists(conn: &Connection, kind: &str, name: &str) -> Result<bool, IndexError> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = ?1 AND name = ?2",
            params![kind, name],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .is_some())
}