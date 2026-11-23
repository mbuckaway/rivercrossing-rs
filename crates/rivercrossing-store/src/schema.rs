// SPDX-License-Identifier: GPL-3.0-only

//! The current SQLite schema and the version gate.
//!
//! [`apply_pragmas`] runs once per opened connection and must run before any
//! transaction. [`ensure_schema`] bootstraps the ledger and then creates,
//! keeps or upgrades the schema, or refuses a file stamped newer than this
//! build.

use rusqlite::{Connection, OptionalExtension};

use crate::error::StoreError;
use crate::migrations::run_migrations;

#[cfg(test)]
mod tests;

/// The schema version this build reads and writes.
pub const SCHEMA_VERSION: u32 = 3;

/// The migration ledger: one row, its id pinned to 1 by a CHECK.
pub(crate) const SCHEMA_VERSION_DDL: &str = "CREATE TABLE IF NOT EXISTS schema_version \
     (id INTEGER PRIMARY KEY CHECK (id = 1), version INTEGER NOT NULL)";

/// The current schema, in creation order, copied from the oracle DDL.
pub(crate) const SCHEMA_STATEMENTS: [&str; 8] = [
    r"
    CREATE TABLE ride (
        id                 INTEGER PRIMARY KEY,
        name               TEXT    NOT NULL,
        event_date         TEXT    NOT NULL,
        venue              TEXT    NOT NULL,
        course_name        TEXT    NOT NULL,
        lap_km             REAL    NOT NULL,
        organizer          TEXT    NOT NULL,
        scorer             TEXT    NOT NULL,
        logo_png           BLOB,
        planned_start      INTEGER NOT NULL,
        planned_duration_s INTEGER NOT NULL,
        actual_start       INTEGER,
        finished_at        INTEGER,
        status             TEXT    NOT NULL DEFAULT 'draft'
                             CHECK (status IN
                               ('draft', 'running', 'finished', 'reopened')),
        entry_mode         TEXT    NOT NULL
                             CHECK (entry_mode IN ('solo', 'mixed')),
        max_team_size      INTEGER NOT NULL,
        plate_model        TEXT    NOT NULL
                             CHECK (plate_model IN
                               ('rider_pooled', 'team_relay')),
        min_lap_s          INTEGER NOT NULL,
        deck_count         INTEGER NOT NULL,
        jokers_per_deck    INTEGER NOT NULL,
        jokers_mode        TEXT    NOT NULL DEFAULT 'total'
                             CHECK (jokers_mode IN ('per_deck', 'total')),
        max_cards          INTEGER,
        tiebreak_order     TEXT    NOT NULL,
        rng_seed           INTEGER NOT NULL,
        created_at         INTEGER NOT NULL,
        updated_at         INTEGER NOT NULL,
        hold_short_laps    INTEGER NOT NULL DEFAULT 1
    )
    ",
    r"
    CREATE TABLE entry (
        id           INTEGER PRIMARY KEY,
        ride_id      INTEGER NOT NULL REFERENCES ride(id),
        plate        TEXT    NOT NULL,
        key          TEXT    NOT NULL,
        display_name TEXT    NOT NULL,
        type         TEXT    NOT NULL CHECK (type IN ('solo', 'team')),
        team_size    INTEGER NOT NULL,
        status       TEXT    NOT NULL CHECK (status IN ('active', 'dnf')),
        dnf_at       INTEGER,
        notes        TEXT,
        logo_card    TEXT,
        retired      INTEGER NOT NULL DEFAULT 0
    )
    ",
    r"
    CREATE UNIQUE INDEX entry_plate_live_unique
        ON entry (ride_id, plate) WHERE retired = 0
    ",
    r"
    CREATE TABLE rider (
        id                INTEGER PRIMARY KEY,
        entry_id          INTEGER NOT NULL REFERENCES entry(id),
        first_name        TEXT    NOT NULL,
        last_name         TEXT    NOT NULL,
        plate             TEXT,
        sort_order        INTEGER NOT NULL,
        sex               TEXT    CHECK (sex IN ('M', 'F')),
        emergency_contact TEXT,
        waiver_signed     INTEGER,
        ccn_reg_id        TEXT
    )
    ",
    r"
    CREATE TABLE crossing (
        id          INTEGER PRIMARY KEY,
        ride_id     INTEGER NOT NULL REFERENCES ride(id),
        entry_id    INTEGER NOT NULL REFERENCES entry(id),
        rider_id    INTEGER REFERENCES rider(id),
        seq         INTEGER NOT NULL,
        crossed_at  INTEGER NOT NULL,
        lap_s       INTEGER NOT NULL,
        flag        TEXT    NOT NULL
                      CHECK (flag IN ('none', 'short', 'manual')),
        voided      INTEGER NOT NULL DEFAULT 0,
        void_reason TEXT,
        UNIQUE (entry_id, seq)
    )
    ",
    r"
    CREATE TABLE card (
        id          INTEGER PRIMARY KEY,
        ride_id     INTEGER NOT NULL REFERENCES ride(id),
        entry_id    INTEGER NOT NULL REFERENCES entry(id),
        crossing_id INTEGER REFERENCES crossing(id),
        shoe_index  INTEGER,
        rank        INTEGER NOT NULL
                      CHECK (rank = 0 OR rank BETWEEN 2 AND 14),
        suit        TEXT    CHECK (suit IN ('s', 'h', 'd', 'c')),
        state       TEXT    NOT NULL
                      CHECK (state IN ('held', 'dealt', 'voided')),
        dealt_at    INTEGER NOT NULL
    )
    ",
    r"
    CREATE TABLE app_session (
        id             INTEGER PRIMARY KEY,
        opened_at      INTEGER NOT NULL,
        closed_at      INTEGER,
        active_ride_id INTEGER REFERENCES ride(id),
        heartbeat_at   INTEGER
    )
    ",
    r"
    CREATE TABLE audit (
        id           INTEGER PRIMARY KEY,
        ride_id      INTEGER NOT NULL REFERENCES ride(id),
        at           INTEGER NOT NULL,
        action       TEXT    NOT NULL,
        payload_json TEXT    NOT NULL
    )
    ",
];

/// The five SQLite failure texts that a retried open may see.
const TRANSIENT_OPEN_ERRORS: [&str; 5] = [
    "disk I/O error",
    "database is locked",
    "database table is locked",
    "database or disk is full",
    "out of memory",
];

/// Apply the spec PRAGMAs to one connection.
///
/// Call once per opened connection, before any transaction: SQLite ignores
/// `PRAGMA foreign_keys` inside a transaction. `journal_mode` persists in the
/// file; `synchronous` and `foreign_keys` are per-connection settings.
///
/// # Errors
///
/// Return a [`StoreError::Sqlite`] failure if SQLite rejects a PRAGMA.
pub fn apply_pragmas(conn: &Connection) -> Result<(), StoreError> {
    conn.query_row("PRAGMA journal_mode = WAL", [], |row| {
        row.get::<_, String>(0)
    })?;
    conn.execute_batch("PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;")?;
    Ok(())
}

/// Bring the database to [`SCHEMA_VERSION`].
///
/// The ledger table is bootstrapped first, then the stamped version decides:
/// `0` creates the schema and stamps it in one transaction, the current
/// version is a no-op, a newer version is refused, and an older version runs
/// the migration chain.
///
/// # Errors
///
/// Return [`StoreError::SchemaVersionMismatch`] for a file stamped newer than
/// this build, [`StoreError::InvalidRow`] for an unreadable ledger value, and
/// a SQLite or migration failure when a step fails.
pub fn ensure_schema(conn: &mut Connection) -> Result<(), StoreError> {
    conn.execute_batch(SCHEMA_VERSION_DDL)?;
    let current = current_version(conn)?;
    if current == SCHEMA_VERSION {
        return Ok(());
    }
    if current == 0 {
        return create_schema(conn);
    }
    if current > SCHEMA_VERSION {
        return Err(StoreError::SchemaVersionMismatch {
            current,
            supported: SCHEMA_VERSION,
        });
    }
    run_migrations(conn, current, SCHEMA_VERSION)
}

/// Return the ledger's stamped version, or `0` when no row exists.
fn current_version(conn: &Connection) -> Result<u32, StoreError> {
    let stored = conn
        .query_row(
            "SELECT version FROM schema_version WHERE id = 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    match stored {
        None => Ok(0),
        Some(value) => u32::try_from(value).map_err(|_| StoreError::InvalidRow {
            table: "schema_version",
            column: "version",
            stored: value.to_string(),
        }),
    }
}

/// Create the current schema and stamp it, in one transaction.
fn create_schema(conn: &mut Connection) -> Result<(), StoreError> {
    let transaction = conn.transaction()?;
    for statement in SCHEMA_STATEMENTS {
        transaction.execute(statement, [])?;
    }
    transaction.execute(
        "INSERT OR REPLACE INTO schema_version (id, version) VALUES (1, ?1)",
        [SCHEMA_VERSION],
    )?;
    transaction.commit()?;
    Ok(())
}

/// Return whether *text* names a transient SQLite open failure.
///
/// `Store::open` retries on these five conditions only; a schema conflict or
/// a missing file fails on the first attempt. The predicate takes the error
/// text, so a unit test drives it without a real I/O fault.
#[must_use]
pub fn is_transient_open_error(text: &str) -> bool {
    TRANSIENT_OPEN_ERRORS
        .iter()
        .any(|needle| text.contains(needle))
}
