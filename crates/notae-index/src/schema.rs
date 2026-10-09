//! Schema versioning and migration application.
//!
//! Every database carries the version of the schema it was built with in the
//! `schema_migrations` table. Opening a database applies only the migrations
//! newer than that version, inside a single transaction, so a failed migration
//! leaves the file exactly as it was.

use rusqlite::{Connection, OptionalExtension};

use crate::store::IndexError;

/// Version of the schema this build creates and expects.
pub const SCHEMA_VERSION: u32 = 1;

/// The bookkeeping table recording applied migrations.
const CREATE_MIGRATIONS_TABLE: &str = "\
CREATE TABLE IF NOT EXISTS schema_migrations (
    version    INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT";

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
/// interpret, so opening is refused instead.
pub fn migrate(conn: &Connection) -> Result<u32, IndexError> {
    conn.execute_batch(CREATE_MIGRATIONS_TABLE)?;
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
    if pending.is_empty() {
        return Ok(current);
    }

    // One transaction for the whole batch: SQLite has transactional DDL, so a
    // failure halfway through leaves no half-applied schema behind.
    let transaction = conn.unchecked_transaction()?;
    for migration in pending {
        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version) VALUES (?1)",
            [i64::from(migration.version)],
        )?;
    }
    transaction.commit()?;
    Ok(SCHEMA_VERSION)
}