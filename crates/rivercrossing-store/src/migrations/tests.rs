// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the frozen migration chain.
//!
//! Every test writes a real database file from frozen DDL, then drives
//! `ensure_schema` or `run_migrations` and reads the file back through a
//! second connection. No binary fixture is checked into the tree.

use std::collections::BTreeSet;
use std::error::Error;
use std::path::Path;

use pretty_assertions::assert_eq;
use rusqlite::types::Value;
use rusqlite::{Connection, ErrorCode, params};
use uuid::Uuid;

use super::{MIGRATIONS, run_migrations};
use crate::error::StoreError;
use crate::schema::{
    SCHEMA_STATEMENTS, SCHEMA_VERSION, SCHEMA_VERSION_DDL, apply_pragmas, ensure_schema,
};

// ------------------------------------------------- frozen v1 schema and rows

/// The released v1 ledger, frozen.
const V1_LEDGER_DDL: &str = "CREATE TABLE schema_version (id INTEGER PRIMARY KEY CHECK (id = 1), \
     version INTEGER NOT NULL)";

/// The released v1 schema, frozen: `entry` still carries the inline
/// `UNIQUE (ride_id, plate)` and no `key` or `retired` column.
const V1_DDL: [&str; 7] = [
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
        display_name TEXT    NOT NULL,
        type         TEXT    NOT NULL CHECK (type IN ('solo', 'team')),
        team_size    INTEGER NOT NULL,
        status       TEXT    NOT NULL CHECK (status IN ('active', 'dnf')),
        dnf_at       INTEGER,
        notes        TEXT,
        logo_card    TEXT,
        UNIQUE (ride_id, plate)
    )
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

const V1_RIDE_ROW: &str = "
    INSERT INTO ride (
        id, name, event_date, venue, course_name, lap_km, organizer, scorer,
        logo_png, planned_start, planned_duration_s, actual_start, finished_at,
        status, entry_mode, max_team_size, plate_model, min_lap_s, deck_count,
        jokers_per_deck, jokers_mode, max_cards, tiebreak_order, rng_seed,
        created_at, updated_at, hold_short_laps
    ) VALUES (
        1, 'GORBA EPIC 2026', '2026-09-20', 'Sea to Sky Gondola', 'Gondola Loop',
        8.0, 'GORBA', 'K. Singh', NULL, 1789898400, 21600, 1789898400, NULL,
        'running', 'mixed', 4, 'rider_pooled', 1080, 8, 2, 'total', NULL,
        '[\"laps\",\"total_time\",\"high_card\"]', 20260920, 1789898400, 1789898400, 1
    )
";

const V1_ENTRY_ROWS: [&str; 3] = [
    "
    INSERT INTO entry (id, ride_id, plate, display_name, type, team_size,
                       status, dnf_at, notes, logo_card)
    VALUES (1, 1, '12', 'Alice', 'solo', 1, 'active', NULL, '', NULL)
    ",
    "
    INSERT INTO entry (id, ride_id, plate, display_name, type, team_size,
                       status, dnf_at, notes, logo_card)
    VALUES (2, 1, '45', 'Dirt Dynamos', 'team', 2, 'active', NULL,
            'carry a tail light', NULL)
    ",
    "
    INSERT INTO entry (id, ride_id, plate, display_name, type, team_size,
                       status, dnf_at, notes, logo_card)
    VALUES (3, 1, '9', 'Bo', 'solo', 1, 'dnf', 1789899000, 'cramp', 'As')
    ",
];

const V1_CHILD_ROWS: [&str; 13] = [
    "
    INSERT INTO rider (id, entry_id, first_name, last_name, plate, sort_order,
                       sex, emergency_contact, waiver_signed, ccn_reg_id)
    VALUES (1, 1, 'Alice', 'Wong', '12', 0, 'F', '604-555-0101', 1, 'CCN-77')
    ",
    "
    INSERT INTO rider (id, entry_id, first_name, last_name, plate, sort_order,
                       sex, emergency_contact, waiver_signed, ccn_reg_id)
    VALUES (2, 2, 'Sarah', 'Roy', '45', 0, 'F', NULL, NULL, NULL)
    ",
    "
    INSERT INTO rider (id, entry_id, first_name, last_name, plate, sort_order,
                       sex, emergency_contact, waiver_signed, ccn_reg_id)
    VALUES (3, 2, 'Priya', 'Nair', '46', 1, 'F', NULL, NULL, NULL)
    ",
    "
    INSERT INTO rider (id, entry_id, first_name, last_name, plate, sort_order,
                       sex, emergency_contact, waiver_signed, ccn_reg_id)
    VALUES (4, 3, 'Bo', 'Diaz', '9', 0, 'M', NULL, NULL, NULL)
    ",
    "
    INSERT INTO crossing (id, ride_id, entry_id, rider_id, seq, crossed_at,
                          lap_s, flag, voided, void_reason)
    VALUES (1, 1, 1, 1, 1, 1789898520, 120, 'none', 0, NULL)
    ",
    "
    INSERT INTO crossing (id, ride_id, entry_id, rider_id, seq, crossed_at,
                          lap_s, flag, voided, void_reason)
    VALUES (2, 1, 1, 1, 2, 1789898640, 132, 'none', 0, NULL)
    ",
    "
    INSERT INTO crossing (id, ride_id, entry_id, rider_id, seq, crossed_at,
                          lap_s, flag, voided, void_reason)
    VALUES (3, 1, 2, 2, 1, 1789898700, 180, 'short', 1, 'cut the loop')
    ",
    "
    INSERT INTO card (id, ride_id, entry_id, crossing_id, shoe_index, rank,
                      suit, state, dealt_at)
    VALUES (1, 1, 1, 1, 0, 14, 's', 'dealt', 1789898520)
    ",
    "
    INSERT INTO card (id, ride_id, entry_id, crossing_id, shoe_index, rank,
                      suit, state, dealt_at)
    VALUES (2, 1, 1, 2, 1, 3, 'h', 'dealt', 1789898640)
    ",
    "
    INSERT INTO card (id, ride_id, entry_id, crossing_id, shoe_index, rank,
                      suit, state, dealt_at)
    VALUES (3, 1, 2, 3, 2, 0, NULL, 'held', 1789898700)
    ",
    "
    INSERT INTO app_session (id, opened_at, closed_at, active_ride_id, heartbeat_at)
    VALUES (1, 1789898000, NULL, 1, 1789898900)
    ",
    "
    INSERT INTO audit (id, ride_id, at, action, payload_json)
    VALUES (1, 1, 1789898400, 'start_ride', '{\"source\": \"setup\"}')
    ",
    "
    INSERT INTO audit (id, ride_id, at, action, payload_json)
    VALUES (2, 1, 1789898400, 'start', '{\"actual_start\": \"2026-09-20T10:00:00\"}')
    ",
];

const V1_CROSSING_AUDIT_ROW: &str = "
    INSERT INTO audit (id, ride_id, at, action, payload_json)
    VALUES (3, 1, 1789898520, 'record_crossing',
            '{\"plate\": \"12\", \"entry_id\": \"12\", \"lap\": 1,
              \"crossed_at\": \"2026-09-20T10:02:00\", \"reason\": \"Alice\"}')
";

const V1_SINGLE_ENTRY_SEED: [&str; 2] = [V1_RIDE_ROW, V1_ENTRY_ROWS[0]];

const V1_ORPHAN_SEED: [&str; 2] = [
    V1_RIDE_ROW,
    "
    INSERT INTO rider (id, entry_id, first_name, last_name, plate, sort_order)
    VALUES (1, 7, 'Ghost', 'Rider', '7', 0)
    ",
];

/// Two rider rows and one crossing row that name entries which do not exist,
/// so the check must dedupe the rider table and sort both names.
const V1_MULTI_ORPHAN_SEED: [&str; 4] = [
    V1_RIDE_ROW,
    "
    INSERT INTO rider (id, entry_id, first_name, last_name, plate, sort_order)
    VALUES (1, 7, 'Ghost', 'Rider', '7', 0)
    ",
    "
    INSERT INTO rider (id, entry_id, first_name, last_name, plate, sort_order)
    VALUES (2, 8, 'Other', 'Ghost', '8', 0)
    ",
    "
    INSERT INTO crossing (id, ride_id, entry_id, seq, crossed_at, lap_s, flag)
    VALUES (1, 1, 7, 1, 1789898520, 120, 'none')
    ",
];

const V1_ENTRY_COLUMNS: [&str; 10] = [
    "id",
    "ride_id",
    "plate",
    "display_name",
    "type",
    "team_size",
    "status",
    "dnf_at",
    "notes",
    "logo_card",
];

const V2_ENTRY_COLUMNS: [&str; 12] = [
    "id",
    "ride_id",
    "plate",
    "key",
    "display_name",
    "type",
    "team_size",
    "status",
    "dnf_at",
    "notes",
    "logo_card",
    "retired",
];

// --------------------------------------------------------------- fixtures

/// Write a released-build v1 database file, then *seed*'s rows.
///
/// Foreign keys are turned off first: the bundled SQLite builds with
/// `SQLITE_DEFAULT_FOREIGN_KEYS=1`, while the v1 writer left them off, and
/// the orphan fixture depends on that.
fn write_v1_file(path: &Path, seed: &[&str]) -> Result<(), Box<dyn Error>> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA foreign_keys = OFF")?;
    for statement in V1_DDL.iter().chain([V1_LEDGER_DDL].iter()) {
        conn.execute_batch(statement)?;
    }
    conn.execute_batch("INSERT INTO schema_version (id, version) VALUES (1, 1)")?;
    for statement in seed {
        conn.execute_batch(statement)?;
    }
    conn.close().map_err(|(_, error)| error)?;
    Ok(())
}

/// Open *path*, apply the PRAGMAs and bring the schema to the current version.
fn ensure_current(path: &Path) -> Result<(), StoreError> {
    let mut conn = Connection::open(path)?;
    apply_pragmas(&conn)?;
    ensure_schema(&mut conn)?;
    Ok(())
}

/// Run one query against *path* through a new connection, and map its rows.
fn rows<T>(
    path: &Path,
    sql: &str,
    map: impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> rusqlite::Result<Vec<T>> {
    let conn = Connection::open(path)?;
    let mut statement = conn.prepare(sql)?;
    let mapped = statement.query_map([], map)?;
    mapped.collect()
}

/// Return one scalar from *path*.
fn scalar<T: rusqlite::types::FromSql>(path: &Path, sql: &str) -> rusqlite::Result<T> {
    let conn = Connection::open(path)?;
    conn.query_row(sql, [], |row| row.get(0))
}

/// Return the ledger's stamped version.
fn version(path: &Path) -> rusqlite::Result<i64> {
    scalar(path, "SELECT version FROM schema_version WHERE id = 1")
}

/// One `PRAGMA table_info` row: cid, name, type, notnull, default, pk.
type TableInfoRow = (i64, String, String, i64, Option<String>, i64);

/// One `PRAGMA index_list` row: seq, name, unique, origin, partial.
type IndexListRow = (i64, String, i64, String, i64);

/// One copied `entry` row, in the v1 column order.
type EntryCopy = (
    i64,
    i64,
    String,
    String,
    String,
    i64,
    String,
    Option<i64>,
    Option<String>,
    Option<String>,
);

/// Return one table's `PRAGMA table_info` rows, in column order.
fn table_info_rows(path: &Path, table: &str) -> rusqlite::Result<Vec<TableInfoRow>> {
    let sql = format!("PRAGMA table_info({table})");
    rows(path, &sql, |row| {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
        ))
    })
}

/// Return one table's column names, in order.
fn column_names(path: &Path, table: &str) -> rusqlite::Result<Vec<String>> {
    Ok(table_info_rows(path, table)?
        .into_iter()
        .map(|(_, name, _, _, _, _)| name)
        .collect())
}

/// Return one child table's `(id, entry_id)` rows, in id order.
fn entry_links(path: &Path, table: &str) -> rusqlite::Result<Vec<(i64, i64)>> {
    rows(
        path,
        &format!("SELECT id, entry_id FROM {table} ORDER BY id"),
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
}

/// Return one table's `PRAGMA index_list` rows.
fn index_list_rows(path: &Path, table: &str) -> rusqlite::Result<Vec<IndexListRow>> {
    let sql = format!("PRAGMA index_list({table})");
    rows(path, &sql, |row| {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
        ))
    })
}

/// Return one audit row's payload, decoded.
fn payload(path: &Path, audit_id: i64) -> Result<serde_json::Value, Box<dyn Error>> {
    let raw: String = scalar(
        path,
        &format!("SELECT payload_json FROM audit WHERE id = {audit_id}"),
    )?;
    Ok(serde_json::from_str(&raw)?)
}

/// Return one payload field as text.
fn text_field<'a>(payload: &'a serde_json::Value, field: &str) -> Option<&'a str> {
    payload.get(field).and_then(serde_json::Value::as_str)
}

/// Return every `(id, payload_json)` audit row, in id order.
fn audit_rows(path: &Path) -> rusqlite::Result<Vec<(i64, String)>> {
    rows(
        path,
        "SELECT id, payload_json FROM audit ORDER BY id",
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
}

// A v2 file: the current DDL (v3 changes no DDL) stamped 2, the released
// ride row, then the caller's entry and audit rows.

/// A v2 `entry` row for the fixture.
#[derive(Default)]
struct V2Entry {
    id: i64,
    ride_id: i64,
    plate: &'static str,
    key: &'static str,
    display_name: &'static str,
    entry_type: &'static str,
    team_size: i64,
    retired: i64,
}

/// A v2 `audit` row for the fixture.
struct V2Audit {
    id: i64,
    ride_id: i64,
    at: i64,
    action: &'static str,
    payload: &'static str,
}

/// The rows and extra rides of one v2 database file.
#[derive(Default)]
struct V2Seed<'a> {
    entries: &'a [V2Entry],
    audit: &'a [V2Audit],
    extra_rides: &'a [&'a str],
}

/// Write a v2 database file stamped version 2.
fn write_v2_file(path: &Path, seed: &V2Seed<'_>) -> Result<(), Box<dyn Error>> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA foreign_keys = OFF")?;
    for statement in SCHEMA_STATEMENTS {
        conn.execute_batch(statement)?;
    }
    conn.execute_batch(SCHEMA_VERSION_DDL)?;
    conn.execute_batch("INSERT INTO schema_version (id, version) VALUES (1, 2)")?;
    conn.execute_batch(V1_RIDE_ROW)?;
    for statement in seed.extra_rides {
        conn.execute_batch(statement)?;
    }
    for entry in seed.entries {
        conn.execute(
            "INSERT INTO entry (id, ride_id, plate, key, display_name, type,
                                team_size, status, dnf_at, notes, logo_card,
                                retired)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', NULL, '', NULL, ?8)",
            params![
                entry.id,
                entry.ride_id,
                entry.plate,
                entry.key,
                entry.display_name,
                entry.entry_type,
                entry.team_size,
                entry.retired,
            ],
        )?;
    }
    for row in seed.audit {
        conn.execute(
            "INSERT INTO audit (id, ride_id, at, action, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![row.id, row.ride_id, row.at, row.action, row.payload],
        )?;
    }
    conn.close().map_err(|(_, error)| error)?;
    Ok(())
}

const ALICE_KEY: &str = "0f1e2d3c4b5a49788796a5b4c3d2e1f0";
const DYNAMOS_KEY: &str = "1a2b3c4d5e6f408192a3b4c5d6e7f809";
const RIDE2_KEY: &str = "2b3c4d5e6f704192a3b4c5d6e7f8091a";

const V2_ALICE: V2Entry = V2Entry {
    id: 1,
    ride_id: 1,
    plate: "12",
    key: ALICE_KEY,
    display_name: "Alice",
    entry_type: "solo",
    team_size: 1,
    retired: 0,
};

const V2_DYNAMOS: V2Entry = V2Entry {
    id: 2,
    ride_id: 1,
    plate: "45",
    key: DYNAMOS_KEY,
    display_name: "Dirt Dynamos",
    entry_type: "team",
    team_size: 2,
    retired: 0,
};

const V2_RETIRED_ALICE: V2Entry = V2Entry {
    id: 1,
    ride_id: 1,
    plate: "12",
    key: ALICE_KEY,
    display_name: "Alice",
    entry_type: "solo",
    team_size: 1,
    retired: 1,
};

const V2_RIDE2_ALICE: V2Entry = V2Entry {
    id: 3,
    ride_id: 2,
    plate: "12",
    key: RIDE2_KEY,
    display_name: "Alice",
    entry_type: "solo",
    team_size: 1,
    retired: 0,
};

const V2_START: V2Audit = V2Audit {
    id: 1,
    ride_id: 1,
    at: 1_789_898_400,
    action: "start",
    payload: "{\"actual_start\": \"2026-09-20T10:00:00\"}",
};

const V2_LEGACY_CROSSING: V2Audit = V2Audit {
    id: 2,
    ride_id: 1,
    at: 1_789_898_520,
    action: "record_crossing",
    payload: "{\"plate\": \"12\", \"entry_id\": \"12\", \"lap\": 1, \
              \"crossed_at\": \"2026-09-20T10:02:00\", \"reason\": \"Alice\"}",
};

const V2_REASSIGN: V2Audit = V2Audit {
    id: 2,
    ride_id: 1,
    at: 1_789_898_520,
    action: "reassign",
    payload: "{\"seq\": 1, \"old_entry_id\": \"12\", \"new_entry_id\": \"45\", \
              \"new_plate\": \"45\", \"reason\": \"moved\"}",
};

const V2_UNKNOWN_IDENTITY: V2Audit = V2Audit {
    id: 3,
    ride_id: 1,
    at: 1_789_898_580,
    action: "dnf",
    payload: "{\"entry_id\": \"99\", \"plate\": \"99\", \"rider\": false, \
              \"reason\": \"no show\"}",
};

const V2_ALREADY_KEYED: V2Audit = V2Audit {
    id: 4,
    ride_id: 1,
    at: 1_789_898_640,
    action: "void_card",
    payload: "{\"entry_id\": \"0f1e2d3c4b5a49788796a5b4c3d2e1f0\", \
              \"card\": \"As\", \"reason\": \"duplicate\"}",
};

const V2_NON_STRING_IDENTITY: V2Audit = V2Audit {
    id: 5,
    ride_id: 1,
    at: 1_789_898_700,
    action: "record_crossing",
    payload: "{\"entry_id\": 93, \"plate\": \"93\", \"reason\": \"hand-edited\"}",
};

const V2_MALFORMED: V2Audit = V2Audit {
    id: 4,
    ride_id: 1,
    at: 1_789_898_640,
    action: "record_crossing",
    payload: "not json at all",
};

const V2_NON_OBJECT: V2Audit = V2Audit {
    id: 5,
    ride_id: 1,
    at: 1_789_898_700,
    action: "record_crossing",
    payload: "[]",
};

const V2_SECOND_RIDE_CROSSING: V2Audit = V2Audit {
    id: 6,
    ride_id: 2,
    at: 1_789_898_520,
    action: "record_crossing",
    payload: "{\"plate\": \"12\", \"entry_id\": \"12\", \"lap\": 1, \
              \"reason\": \"Alice\"}",
};

const V2_SECOND_RIDE_ROW: &str = "
    INSERT INTO ride
    SELECT 2, 'GORBA EPIC 2026 (2)', event_date, venue, course_name, lap_km,
           organizer, scorer, logo_png, planned_start, planned_duration_s,
           actual_start, finished_at, status, entry_mode, max_team_size,
           plate_model, min_lap_s, deck_count, jokers_per_deck, jokers_mode,
           max_cards, tiebreak_order, rng_seed, created_at, updated_at,
           hold_short_laps
      FROM ride WHERE id = 1
";

// -------------------------------------------------------- v1 to v2 migration

#[test]
fn test_migrations_ensure_schema_upgrades_a_v1_file_to_version_three() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v1.db");
    let seed = [
        V1_RIDE_ROW,
        V1_ENTRY_ROWS[0],
        V1_ENTRY_ROWS[1],
        V1_ENTRY_ROWS[2],
    ];
    write_v1_file(&path, &seed)?;

    ensure_current(&path)?;

    assert_eq!(version(&path)?, i64::from(SCHEMA_VERSION));
    Ok(())
}

#[test]
fn test_migrations_v1_to_v2_rebuilds_entry_in_the_v2_column_order() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v1.db");
    write_v1_file(&path, &[V1_RIDE_ROW, V1_ENTRY_ROWS[0]])?;

    ensure_current(&path)?;

    assert_eq!(column_names(&path, "entry")?, V2_ENTRY_COLUMNS);
    Ok(())
}

#[test]
fn test_migrations_v1_to_v2_mints_a_fresh_uuid4_key_per_entry() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v1.db");
    let seed = [
        V1_RIDE_ROW,
        V1_ENTRY_ROWS[0],
        V1_ENTRY_ROWS[1],
        V1_ENTRY_ROWS[2],
    ];
    write_v1_file(&path, &seed)?;

    ensure_current(&path)?;

    let keys: Vec<String> = rows(&path, "SELECT key FROM entry ORDER BY id", |row| row.get(0))?;
    let shaped: Vec<bool> = keys
        .iter()
        .map(|key| {
            Uuid::parse_str(key)
                .is_ok_and(|uuid| uuid.get_version_num() == 4 && uuid.simple().to_string() == *key)
        })
        .collect();
    let distinct: BTreeSet<&String> = keys.iter().collect();

    assert_eq!((shaped, distinct.len(), keys.len()), (vec![true; 3], 3, 3));
    Ok(())
}

#[test]
fn test_migrations_v1_to_v2_marks_every_migrated_entry_live() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v1.db");
    let seed = [
        V1_RIDE_ROW,
        V1_ENTRY_ROWS[0],
        V1_ENTRY_ROWS[1],
        V1_ENTRY_ROWS[2],
    ];
    write_v1_file(&path, &seed)?;

    ensure_current(&path)?;

    let retired: Vec<(i64, i64)> =
        rows(&path, "SELECT id, retired FROM entry ORDER BY id", |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
    assert_eq!(retired, [(1, 0), (2, 0), (3, 0)]);
    Ok(())
}

#[test]
fn test_migrations_v1_to_v2_preserves_every_v1_entry_column() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v1.db");
    let seed = [
        V1_RIDE_ROW,
        V1_ENTRY_ROWS[0],
        V1_ENTRY_ROWS[1],
        V1_ENTRY_ROWS[2],
    ];
    write_v1_file(&path, &seed)?;

    ensure_current(&path)?;

    let copied: Vec<EntryCopy> = rows(
        &path,
        "SELECT id, ride_id, plate, display_name, type, team_size, status, \
         dnf_at, notes, logo_card FROM entry ORDER BY id",
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
            ))
        },
    )?;
    assert_eq!(copied, expected_v1_copies());
    Ok(())
}

/// Return the three v1 `entry` rows as the rebuild must copy them.
fn expected_v1_copies() -> Vec<EntryCopy> {
    vec![
        (
            1,
            1,
            "12".to_owned(),
            "Alice".to_owned(),
            "solo".to_owned(),
            1,
            "active".to_owned(),
            None,
            Some(String::new()),
            None,
        ),
        (
            2,
            1,
            "45".to_owned(),
            "Dirt Dynamos".to_owned(),
            "team".to_owned(),
            2,
            "active".to_owned(),
            None,
            Some("carry a tail light".to_owned()),
            None,
        ),
        (
            3,
            1,
            "9".to_owned(),
            "Bo".to_owned(),
            "solo".to_owned(),
            1,
            "dnf".to_owned(),
            Some(1_789_899_000),
            Some("cramp".to_owned()),
            Some("As".to_owned()),
        ),
    ]
}

#[test]
fn test_migrations_v1_to_v2_preserves_the_links_child_rows_hold() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v1.db");
    let seed: Vec<&str> = [V1_RIDE_ROW]
        .into_iter()
        .chain(V1_ENTRY_ROWS)
        .chain(V1_CHILD_ROWS)
        .chain([V1_CROSSING_AUDIT_ROW])
        .collect();
    write_v1_file(&path, &seed)?;

    ensure_current(&path)?;

    let violations: Vec<String> = rows(&path, "PRAGMA foreign_key_check", |row| row.get(0))?;

    assert_eq!(
        (
            violations,
            entry_links(&path, "rider")?,
            entry_links(&path, "crossing")?,
            entry_links(&path, "card")?,
        ),
        (
            Vec::<String>::new(),
            vec![(1, 1), (2, 2), (3, 2), (4, 3)],
            vec![(1, 1), (2, 1), (3, 2)],
            vec![(1, 1), (2, 1), (3, 2)],
        )
    );
    Ok(())
}

#[test]
fn test_migrations_v1_to_v2_leaves_only_the_live_plate_unique_index() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v1.db");
    write_v1_file(&path, &[V1_RIDE_ROW, V1_ENTRY_ROWS[0]])?;

    ensure_current(&path)?;

    let index_columns: Vec<String> =
        rows(&path, "PRAGMA index_info(entry_plate_live_unique)", |row| {
            row.get(2)
        })?;

    assert_eq!(
        (index_list_rows(&path, "entry")?, index_columns),
        (
            vec![(
                0,
                "entry_plate_live_unique".to_owned(),
                1,
                "c".to_owned(),
                1
            )],
            vec!["ride_id".to_owned(), "plate".to_owned()],
        )
    );
    Ok(())
}

#[test]
fn test_migrations_v1_to_v2_enforces_plate_uniqueness_among_live_rows() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v1.db");
    write_v1_file(&path, &[V1_RIDE_ROW, V1_ENTRY_ROWS[0]])?;
    ensure_current(&path)?;
    let conn = Connection::open(&path)?;

    let result = conn.execute(
        "INSERT INTO entry (ride_id, plate, key, display_name, type, team_size,
                            status, retired)
         VALUES (1, '12', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 'Ghost', 'solo', 1,
                 'active', 0)",
        [],
    );

    assert!(matches!(
        result,
        Err(rusqlite::Error::SqliteFailure(error, _))
            if error.code == ErrorCode::ConstraintViolation
    ));
    Ok(())
}

#[test]
fn test_migrations_v1_to_v2_allows_a_retired_row_to_hold_a_live_plate() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v1.db");
    write_v1_file(&path, &[V1_RIDE_ROW, V1_ENTRY_ROWS[0]])?;
    ensure_current(&path)?;
    Connection::open(&path)?.execute(
        "INSERT INTO entry (ride_id, plate, key, display_name, type, team_size,
                            status, retired)
         VALUES (1, '12', 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 'Alice', 'solo', 1,
                 'active', 1)",
        [],
    )?;

    let plates: Vec<(String, i64)> = rows(
        &path,
        "SELECT plate, retired FROM entry WHERE ride_id = 1 AND plate = '12' \
         ORDER BY id",
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    assert_eq!(plates, [("12".to_owned(), 0), ("12".to_owned(), 1)]);
    Ok(())
}

#[test]
fn test_migrations_v1_to_v2_leaves_entry_matching_a_fresh_file() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let migrated = directory.path().join("v1.db");
    write_v1_file(&migrated, &[V1_RIDE_ROW, V1_ENTRY_ROWS[0]])?;
    ensure_current(&migrated)?;
    let fresh = directory.path().join("fresh.db");
    ensure_current(&fresh)?;

    assert_eq!(
        (
            table_info_rows(&migrated, "entry")?,
            index_list_rows(&migrated, "entry")?,
        ),
        (
            table_info_rows(&fresh, "entry")?,
            index_list_rows(&fresh, "entry")?,
        )
    );
    Ok(())
}

#[test]
fn test_migrations_v1_to_v2_given_no_entry_rows_migrates_cleanly() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v1.db");
    write_v1_file(&path, &[V1_RIDE_ROW])?;

    ensure_current(&path)?;

    assert_eq!(
        (
            scalar::<i64>(&path, "SELECT COUNT(*) FROM entry")?,
            version(&path)?,
        ),
        (0, i64::from(SCHEMA_VERSION))
    );
    Ok(())
}

#[test]
fn test_migrations_v1_to_v2_given_one_entry_row_copies_it() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v1.db");
    write_v1_file(&path, &V1_SINGLE_ENTRY_SEED)?;

    ensure_current(&path)?;

    let copied: Vec<(i64, String, i64)> =
        rows(&path, "SELECT id, plate, retired FROM entry", |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
    assert_eq!(copied, [(1, "12".to_owned(), 0)]);
    Ok(())
}

#[test]
fn test_migrations_v1_to_v2_with_an_orphan_child_row_raises_and_rolls_back()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("orphan.db");
    write_v1_file(&path, &V1_ORPHAN_SEED)?;
    let mut conn = Connection::open(&path)?;
    apply_pragmas(&conn)?;

    let error = match ensure_schema(&mut conn) {
        Ok(()) => panic!("the orphan link must abort the migration"),
        Err(error) => error,
    };

    assert_eq!(
        (
            matches!(error, StoreError::ForeignKeys { ref tables } if tables == "rider"),
            version(&path)?,
            column_names(&path, "entry")?,
            scalar::<i64>(&path, "PRAGMA foreign_keys")?,
        ),
        (true, 1, V1_ENTRY_COLUMNS.map(str::to_owned).to_vec(), 1)
    );
    Ok(())
}

#[test]
fn test_migrations_v1_to_v2_names_each_violating_table_once_in_sorted_order()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("multi-orphan.db");
    write_v1_file(&path, &V1_MULTI_ORPHAN_SEED)?;
    let mut conn = Connection::open(&path)?;
    apply_pragmas(&conn)?;

    let error = match ensure_schema(&mut conn) {
        Ok(()) => panic!("the orphan links must abort the migration"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        StoreError::ForeignKeys { ref tables } if tables == "crossing, rider"
    ));
    Ok(())
}

#[test]
fn test_migrations_run_stamps_the_ledger_at_the_target_version() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v1.db");
    let seed = [
        V1_RIDE_ROW,
        V1_ENTRY_ROWS[0],
        V1_ENTRY_ROWS[1],
        V1_ENTRY_ROWS[2],
    ];
    write_v1_file(&path, &seed)?;

    let mut conn = Connection::open(&path)?;
    run_migrations(&mut conn, 1, SCHEMA_VERSION)?;
    drop(conn);

    assert_eq!(
        (
            version(&path)?,
            scalar::<i64>(&path, "SELECT COUNT(*) FROM entry WHERE key <> ''")?,
        ),
        (i64::from(SCHEMA_VERSION), 3)
    );
    Ok(())
}

#[test]
fn test_migrations_run_from_the_target_version_changes_nothing() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("current.db");
    ensure_current(&path)?;
    let before: Vec<(i64, String)> = rows(&path, "SELECT id, key FROM entry", |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?;

    let mut conn = Connection::open(&path)?;
    run_migrations(&mut conn, SCHEMA_VERSION, SCHEMA_VERSION)?;
    drop(conn);

    let after: Vec<(i64, String)> = rows(&path, "SELECT id, key FROM entry", |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?;
    assert_eq!(
        (version(&path)?, after),
        (i64::from(SCHEMA_VERSION), before)
    );
    Ok(())
}

#[test]
fn test_migrations_run_from_version_zero_returns_unknown_migration() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v0.db");
    let mut conn = Connection::open(&path)?;
    conn.execute_batch(SCHEMA_VERSION_DDL)?;

    let error = match run_migrations(&mut conn, 0, SCHEMA_VERSION) {
        Ok(()) => panic!("version 0 names no step"),
        Err(error) => error,
    };

    assert!(matches!(error, StoreError::UnknownMigration { version: 0 }));
    Ok(())
}

#[test]
fn test_migrations_cover_every_version_below_the_current_one() {
    let versions: Vec<u32> = MIGRATIONS.iter().map(|(version, _)| *version).collect();

    assert_eq!(versions, (1..SCHEMA_VERSION).collect::<Vec<u32>>());
}

// -------------------------------------------------------- v2 to v3 migration

#[test]
fn test_migrations_v2_to_v3_rewrites_a_legacy_audit_plate_to_its_key() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v2.db");
    write_v2_file(
        &path,
        &V2Seed {
            entries: &[V2_ALICE],
            audit: &[V2_START, V2_LEGACY_CROSSING],
            ..V2Seed::default()
        },
    )?;

    ensure_current(&path)?;

    let rewritten = payload(&path, 2)?;
    assert_eq!(
        (text_field(&rewritten, "entry_id"), version(&path)?),
        (Some(ALICE_KEY), i64::from(SCHEMA_VERSION))
    );
    Ok(())
}

#[test]
fn test_migrations_v2_to_v3_rewrites_only_a_plated_entry_identity() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v2.db");
    write_v2_file(
        &path,
        &V2Seed {
            entries: &[V2_ALICE],
            audit: &[
                V2_START,
                V2_LEGACY_CROSSING,
                V2_UNKNOWN_IDENTITY,
                V2_ALREADY_KEYED,
                V2_NON_STRING_IDENTITY,
            ],
            ..V2Seed::default()
        },
    )?;
    let untouched: Vec<(i64, String)> = rows(
        &path,
        "SELECT id, payload_json FROM audit WHERE id > 2 ORDER BY id",
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    ensure_current(&path)?;

    let rewritten = payload(&path, 2)?;
    let after: Vec<(i64, String)> = rows(
        &path,
        "SELECT id, payload_json FROM audit WHERE id > 2 ORDER BY id",
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(
        (
            text_field(&rewritten, "entry_id"),
            text_field(&rewritten, "plate"),
            text_field(&rewritten, "reason"),
            after,
        ),
        (Some(ALICE_KEY), Some("12"), Some("Alice"), untouched)
    );
    Ok(())
}

#[test]
fn test_migrations_v2_to_v3_rewrites_both_identities_in_a_reassign_payload()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v2.db");
    write_v2_file(
        &path,
        &V2Seed {
            entries: &[V2_ALICE, V2_DYNAMOS],
            audit: &[V2_START, V2_REASSIGN],
            ..V2Seed::default()
        },
    )?;

    ensure_current(&path)?;

    let rewritten = payload(&path, 2)?;
    assert_eq!(
        (
            text_field(&rewritten, "old_entry_id"),
            text_field(&rewritten, "new_entry_id"),
            text_field(&rewritten, "new_plate"),
        ),
        (Some(ALICE_KEY), Some(DYNAMOS_KEY), Some("45"))
    );
    Ok(())
}

#[test]
fn test_migrations_v2_to_v3_resolves_each_rides_plates_separately() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v2.db");
    write_v2_file(
        &path,
        &V2Seed {
            entries: &[V2_ALICE, V2_RIDE2_ALICE],
            audit: &[V2_LEGACY_CROSSING, V2_SECOND_RIDE_CROSSING],
            extra_rides: &[V2_SECOND_RIDE_ROW],
        },
    )?;

    ensure_current(&path)?;

    assert_eq!(
        (
            text_field(&payload(&path, 2)?, "entry_id"),
            text_field(&payload(&path, 6)?, "entry_id"),
        ),
        (Some(ALICE_KEY), Some(RIDE2_KEY))
    );
    Ok(())
}

#[test]
fn test_migrations_v2_to_v3_skips_a_malformed_payload_without_aborting()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v2.db");
    write_v2_file(
        &path,
        &V2Seed {
            entries: &[V2_ALICE],
            audit: &[V2_START, V2_MALFORMED, V2_LEGACY_CROSSING, V2_NON_OBJECT],
            ..V2Seed::default()
        },
    )?;

    ensure_current(&path)?;

    let skipped: Vec<(i64, String)> = rows(
        &path,
        "SELECT id, payload_json FROM audit WHERE id > 3 ORDER BY id",
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(
        (skipped, text_field(&payload(&path, 2)?, "entry_id")),
        (
            vec![(4, "not json at all".to_owned()), (5, "[]".to_owned()),],
            Some(ALICE_KEY),
        )
    );
    Ok(())
}

#[test]
fn test_migrations_v2_to_v3_skips_a_non_text_payload() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v2.db");
    write_v2_file(
        &path,
        &V2Seed {
            entries: &[V2_ALICE],
            audit: &[V2_LEGACY_CROSSING],
            ..V2Seed::default()
        },
    )?;
    let blob = Value::Blob(vec![0xde, 0xad, 0xbe, 0xef]);
    Connection::open(&path)?.execute(
        "INSERT INTO audit (id, ride_id, at, action, payload_json)
         VALUES (7, 1, 1789898800, 'record_crossing', ?1)",
        [blob.clone()],
    )?;

    ensure_current(&path)?;

    assert_eq!(
        (
            scalar::<Value>(&path, "SELECT payload_json FROM audit WHERE id = 7")?,
            version(&path)?,
        ),
        (blob, i64::from(SCHEMA_VERSION))
    );
    Ok(())
}

#[test]
fn test_migrations_v2_to_v3_leaves_a_retired_entrys_plate_unresolved() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v2.db");
    write_v2_file(
        &path,
        &V2Seed {
            entries: &[V2_RETIRED_ALICE, V2_DYNAMOS],
            audit: &[V2_REASSIGN],
            ..V2Seed::default()
        },
    )?;

    ensure_current(&path)?;

    let rewritten = payload(&path, 2)?;
    assert_eq!(
        (
            text_field(&rewritten, "old_entry_id"),
            text_field(&rewritten, "new_entry_id"),
        ),
        (Some("12"), Some(DYNAMOS_KEY))
    );
    Ok(())
}

#[test]
fn test_migrations_v2_to_v3_leaves_already_keyed_values_untouched() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v2.db");
    write_v2_file(
        &path,
        &V2Seed {
            entries: &[V2_ALICE],
            audit: &[V2_LEGACY_CROSSING, V2_ALREADY_KEYED],
            ..V2Seed::default()
        },
    )?;
    let keyed_before: String = scalar(&path, "SELECT payload_json FROM audit WHERE id = 4")?;

    ensure_current(&path)?;

    let keyed_after: String = scalar(&path, "SELECT payload_json FROM audit WHERE id = 4")?;
    assert_eq!(
        (keyed_after, text_field(&payload(&path, 2)?, "entry_id")),
        (keyed_before, Some(ALICE_KEY))
    );
    Ok(())
}

#[test]
fn test_migrations_v2_to_v3_leaves_a_non_string_identity_untouched() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v2.db");
    write_v2_file(
        &path,
        &V2Seed {
            entries: &[V2_ALICE],
            audit: &[V2_NON_STRING_IDENTITY],
            ..V2Seed::default()
        },
    )?;

    ensure_current(&path)?;

    assert_eq!(
        payload(&path, 5)?,
        serde_json::json!({"entry_id": 93, "plate": "93", "reason": "hand-edited"})
    );
    Ok(())
}

#[test]
fn test_migrations_v2_to_v3_leaves_the_entry_rows_untouched() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v2.db");
    write_v2_file(
        &path,
        &V2Seed {
            entries: &[V2_ALICE, V2_DYNAMOS],
            audit: &[V2_START, V2_LEGACY_CROSSING],
            ..V2Seed::default()
        },
    )?;

    ensure_current(&path)?;

    let entries: Vec<(i64, String, String, i64)> = rows(
        &path,
        "SELECT id, plate, key, retired FROM entry ORDER BY id",
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    assert_eq!(
        (entries, version(&path)?),
        (
            vec![
                (1, "12".to_owned(), ALICE_KEY.to_owned(), 0),
                (2, "45".to_owned(), DYNAMOS_KEY.to_owned(), 0),
            ],
            i64::from(SCHEMA_VERSION),
        )
    );
    Ok(())
}

#[test]
fn test_migrations_v2_to_v3_given_a_second_open_leaves_the_audit_untouched()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v2.db");
    write_v2_file(
        &path,
        &V2Seed {
            entries: &[V2_ALICE],
            audit: &[V2_START, V2_LEGACY_CROSSING],
            ..V2Seed::default()
        },
    )?;
    ensure_current(&path)?;
    let rewritten = audit_rows(&path)?;

    ensure_current(&path)?;

    assert_eq!(
        (
            audit_rows(&path)?,
            text_field(&payload(&path, 2)?, "entry_id")
        ),
        (rewritten, Some(ALICE_KEY))
    );
    Ok(())
}

#[test]
fn test_migrations_v2_to_v3_given_a_failed_rewrite_rolls_the_file_back()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v2.db");
    write_v2_file(
        &path,
        &V2Seed {
            entries: &[V2_ALICE],
            audit: &[V2_LEGACY_CROSSING],
            ..V2Seed::default()
        },
    )?;
    let before = audit_rows(&path)?;
    Connection::open(&path)?.execute_batch(
        "CREATE TRIGGER audit_payload_frozen BEFORE UPDATE ON audit
         BEGIN SELECT RAISE(ABORT, 'audit payload frozen'); END",
    )?;

    let error = match ensure_current(&path) {
        Ok(()) => panic!("the frozen trigger must abort the rewrite"),
        Err(error) => error,
    };

    assert_eq!(
        (
            error.to_string().contains("audit payload frozen"),
            version(&path)?,
            audit_rows(&path)?,
        ),
        (true, 2, before)
    );
    Ok(())
}

#[test]
fn test_migrations_v1_to_v3_chain_rewrites_the_legacy_entry_id() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("v1.db");
    let seed: Vec<&str> = [V1_RIDE_ROW]
        .into_iter()
        .chain(V1_ENTRY_ROWS)
        .chain(V1_CHILD_ROWS)
        .chain([V1_CROSSING_AUDIT_ROW])
        .collect();
    write_v1_file(&path, &seed)?;

    ensure_current(&path)?;

    let alice_key: String = scalar(
        &path,
        "SELECT key FROM entry WHERE ride_id = 1 AND plate = '12'",
    )?;
    let rewritten = payload(&path, 3)?;
    assert_eq!(
        (text_field(&rewritten, "entry_id"), version(&path)?),
        (Some(alice_key.as_str()), i64::from(SCHEMA_VERSION))
    );
    Ok(())
}
