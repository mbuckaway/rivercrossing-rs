// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the schema: the fresh-file shape, the PRAGMAs, the version
//! gate, every oracle CHECK, the column defaults and the transient-error
//! matcher. Each test drives a real SQLite file in a `TempDir`.

use std::path::Path;

use pretty_assertions::assert_eq;
use rstest::rstest;
use rusqlite::{Connection, ErrorCode};
use tempfile::TempDir;

use super::{
    SCHEMA_VERSION, SCHEMA_VERSION_DDL, apply_pragmas, ensure_schema, is_transient_open_error,
};
use crate::error::StoreError;

/// A minimal valid ride row, so a test can seed one without a `Store`.
const VALID_RIDE_INSERT: &str = "
    INSERT INTO ride (
        id, name, event_date, venue, course_name, lap_km, organizer, scorer,
        planned_start, planned_duration_s, status, entry_mode, max_team_size,
        plate_model, min_lap_s, deck_count, jokers_per_deck, tiebreak_order,
        rng_seed, created_at, updated_at
    ) VALUES (
        1, 'GORBA EPIC 2026', '2026-09-20', 'Sea to Sky Gondola', 'Gondola Loop',
        8.0, 'GORBA', 'K. Singh', 1789898400, 21600, 'draft', 'mixed', 4,
        'rider_pooled', 1080, 8, 2, '[\"laps\"]', 7, 1789898400, 1789898400
    )
";

/// Open the schema file in *directory* and apply the PRAGMAs.
fn open_file(directory: &TempDir) -> Connection {
    Connection::open(directory.path().join("rides.db")).expect("the test database opens")
}

/// Create a current schema in a fresh file in a `TempDir`.
///
/// The connection is dropped before the directory, so SQLite closes the file
/// and its WAL first.
fn fresh_db() -> (Connection, TempDir) {
    let directory = tempfile::tempdir().expect("the test directory is created");
    let mut conn = open_file(&directory);
    apply_pragmas(&conn).expect("the PRAGMAs apply");
    ensure_schema(&mut conn).expect("the schema is created");
    (conn, directory)
}

/// Return the sorted names of every user table in the database.
fn table_names(conn: &Connection) -> Vec<String> {
    let mut statement = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' \
             AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .expect("the catalog query prepares");
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("the catalog query runs");
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .expect("the catalog rows decode")
}

/// Return one table's `(name, declared type, default text)` rows, in order.
fn table_info(conn: &Connection, table: &str) -> Vec<(String, String, Option<String>)> {
    let sql = format!("PRAGMA table_info({table})");
    let mut statement = conn.prepare(&sql).expect("the table_info query prepares");
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })
        .expect("the table_info query runs");
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .expect("the table_info rows decode")
}

/// Return one table's column names, in order.
fn column_names(conn: &Connection, table: &str) -> Vec<String> {
    table_info(conn, table)
        .into_iter()
        .map(|(name, _, _)| name)
        .collect()
}

/// Return one scalar integer from the database.
fn scalar_i64(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |row| row.get(0))
        .expect("the scalar query returns one row")
}

/// Assert that *statement* fails its CHECK or foreign key constraint.
fn assert_constraint_rejected(conn: &Connection, statement: &str) {
    let result = conn.execute(statement, []);
    assert!(
        matches!(
            result,
            Err(rusqlite::Error::SqliteFailure(error, _))
                if error.code == ErrorCode::ConstraintViolation
        ),
        "expected a constraint violation from `{statement}`, got {result:?}"
    );
}

/// Seed one valid ride, entry, rider, crossing and card row.
fn seed_rows(conn: &Connection) {
    conn.execute_batch(VALID_RIDE_INSERT)
        .expect("the ride row seeds");
    conn.execute_batch(
        "INSERT INTO entry (id, ride_id, plate, key, display_name, type, team_size,
                            status)
         VALUES (1, 1, '12', 'key-a', 'Alice', 'solo', 1, 'active');
         INSERT INTO rider (id, entry_id, first_name, last_name, sort_order, sex)
         VALUES (1, 1, 'Alice', 'Wong', 0, 'F');
         INSERT INTO crossing (id, ride_id, entry_id, seq, crossed_at, lap_s, flag)
         VALUES (1, 1, 1, 1, 1789898520, 120, 'none');
         INSERT INTO card (id, ride_id, entry_id, rank, suit, state, dealt_at)
         VALUES (1, 1, 1, 14, 's', 'dealt', 1789898520);",
    )
    .expect("the child rows seed");
}

// ------------------------------------------------------- fresh-file shape

#[test]
fn test_schema_a_fresh_file_creates_exactly_the_eight_tables() {
    let (conn, _directory) = fresh_db();

    assert_eq!(
        table_names(&conn),
        [
            "app_session",
            "audit",
            "card",
            "crossing",
            "entry",
            "ride",
            "rider",
            "schema_version",
        ]
    );
}

#[test]
fn test_schema_a_fresh_file_passes_the_integrity_check() {
    let (conn, _directory) = fresh_db();

    let verdict: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .expect("the integrity check runs");

    assert_eq!(verdict, "ok");
}

#[test]
fn test_schema_the_schema_version_constant_is_three() {
    assert_eq!(SCHEMA_VERSION, 3);
}

#[test]
fn test_schema_a_fresh_file_stamps_one_ledger_row_at_the_current_version() {
    let (conn, _directory) = fresh_db();

    assert_eq!(
        (
            scalar_i64(&conn, "SELECT version FROM schema_version WHERE id = 1"),
            scalar_i64(&conn, "SELECT COUNT(*) FROM schema_version"),
        ),
        (i64::from(SCHEMA_VERSION), 1)
    );
}

#[test]
fn test_schema_a_current_file_reopens_as_a_noop_keeping_its_rows() {
    let (mut conn, _directory) = fresh_db();
    conn.execute_batch(VALID_RIDE_INSERT)
        .expect("the ride row seeds");

    ensure_schema(&mut conn).expect("the second ensure is a no-op");

    assert_eq!(
        (
            scalar_i64(&conn, "SELECT COUNT(*) FROM ride"),
            scalar_i64(&conn, "SELECT version FROM schema_version"),
            scalar_i64(&conn, "SELECT COUNT(*) FROM schema_version"),
        ),
        (1, i64::from(SCHEMA_VERSION), 1)
    );
}

#[test]
fn test_schema_an_empty_file_becomes_a_full_current_database() {
    let directory = tempfile::tempdir().expect("the test directory is created");
    let path = directory.path().join("v0.db");
    Connection::open(&path)
        .expect("the empty file is created")
        .close()
        .expect("the empty file closes");

    let mut conn = Connection::open(&path).expect("the empty file reopens");
    apply_pragmas(&conn).expect("the PRAGMAs apply");
    ensure_schema(&mut conn).expect("the schema is created");

    assert_eq!(
        (
            table_names(&conn),
            scalar_i64(&conn, "SELECT version FROM schema_version"),
        ),
        (
            [
                "app_session",
                "audit",
                "card",
                "crossing",
                "entry",
                "ride",
                "rider",
                "schema_version",
            ]
            .map(str::to_owned)
            .to_vec(),
            i64::from(SCHEMA_VERSION),
        )
    );
}

#[test]
fn test_schema_opens_a_file_in_nested_directories_that_the_caller_created() {
    let directory = tempfile::tempdir().expect("the test directory is created");
    let nested = directory.path().join("app").join("data").join("nested");
    std::fs::create_dir_all(&nested).expect("the nested directories are created");
    let path = nested.join("rides.db");

    let mut conn = Connection::open(&path).expect("the nested database opens");
    apply_pragmas(&conn).expect("the PRAGMAs apply");
    ensure_schema(&mut conn).expect("the schema is created");

    assert!(Path::new(&path).is_file());
}

#[test]
fn test_schema_the_ride_table_carries_the_v2_columns() {
    let (conn, _directory) = fresh_db();

    assert_eq!(
        column_names(&conn, "ride"),
        [
            "id",
            "name",
            "event_date",
            "venue",
            "course_name",
            "lap_km",
            "organizer",
            "scorer",
            "logo_png",
            "planned_start",
            "planned_duration_s",
            "actual_start",
            "finished_at",
            "status",
            "entry_mode",
            "max_team_size",
            "plate_model",
            "min_lap_s",
            "deck_count",
            "jokers_per_deck",
            "jokers_mode",
            "max_cards",
            "tiebreak_order",
            "rng_seed",
            "created_at",
            "updated_at",
            "hold_short_laps",
        ]
        .map(str::to_owned)
        .to_vec()
    );
}

#[test]
fn test_schema_the_entry_table_carries_the_key_and_retired_columns() {
    let (conn, _directory) = fresh_db();

    assert_eq!(
        column_names(&conn, "entry"),
        [
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
        ]
        .map(str::to_owned)
        .to_vec()
    );
}

#[test]
fn test_schema_the_rider_table_carries_the_nullable_sex_column() {
    let (conn, _directory) = fresh_db();

    assert_eq!(
        column_names(&conn, "rider"),
        [
            "id",
            "entry_id",
            "first_name",
            "last_name",
            "plate",
            "sort_order",
            "sex",
            "emergency_contact",
            "waiver_signed",
            "ccn_reg_id",
        ]
        .map(str::to_owned)
        .to_vec()
    );
}

#[test]
fn test_schema_the_ride_columns_default_hold_short_laps_and_jokers_mode() {
    let (conn, _directory) = fresh_db();

    let defaults = table_info(&conn, "ride")
        .into_iter()
        .filter(|(name, _, _)| name == "hold_short_laps" || name == "jokers_mode")
        .collect::<Vec<_>>();

    assert_eq!(
        defaults,
        [
            (
                "jokers_mode".to_owned(),
                "TEXT".to_owned(),
                Some("'total'".to_owned())
            ),
            (
                "hold_short_laps".to_owned(),
                "INTEGER".to_owned(),
                Some("1".to_owned())
            ),
        ]
    );
}

// ---------------------------------------------------------------- PRAGMAs

#[test]
fn test_schema_apply_pragmas_sets_synchronous_to_normal() {
    let directory = tempfile::tempdir().expect("the test directory is created");
    let conn = open_file(&directory);

    apply_pragmas(&conn).expect("the PRAGMAs apply");

    assert_eq!(scalar_i64(&conn, "PRAGMA synchronous"), 1);
}

#[test]
fn test_schema_apply_pragmas_turns_foreign_keys_on() {
    let directory = tempfile::tempdir().expect("the test directory is created");
    let conn = open_file(&directory);

    apply_pragmas(&conn).expect("the PRAGMAs apply");

    assert_eq!(scalar_i64(&conn, "PRAGMA foreign_keys"), 1);
}

#[test]
fn test_schema_apply_pragmas_rejects_a_row_that_names_no_ride() {
    let (conn, _directory) = fresh_db();

    assert_constraint_rejected(
        &conn,
        "INSERT INTO entry (ride_id, plate, key, display_name, type, team_size,
                            status)
         VALUES (999, 'P1', 'key-b', 'ghost', 'solo', 1, 'active')",
    );
}

#[test]
fn test_schema_apply_pragmas_leaves_wal_on_the_file_for_a_later_connection() {
    let (conn, directory) = fresh_db();
    drop(conn);

    let later = Connection::open(directory.path().join("rides.db")).expect("the database reopens");

    assert_eq!(scalar_text(&later, "PRAGMA journal_mode"), "wal");
}

/// Return one scalar text from the database.
fn scalar_text(conn: &Connection, sql: &str) -> String {
    conn.query_row(sql, [], |row| row.get(0))
        .expect("the scalar query returns one row")
}

// ----------------------------------------------------------- version gate

#[rstest]
#[case(4)]
#[case(99)]
#[case(999)]
fn test_schema_a_newer_version_is_refused_with_the_exact_message(#[case] stored: u32) {
    let directory = tempfile::tempdir().expect("the test directory is created");
    let mut conn = open_file(&directory);
    conn.execute_batch(SCHEMA_VERSION_DDL)
        .expect("the ledger bootstraps");
    conn.execute(
        "INSERT INTO schema_version (id, version) VALUES (1, ?1)",
        [stored],
    )
    .expect("the ledger row seeds");
    let expected = format!(
        "Database schema version {stored} does not match this build's schema \
         version {SCHEMA_VERSION}. Rename or delete the database file to continue."
    );

    let error = match ensure_schema(&mut conn) {
        Ok(()) => panic!("a newer version must be refused"),
        Err(error) => error,
    };

    assert_eq!(error.to_string(), expected);
    assert!(matches!(
        error,
        StoreError::SchemaVersionMismatch { current, supported }
            if current == stored && supported == SCHEMA_VERSION
    ));
}

#[test]
fn test_schema_a_mid_create_collision_leaves_no_ride_and_an_empty_ledger() {
    let directory = tempfile::tempdir().expect("the test directory is created");
    let path = directory.path().join("conflict.db");
    Connection::open(&path)
        .expect("the conflicting database opens")
        .execute_batch("CREATE TABLE audit (x INTEGER)")
        .expect("the conflicting table is created");
    let mut conn = Connection::open(&path).expect("the conflicting database reopens");
    apply_pragmas(&conn).expect("the PRAGMAs apply");

    let error = match ensure_schema(&mut conn) {
        Ok(()) => panic!("the collision must abort the create"),
        Err(error) => error,
    };

    assert_eq!(
        (
            matches!(error, StoreError::Sqlite(_)),
            error.to_string().contains("already exists"),
        ),
        (true, true)
    );
    assert_eq!(
        (
            table_names(&conn).contains(&"ride".to_owned()),
            table_names(&conn).contains(&"audit".to_owned()),
            table_names(&conn).contains(&"schema_version".to_owned()),
            scalar_i64(&conn, "SELECT COUNT(*) FROM schema_version"),
        ),
        (false, true, true, 0)
    );
}

#[test]
fn test_schema_a_negative_ledger_version_is_reported_as_an_invalid_row() {
    let directory = tempfile::tempdir().expect("the test directory is created");
    let mut conn = open_file(&directory);
    conn.execute_batch(SCHEMA_VERSION_DDL)
        .expect("the ledger bootstraps");
    conn.execute_batch("INSERT INTO schema_version (id, version) VALUES (1, -1)")
        .expect("the corrupt ledger row seeds");

    let error = match ensure_schema(&mut conn) {
        Ok(()) => panic!("a negative version must be refused"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        StoreError::InvalidRow {
            table: "schema_version",
            column: "version",
            ref stored,
        } if stored == "-1"
    ));
}

// ----------------------------------------------------------- oracle CHECKs

#[test]
fn test_schema_check_rejects_an_unknown_ride_status() {
    let (conn, _directory) = fresh_db();
    seed_rows(&conn);

    assert_constraint_rejected(&conn, "UPDATE ride SET status = 'paused'");
}

#[test]
fn test_schema_check_rejects_an_unknown_entry_mode() {
    let (conn, _directory) = fresh_db();
    seed_rows(&conn);

    assert_constraint_rejected(&conn, "UPDATE ride SET entry_mode = 'team'");
}

#[test]
fn test_schema_check_rejects_an_unknown_plate_model() {
    let (conn, _directory) = fresh_db();
    seed_rows(&conn);

    assert_constraint_rejected(&conn, "UPDATE ride SET plate_model = 'pooled'");
}

#[test]
fn test_schema_check_rejects_an_unknown_jokers_mode() {
    let (conn, _directory) = fresh_db();
    seed_rows(&conn);

    assert_constraint_rejected(&conn, "UPDATE ride SET jokers_mode = 'none'");
}

#[test]
fn test_schema_check_rejects_an_unknown_entry_type() {
    let (conn, _directory) = fresh_db();
    seed_rows(&conn);

    assert_constraint_rejected(&conn, "UPDATE entry SET type = 'relay'");
}

#[test]
fn test_schema_check_rejects_an_unknown_entry_status() {
    let (conn, _directory) = fresh_db();
    seed_rows(&conn);

    assert_constraint_rejected(&conn, "UPDATE entry SET status = 'retired'");
}

#[test]
fn test_schema_check_rejects_an_unknown_crossing_flag() {
    let (conn, _directory) = fresh_db();
    seed_rows(&conn);

    assert_constraint_rejected(&conn, "UPDATE crossing SET flag = 'auto'");
}

#[test]
fn test_schema_check_rejects_a_card_rank_between_one_and_the_ace() {
    let (conn, _directory) = fresh_db();
    seed_rows(&conn);

    assert_constraint_rejected(&conn, "UPDATE card SET rank = 1");
}

#[test]
fn test_schema_check_rejects_a_card_rank_above_the_ace() {
    let (conn, _directory) = fresh_db();
    seed_rows(&conn);

    assert_constraint_rejected(&conn, "UPDATE card SET rank = 15");
}

#[test]
fn test_schema_check_rejects_an_unknown_card_suit() {
    let (conn, _directory) = fresh_db();
    seed_rows(&conn);

    assert_constraint_rejected(&conn, "UPDATE card SET suit = 'x'");
}

#[test]
fn test_schema_check_rejects_an_unknown_card_state() {
    let (conn, _directory) = fresh_db();
    seed_rows(&conn);

    assert_constraint_rejected(&conn, "UPDATE card SET state = 'burned'");
}

#[test]
fn test_schema_check_rejects_an_unknown_rider_sex() {
    let (conn, _directory) = fresh_db();
    seed_rows(&conn);

    assert_constraint_rejected(&conn, "UPDATE rider SET sex = 'X'");
}

// ------------------------------------------------------ transient matcher

#[rstest]
#[case("disk I/O error")]
#[case("database is locked")]
#[case("database table is locked")]
#[case("database or disk is full")]
#[case("out of memory")]
fn test_schema_transient_open_error_is_true_for_the_five_oracle_strings(#[case] text: &str) {
    assert!(is_transient_open_error(text));
}

#[rstest]
#[case("unable to open database file")]
#[case("table ride already exists")]
#[case("no such table: ride")]
#[case("")]
fn test_schema_transient_open_error_is_false_for_other_text(#[case] text: &str) {
    assert!(!is_transient_open_error(text));
}

#[test]
fn test_schema_transient_open_error_matches_inside_a_longer_message() {
    assert!(is_transient_open_error(
        "unable to open database file: disk I/O error while reading the WAL"
    ));
}
