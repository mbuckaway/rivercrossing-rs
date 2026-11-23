// SPDX-License-Identifier: GPL-3.0-only

//! The store's error type.

use std::path::PathBuf;

use rivercrossing_cards::CardsError;
use rivercrossing_ride::RideError;
use rivercrossing_roster::error::RosterError;

#[cfg(test)]
mod tests;

/// A failure from the store, the schema or a migration.
///
/// `Debug` is derived. `PartialEq` is not, because [`std::io::Error`] is not
/// comparable; a test matches on the variant or on [`std::string::ToString`].
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The database's stamped schema version is newer than this build's.
    #[error(
        "Database schema version {current} does not match this build's schema \
         version {supported}. Rename or delete the database file to continue."
    )]
    SchemaVersionMismatch {
        /// The version the database's ledger holds.
        current: u32,
        /// The version this build reads and writes.
        supported: u32,
    },
    /// A version below [`crate::schema::SCHEMA_VERSION`] has no migration step.
    #[error("no schema migration from version {version}")]
    UnknownMigration {
        /// The version that names no registered step.
        version: u32,
    },
    /// A migration step failed.
    #[error("schema migration from version {from} to version {to} failed: {source}")]
    Migration {
        /// The version the migration started from.
        from: u32,
        /// The version the migration targeted.
        to: u32,
        /// The SQLite failure that aborted the step.
        source: rusqlite::Error,
    },
    /// A migration left foreign key violations behind.
    #[error("v1 -> v2 schema migration left foreign key violations in {tables}")]
    ForeignKeys {
        /// The distinct child tables that hold the violations, sorted.
        tables: String,
    },
    /// No ride holds the id.
    #[error("no ride with id {id}")]
    RideNotFound {
        /// The id that names no ride.
        id: i64,
    },
    /// The typed name does not match the ride's stored name.
    #[error("typed name {typed:?} does not match ride {id} name {name:?}")]
    RideNameMismatch {
        /// The ride that the caller named.
        id: i64,
        /// The name the caller typed.
        typed: String,
        /// The name the ride stores.
        name: String,
    },
    /// A ride is RUNNING, so a delete is refused.
    #[error("ride {id} is RUNNING and cannot be deleted")]
    RideRunning {
        /// The ride that is RUNNING.
        id: i64,
    },
    /// A stored tiebreak order names a rule this build does not know.
    #[error("ride {ride_id} stores a tiebreak order this build does not know: {stored}")]
    TiebreakOrder {
        /// The ride that holds the value.
        ride_id: i64,
        /// The stored tiebreak text.
        stored: String,
    },
    /// A stored ride row does not build a valid config.
    #[error("invalid ride config: {source}")]
    InvalidConfig {
        /// The config error.
        source: RideError,
    },
    /// A stored deck count and joker count do not build a valid shoe.
    #[error("invalid shoe: {source}")]
    InvalidShoe {
        /// The shoe error.
        source: CardsError,
    },
    /// A stored number does not fit the field's type.
    #[error("{field} value {value} exceeds the maximum {max}")]
    OutOfRange {
        /// The field that holds the number.
        field: &'static str,
        /// The stored number.
        value: i64,
        /// The largest value the field accepts.
        max: u32,
    },
    /// A stored roster row does not build a valid roster.
    #[error("invalid roster: {source}")]
    InvalidRoster {
        /// The roster error.
        source: RosterError,
    },
    /// An audit row does not replay.
    #[error("cannot replay audit row {row_id} for ride {ride_id}: {detail}")]
    Replay {
        /// The audit row that failed.
        row_id: i64,
        /// The ride that owns the row.
        ride_id: i64,
        /// The payload or engine failure, rendered as text.
        ///
        /// The text is a string because a payload can fail to decode before
        /// the ride engine ever sees it, so the failure is not always an
        /// engine error.
        detail: String,
    },
    /// An audit row's payload is not a JSON object.
    #[error("cannot replay audit row {row_id}: the payload is not a JSON object")]
    InvalidPayload {
        /// The audit row that holds the payload.
        row_id: i64,
    },
    /// A stored column holds a value this build does not know.
    #[error("{table}.{column} holds a value this build does not know: {stored}")]
    InvalidRow {
        /// The table that holds the column.
        table: &'static str,
        /// The column that holds the value.
        column: &'static str,
        /// The stored text.
        stored: String,
    },
    /// Another connection holds a transaction, so the checkpoint could not finish.
    #[error(
        "wal_checkpoint busy ({frames} frames not checkpointed); another connection \
         holds a transaction on the database"
    )]
    CheckpointBusy {
        /// The number of frames that the checkpoint left unwritten.
        frames: i32,
    },
    /// The backup rotation count must be at least one.
    #[error("keep must be at least 1")]
    KeepOutOfRange {
        /// The rotation count that the caller supplied.
        keep: usize,
    },
    /// There is no database file to back up.
    #[error("no database file to back up: {path}", path = .path.display())]
    MissingDatabase {
        /// The database path that holds no file.
        path: PathBuf,
    },
    /// A backup file does not exist.
    #[error("backup file not found: {path}", path = .path.display())]
    MissingBackupSource {
        /// The backup path that holds no file.
        path: PathBuf,
    },
    /// SQLite reported a failure.
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    /// The file system reported a failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// A JSON payload does not decode or encode.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
