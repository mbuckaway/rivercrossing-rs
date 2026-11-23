// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the `Store`: open, the ride library, the config writes,
//! duplication, the delete guard and the session bookkeeping.
//!
//! Every test drives a real SQLite file in a `TempDir`. The store's own
//! connection is used for the raw seeds that the oracle tests also write
//! directly, such as a RUNNING status or a heartbeat.

use std::path::{Path, PathBuf};

use pretty_assertions::assert_eq;
use rivercrossing_cards::{Card, JokersMode};
use rivercrossing_ride::{
    Clock, EntryId, EntryMode, Event, EventAction, PlateModel, RideConfig, RideConfigParams,
    RideStatus, SystemClock, TieBreak, Timestamp,
};
use rivercrossing_roster::enums::{EntryStatus, EntryType};
use rivercrossing_roster::roster::Roster;
use rivercrossing_roster::types::{AuditEvent, Entry, Rider};
use rstest::rstest;
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use tempfile::TempDir;
use time::{Date, Duration, Month, OffsetDateTime, Time};

use super::{Store, random_63_bit};
use crate::backup::backup_dir_for;
use crate::error::StoreError;
use crate::session::{PreviousSession, SessionState};

// ------------------------------------------------------------- fixtures

/// A stored ride row, every column, for the write tests.
#[derive(Debug)]
struct StoredRide {
    name: String,
    event_date: String,
    venue: String,
    course_name: String,
    lap_km: f64,
    organizer: String,
    scorer: String,
    logo_png: Option<Vec<u8>>,
    planned_start: i64,
    planned_duration_s: i64,
    actual_start: Option<i64>,
    finished_at: Option<i64>,
    status: String,
    entry_mode: String,
    max_team_size: i64,
    plate_model: String,
    min_lap_s: i64,
    deck_count: i64,
    jokers_per_deck: i64,
    jokers_mode: String,
    max_cards: Option<i64>,
    tiebreak_order: String,
    rng_seed: i64,
    created_at: i64,
    updated_at: i64,
    hold_short_laps: i64,
}

/// The columns of one seeded entry row.
struct EntrySeed<'a> {
    /// The ride the entry belongs to.
    ride_id: i64,
    /// The plate.
    plate: &'a str,
    /// The entry's stable key.
    key: &'a str,
    /// The display name.
    display_name: &'a str,
}

/// The columns of one seeded rider row.
struct RiderSeed<'a> {
    /// The entry the rider belongs to.
    entry_id: i64,
    /// The first name.
    first_name: &'a str,
    /// The last name.
    last_name: &'a str,
    /// The rider's plate.
    plate: &'a str,
}

/// The ids that one seeded card row references.
struct CrossingIds {
    /// The ride.
    ride: i64,
    /// The entry.
    entry: i64,
    /// The crossing.
    crossing: i64,
}

/// Return the fixture event date, 20 September 2026.
fn event_date() -> Date {
    Date::from_calendar_date(2026, Month::September, 20).expect("the fixture date is valid")
}

/// Return the fixture planned start, 20 September 2026 at 10:00 UTC.
fn planned_start() -> Timestamp {
    let time = Time::from_hms(10, 0, 0).expect("the fixture time is valid");
    OffsetDateTime::new_utc(event_date(), time)
}

/// Build a valid ride config with the oracle fixture values.
fn config() -> RideConfig {
    RideConfig::new(RideConfigParams {
        name: "GORBA EPIC 2026".to_owned(),
        event_date: event_date(),
        venue: "Sea to Sky Gondola".to_owned(),
        lap_km: 8.0,
        organizer: "GORBA".to_owned(),
        scorer: "K. Singh".to_owned(),
        planned_start: planned_start(),
        planned_duration_s: 21_600,
        min_lap_s: 1_080,
        entry_mode: EntryMode::Mixed,
        plate_model: PlateModel::RiderPooled,
        ..RideConfigParams::default()
    })
    .expect("the fixture config is valid")
}

/// Build the fixture config under another name.
fn config_named(name: &str) -> RideConfig {
    let mut config = config();
    config.name = name.to_owned();
    config
}

/// Build the fully changed config that the update test submits.
fn edited_config() -> RideConfig {
    let mut edited = config_named("After");
    edited.event_date =
        Date::from_calendar_date(2026, Month::October, 4).expect("the edited date is valid");
    edited.venue = "New Venue".to_owned();
    edited.lap_km = 6.5;
    edited.organizer = "Org 2".to_owned();
    edited.scorer = "Scorer 2".to_owned();
    edited.planned_start = planned_start() + Duration::hours(2);
    edited.planned_duration_s = 7_200;
    edited.min_lap_s = 90;
    edited.max_team_size = 6;
    edited.deck_count = 2;
    edited.jokers_per_deck = 0;
    edited.jokers_mode = JokersMode::PerDeck;
    edited.entry_mode = EntryMode::Solo;
    edited.plate_model = PlateModel::TeamRelay;
    edited.max_cards = Some(5);
    edited.tiebreak_order = [TieBreak::HighCard, TieBreak::Laps, TieBreak::TotalTime];
    edited.hold_short_laps = false;
    edited
}

/// Open a store over a fresh database in a new `TempDir`.
///
/// The directory is returned first so the test keeps it alive: dropping it
/// first would remove the database under the open connection.
fn temp_store() -> (TempDir, Store) {
    let directory = tempfile::tempdir().expect("the test directory is created");
    let store = Store::open(directory.path().join("rides.db")).expect("the store opens");
    (directory, store)
}

/// Read one ride row back, every column.
fn stored_ride(store: &Store, ride_id: i64) -> StoredRide {
    store
        .conn
        .query_row(
            "SELECT name, event_date, venue, course_name, lap_km, organizer, scorer, \
             logo_png, planned_start, planned_duration_s, actual_start, finished_at, \
             status, entry_mode, max_team_size, plate_model, min_lap_s, deck_count, \
             jokers_per_deck, jokers_mode, max_cards, tiebreak_order, rng_seed, \
             created_at, updated_at, hold_short_laps FROM ride WHERE id = ?1",
            [ride_id],
            |row| {
                Ok(StoredRide {
                    name: row.get(0)?,
                    event_date: row.get(1)?,
                    venue: row.get(2)?,
                    course_name: row.get(3)?,
                    lap_km: row.get(4)?,
                    organizer: row.get(5)?,
                    scorer: row.get(6)?,
                    logo_png: row.get(7)?,
                    planned_start: row.get(8)?,
                    planned_duration_s: row.get(9)?,
                    actual_start: row.get(10)?,
                    finished_at: row.get(11)?,
                    status: row.get(12)?,
                    entry_mode: row.get(13)?,
                    max_team_size: row.get(14)?,
                    plate_model: row.get(15)?,
                    min_lap_s: row.get(16)?,
                    deck_count: row.get(17)?,
                    jokers_per_deck: row.get(18)?,
                    jokers_mode: row.get(19)?,
                    max_cards: row.get(20)?,
                    tiebreak_order: row.get(21)?,
                    rng_seed: row.get(22)?,
                    created_at: row.get(23)?,
                    updated_at: row.get(24)?,
                    hold_short_laps: row.get(25)?,
                })
            },
        )
        .expect("the ride row reads")
}

/// Return the number of rows in *table*.
fn table_count(store: &Store, table: &str) -> i64 {
    store
        .conn
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("the count query runs")
}

/// Insert one entry row and return its id.
fn insert_entry(store: &Store, seed: &EntrySeed<'_>) -> i64 {
    store
        .conn
        .execute(
            "INSERT INTO entry (ride_id, plate, key, display_name, type, team_size, status) \
             VALUES (?1, ?2, ?3, ?4, 'solo', 1, 'active')",
            params![seed.ride_id, seed.plate, seed.key, seed.display_name],
        )
        .expect("the entry inserts");
    store.conn.last_insert_rowid()
}

/// Insert one rider row.
fn insert_rider(store: &Store, seed: &RiderSeed<'_>) {
    store
        .conn
        .execute(
            "INSERT INTO rider (entry_id, first_name, last_name, plate, sort_order) \
             VALUES (?1, ?2, ?3, ?4, 1)",
            params![seed.entry_id, seed.first_name, seed.last_name, seed.plate],
        )
        .expect("the rider inserts");
}

/// Insert one crossing row and return its id.
fn insert_crossing(store: &Store, ride_id: i64, entry_id: i64) -> i64 {
    store
        .conn
        .execute(
            "INSERT INTO crossing (ride_id, entry_id, seq, crossed_at, lap_s, flag) \
             VALUES (?1, ?2, 1, 100, 50, 'none')",
            params![ride_id, entry_id],
        )
        .expect("the crossing inserts");
    store.conn.last_insert_rowid()
}

/// Insert one card row for the seeded crossing.
fn insert_card(store: &Store, ids: &CrossingIds) {
    store
        .conn
        .execute(
            "INSERT INTO card \
             (ride_id, entry_id, crossing_id, shoe_index, rank, suit, state, dealt_at) \
             VALUES (?1, ?2, ?3, 0, 14, 's', 'dealt', 100)",
            params![ids.ride, ids.entry, ids.crossing],
        )
        .expect("the card inserts");
}

/// Insert one audit row for a recorded crossing.
fn insert_audit(store: &Store, ride_id: i64) {
    store
        .conn
        .execute(
            "INSERT INTO audit (ride_id, at, action, payload_json) \
             VALUES (?1, 100, 'record_crossing', '{}')",
            [ride_id],
        )
        .expect("the audit row inserts");
}

/// Seed one live entry with its rider, crossing, card and audit row.
fn seed_timing_data(store: &Store, ride_id: i64) -> i64 {
    let entry_id = insert_entry(
        store,
        &EntrySeed {
            ride_id,
            plate: "12",
            key: "key-12",
            display_name: "Alice",
        },
    );
    insert_rider(
        store,
        &RiderSeed {
            entry_id,
            first_name: "Alice",
            last_name: "",
            plate: "12",
        },
    );
    let crossing_id = insert_crossing(store, ride_id, entry_id);
    insert_card(
        store,
        &CrossingIds {
            ride: ride_id,
            entry: entry_id,
            crossing: crossing_id,
        },
    );
    insert_audit(store, ride_id);
    entry_id
}

/// Seed a solo entry with one rider and a team entry with two riders.
fn seed_two_entries(store: &Store, ride_id: i64) {
    let solo = insert_entry(
        store,
        &EntrySeed {
            ride_id,
            plate: "12",
            key: "source-key-12",
            display_name: "Alice",
        },
    );
    insert_rider(
        store,
        &RiderSeed {
            entry_id: solo,
            first_name: "Alice",
            last_name: "",
            plate: "12",
        },
    );
    let team = insert_entry(
        store,
        &EntrySeed {
            ride_id,
            plate: "77",
            key: "source-key-77",
            display_name: "Trail Blazers",
        },
    );
    insert_rider(
        store,
        &RiderSeed {
            entry_id: team,
            first_name: "A.",
            last_name: "Roy",
            plate: "77",
        },
    );
    insert_rider(
        store,
        &RiderSeed {
            entry_id: team,
            first_name: "K.",
            last_name: "Singh",
            plate: "77",
        },
    );
}

/// Return one ride's entry display names, in creation order.
fn display_names(store: &Store, ride_id: i64) -> Vec<String> {
    let mut statement = store
        .conn
        .prepare("SELECT display_name FROM entry WHERE ride_id = ?1 ORDER BY id")
        .expect("the display-name query prepares");
    statement
        .query_map([ride_id], |row| row.get::<_, String>(0))
        .expect("the display-name query runs")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("the display-name rows decode")
}

/// Return one ride's rider first and last names, in stored order.
fn rider_names(store: &Store, ride_id: i64) -> Vec<(String, String)> {
    let mut statement = store
        .conn
        .prepare(
            "SELECT first_name, last_name FROM rider \
             WHERE entry_id IN (SELECT id FROM entry WHERE ride_id = ?1) ORDER BY id",
        )
        .expect("the rider-name query prepares");
    statement
        .query_map([ride_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .expect("the rider-name query runs")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("the rider-name rows decode")
}

/// Return `db_path`'s backup files, newest first by name.
fn backup_files(db_path: &Path) -> Vec<PathBuf> {
    let directory = backup_dir_for(db_path);
    if !directory.is_dir() {
        return Vec::new();
    }
    let mut files: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&directory).expect("the backup directory reads") {
        let path = entry.expect("the directory entry reads").path();
        if path.extension().is_some_and(|extension| extension == "db") {
            files.push(path);
        }
    }
    files.sort();
    files.reverse();
    files
}

/// Return the newest session's `closed_at`.
fn newest_closed_at(store: &Store) -> Option<i64> {
    store
        .conn
        .query_row(
            "SELECT closed_at FROM app_session ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("the newest session reads")
}

/// Assert that the update test's stored row holds every edited value.
fn assert_edited_config(row: &StoredRide) {
    assert_edited_identity(row);
    assert_edited_numbers(row);
    assert_edited_policies(row);
}

/// Assert the edited name, date, venue and course.
fn assert_edited_identity(row: &StoredRide) {
    assert_eq!(
        (
            row.name.as_str(),
            row.event_date.as_str(),
            row.venue.as_str(),
            row.course_name.as_str()
        ),
        ("After", "2026-10-04", "New Venue", "New Venue")
    );
}

/// Assert the edited lengths, start and counts.
fn assert_edited_numbers(row: &StoredRide) {
    assert_eq!(
        (
            row.lap_km,
            row.organizer.as_str(),
            row.scorer.as_str(),
            row.planned_start
        ),
        (
            6.5,
            "Org 2",
            "Scorer 2",
            (planned_start() + Duration::hours(2)).unix_timestamp()
        )
    );
    assert_eq!(
        (
            row.planned_duration_s,
            row.min_lap_s,
            row.max_team_size,
            row.deck_count,
            row.jokers_per_deck
        ),
        (7_200, 90, 6, 2, 0)
    );
}

/// Assert the edited policies and tiebreak order.
fn assert_edited_policies(row: &StoredRide) {
    assert_eq!(
        (
            row.entry_mode.as_str(),
            row.plate_model.as_str(),
            row.jokers_mode.as_str(),
            row.max_cards,
            row.tiebreak_order.as_str(),
            row.hold_short_laps
        ),
        (
            "solo",
            "team_relay",
            "per_deck",
            Some(5),
            "[\"high_card\",\"laps\",\"total_time\"]",
            0
        )
    );
}

// ----------------------------------------------------------------- open

#[test]
fn test_store_open_creates_the_parent_directories() {
    // Arrange
    let directory = tempfile::tempdir().expect("the test directory is created");
    let db_path = directory
        .path()
        .join("nested")
        .join("deeper")
        .join("rides.db");

    // Act
    let store = Store::open(&db_path).expect("the store opens");

    // Assert
    assert!(db_path.is_file());
    drop(store);
}

#[test]
fn test_store_open_inserts_exactly_one_session_row() {
    // Arrange
    let (_directory, store) = temp_store();

    // Act
    let opened = store
        .conn
        .query_row(
            "SELECT opened_at, closed_at, active_ride_id, heartbeat_at FROM app_session",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            },
        )
        .expect("the session row reads");

    // Assert
    assert_eq!(table_count(&store, "app_session"), 1);
    assert!(opened.0 > 0);
    assert_eq!((opened.1, opened.2, opened.3), (None, None, None));
}

#[test]
fn test_store_open_refuses_a_newer_schema() {
    // Arrange
    let (directory, store) = temp_store();
    store
        .conn
        .execute("UPDATE schema_version SET version = 4", [])
        .expect("the version is stamped");
    drop(store);

    // Act
    let result = Store::open(directory.path().join("rides.db"));

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::SchemaVersionMismatch {
            current: 4,
            supported: 3
        })
    ));
}

/// A store error whose text names a transient SQLite open condition.
fn transient_open_error() -> StoreError {
    StoreError::Io(std::io::Error::other("disk I/O error"))
}

#[test]
fn test_store_should_retry_a_transient_error_on_an_early_attempt() {
    // Act
    let retry = super::should_retry(&transient_open_error(), 0);

    // Assert
    assert!(retry);
}

#[test]
fn test_store_should_retry_a_transient_error_stops_at_the_last_attempt() {
    // Act
    let retry = super::should_retry(&transient_open_error(), super::OPEN_RETRY_ATTEMPTS - 1);

    // Assert
    assert!(!retry);
}

#[test]
fn test_store_should_retry_a_persistent_error_is_false() {
    // Act
    let retry = super::should_retry(&StoreError::RideNotFound { id: 1 }, 0);

    // Assert
    assert!(!retry);
}

// ---------------------------------------------------------------- rides

#[test]
fn test_store_rides_counts_the_live_entries() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    insert_entry(
        &store,
        &EntrySeed {
            ride_id,
            plate: "12",
            key: "key-12",
            display_name: "Alice",
        },
    );
    insert_entry(
        &store,
        &EntrySeed {
            ride_id,
            plate: "77",
            key: "key-77",
            display_name: "Trail Blazers",
        },
    );
    let retired = insert_entry(
        &store,
        &EntrySeed {
            ride_id,
            plate: "99",
            key: "key-99",
            display_name: "Retired",
        },
    );
    store
        .conn
        .execute("UPDATE entry SET retired = 1 WHERE id = ?1", [retired])
        .expect("the retired flag is set");

    // Act
    let rows = store.rides().expect("the library reads");

    // Assert
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].id, rows[0].name.as_str(), rows[0].entries),
        (ride_id, "GORBA EPIC 2026", 2)
    );
}

#[test]
fn test_store_rides_orders_by_created_at_then_id() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let first = store
        .create_ride(&config_named("First"), None)
        .expect("the ride is created");
    let second = store
        .create_ride(&config_named("Second"), None)
        .expect("the ride is created");
    store
        .conn
        .execute("UPDATE ride SET created_at = 0 WHERE id = ?1", [second])
        .expect("the created time is rewritten");

    // Act
    let rows = store.rides().expect("the library reads");

    // Assert
    assert_eq!(
        rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        [second, first]
    );
}

#[test]
fn test_store_rides_reads_a_bad_event_date_as_invalid_row() {
    // Arrange
    let (_directory, mut store) = temp_store();
    store
        .create_ride(&config(), None)
        .expect("the ride is created");
    store
        .conn
        .execute("UPDATE ride SET event_date = 'not-a-date'", [])
        .expect("the event date is rewritten");

    // Act
    let result = store.rides();

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::InvalidRow {
            table: "ride",
            column: "event_date",
            ..
        })
    ));
}

#[test]
fn test_store_rides_reads_a_bad_status_as_invalid_row() {
    // Arrange
    let (_directory, mut store) = temp_store();
    store
        .create_ride(&config(), None)
        .expect("the ride is created");
    store
        .conn
        .execute_batch("PRAGMA ignore_check_constraints = ON")
        .expect("the check constraint is bypassed");
    store
        .conn
        .execute("UPDATE ride SET status = 'paused'", [])
        .expect("the status is rewritten");
    store
        .conn
        .execute_batch("PRAGMA ignore_check_constraints = OFF")
        .expect("the check constraint returns");

    // Act
    let result = store.rides();

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::InvalidRow {
            table: "ride",
            column: "status",
            ..
        })
    ));
}

// ----------------------------------------------------------- create_ride

#[test]
fn test_store_create_ride_returns_the_new_ride_id() {
    // Arrange
    let (_directory, mut store) = temp_store();

    // Act
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");

    // Assert
    assert_eq!(ride_id, 1);
}

#[test]
fn test_store_create_ride_round_trips_the_summary_row() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");

    // Act
    let rows = store.rides().expect("the library reads");

    // Assert
    assert_eq!(
        rows,
        [crate::row::RideRow {
            id: ride_id,
            name: "GORBA EPIC 2026".to_owned(),
            event_date: event_date(),
            status: RideStatus::Draft,
            entries: 0,
        }]
    );
}

#[test]
fn test_store_create_ride_stores_the_supplied_seed() {
    // Arrange
    let (_directory, mut store) = temp_store();

    // Act
    let ride_id = store
        .create_ride(&config(), Some(20_260_920))
        .expect("the ride is created");

    // Assert
    assert_eq!(stored_ride(&store, ride_id).rng_seed, 20_260_920);
}

#[test]
fn test_store_create_ride_draws_a_63_bit_seed_when_absent() {
    // Arrange
    let (_directory, mut store) = temp_store();

    // Act
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");

    // Assert
    let seed = stored_ride(&store, ride_id).rng_seed;
    assert!(seed >= 0);
    assert!(u64::try_from(seed).expect("the seed is positive") < (1_u64 << 63));
}

#[test]
fn test_store_random_63_bit_stays_below_two_to_the_63() {
    // Act
    let seed = random_63_bit();

    // Assert
    assert!(seed < (1_u64 << 63));
}

#[test]
fn test_store_create_ride_refuses_a_seed_above_the_signed_range() {
    // Arrange
    let (_directory, mut store) = temp_store();

    // Act
    let result = store.create_ride(&config(), Some(u64::MAX));

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::InvalidRow {
            table: "ride",
            column: "rng_seed",
            ..
        })
    ));
    assert_eq!(store.rides().expect("the library reads").len(), 0);
}

#[rstest]
#[case(true, 1)]
#[case(false, 0)]
fn test_store_create_ride_stores_the_hold_short_laps_policy(
    #[case] hold: bool,
    #[case] expected: i64,
) {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut config = config();
    config.hold_short_laps = hold;

    // Act
    let ride_id = store
        .create_ride(&config, None)
        .expect("the ride is created");

    // Assert
    assert_eq!(stored_ride(&store, ride_id).hold_short_laps, expected);
}

#[test]
fn test_store_create_ride_stores_the_logo_bytes() {
    // Arrange
    let (directory, mut store) = temp_store();
    let logo_path = directory.path().join("logo.png");
    std::fs::write(&logo_path, b"png-bytes").expect("the logo file is written");
    let mut config = config();
    config.logo_path = Some(logo_path);

    // Act
    let ride_id = store
        .create_ride(&config, None)
        .expect("the ride is created");

    // Assert
    assert_eq!(
        stored_ride(&store, ride_id).logo_png,
        Some(b"png-bytes".to_vec())
    );
}

#[test]
fn test_store_create_ride_without_a_logo_stores_null() {
    // Arrange
    let (_directory, mut store) = temp_store();

    // Act
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");

    // Assert
    assert_eq!(stored_ride(&store, ride_id).logo_png, None);
}

#[test]
fn test_store_create_ride_an_empty_logo_file_stores_an_empty_blob() {
    // Arrange
    let (directory, mut store) = temp_store();
    let logo_path = directory.path().join("empty.png");
    std::fs::write(&logo_path, b"").expect("the empty logo file is written");
    let mut config = config();
    config.logo_path = Some(logo_path);

    // Act
    let ride_id = store
        .create_ride(&config, None)
        .expect("the ride is created");

    // Assert
    assert_eq!(stored_ride(&store, ride_id).logo_png, Some(Vec::new()));
}

#[test]
fn test_store_create_ride_a_missing_logo_writes_no_row() {
    // Arrange
    let (directory, mut store) = temp_store();
    let mut config = config();
    config.logo_path = Some(directory.path().join("nope.png"));

    // Act
    let result = store.create_ride(&config, None);

    // Assert
    assert!(matches!(result, Err(StoreError::Io(_))));
    assert_eq!(store.rides().expect("the library reads").len(), 0);
}

#[rstest]
#[case(None, None)]
#[case(Some(5), Some(5))]
fn test_store_create_ride_stores_max_cards_null_or_set(
    #[case] max_cards: Option<u32>,
    #[case] expected: Option<i64>,
) {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut config = config();
    config.max_cards = max_cards;

    // Act
    let ride_id = store
        .create_ride(&config, None)
        .expect("the ride is created");

    // Assert
    assert_eq!(stored_ride(&store, ride_id).max_cards, expected);
}

#[test]
fn test_store_create_ride_writes_the_tiebreak_order_as_json() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut config = config();
    config.tiebreak_order = [TieBreak::Laps, TieBreak::HighCard, TieBreak::TotalTime];

    // Act
    let ride_id = store
        .create_ride(&config, None)
        .expect("the ride is created");

    // Assert
    assert_eq!(
        stored_ride(&store, ride_id).tiebreak_order,
        "[\"laps\",\"high_card\",\"total_time\"]"
    );
}

#[test]
fn test_store_create_ride_stores_the_enum_spellings_and_course_name() {
    // Arrange
    let (_directory, mut store) = temp_store();

    // Act
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    let row = stored_ride(&store, ride_id);

    // Assert
    assert_eq!(
        (
            row.status.as_str(),
            row.entry_mode.as_str(),
            row.plate_model.as_str()
        ),
        ("draft", "mixed", "rider_pooled")
    );
    assert_eq!(row.course_name, "Sea to Sky Gondola");
    assert_eq!(row.course_name, row.venue);
    assert_eq!(row.jokers_mode, "total");
}

#[test]
fn test_store_create_ride_stores_the_per_deck_jokers_mode() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut config = config();
    config.jokers_mode = JokersMode::PerDeck;

    // Act
    let ride_id = store
        .create_ride(&config, None)
        .expect("the ride is created");

    // Assert
    assert_eq!(stored_ride(&store, ride_id).jokers_mode, "per_deck");
}

#[test]
fn test_store_create_ride_stores_the_epoch_times() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let before = SystemClock.now().unix_timestamp();

    // Act
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    let after = SystemClock.now().unix_timestamp();

    // Assert
    let row = stored_ride(&store, ride_id);
    assert_eq!(row.planned_start, planned_start().unix_timestamp());
    assert_eq!(row.created_at, row.updated_at);
    assert!(row.created_at >= before);
    assert!(row.created_at <= after);
    assert_eq!((row.actual_start, row.finished_at), (None, None));
}

#[test]
fn test_store_create_ride_round_trips_the_event_date_text() {
    // Arrange
    let (_directory, mut store) = temp_store();

    // Act
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");

    // Assert
    assert_eq!(stored_ride(&store, ride_id).event_date, "2026-09-20");
}

#[test]
fn test_store_create_ride_gives_distinct_ids() {
    // Arrange
    let (_directory, mut store) = temp_store();

    // Act
    let first = store
        .create_ride(&config_named("First"), None)
        .expect("the ride is created");
    let second = store
        .create_ride(&config_named("Second"), None)
        .expect("the ride is created");

    // Assert
    assert_ne!(first, second);
    assert_eq!(
        store
            .rides()
            .expect("the library reads")
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>(),
        [first, second]
    );
}

// ---------------------------------------------------- update_ride_config

#[test]
fn test_store_update_ride_config_rewrites_every_config_column() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config_named("Before"), None)
        .expect("the ride is created");

    // Act
    store
        .update_ride_config(ride_id, &edited_config())
        .expect("the config updates");

    // Assert
    assert_edited_config(&stored_ride(&store, ride_id));
}

#[test]
fn test_store_update_ride_config_leaves_the_lifecycle_columns_alone() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    store
        .conn
        .execute(
            "UPDATE ride SET status = 'running', actual_start = 111, finished_at = 222 WHERE id = ?1",
            [ride_id],
        )
        .expect("the lifecycle columns are seeded");
    let before = stored_ride(&store, ride_id);

    // Act
    store
        .update_ride_config(ride_id, &config_named("Renamed"))
        .expect("the config updates");

    // Assert
    let after = stored_ride(&store, ride_id);
    assert_eq!(after.name, "Renamed");
    assert_eq!(after.status, "running");
    assert_eq!(
        (after.actual_start, after.finished_at),
        (Some(111), Some(222))
    );
    assert_eq!(after.rng_seed, before.rng_seed);
    assert_eq!(after.created_at, before.created_at);
    assert!(after.updated_at >= before.updated_at);
}

#[test]
fn test_store_update_ride_config_keeps_the_stored_logo_when_none_is_given() {
    // Arrange
    let (directory, mut store) = temp_store();
    let logo_path = directory.path().join("logo.png");
    std::fs::write(&logo_path, b"stored-logo").expect("the logo file is written");
    let mut config = config();
    config.logo_path = Some(logo_path);
    let ride_id = store
        .create_ride(&config, None)
        .expect("the ride is created");

    // Act
    store
        .update_ride_config(ride_id, &config_named("Renamed"))
        .expect("the config updates");

    // Assert
    assert_eq!(
        stored_ride(&store, ride_id).logo_png,
        Some(b"stored-logo".to_vec())
    );
}

#[test]
fn test_store_update_ride_config_replaces_the_logo_when_given() {
    // Arrange
    let (directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config_named("Before"), None)
        .expect("the ride is created");
    let logo_path = directory.path().join("new-logo.png");
    std::fs::write(&logo_path, b"new-logo").expect("the logo file is written");
    let mut edited = config_named("After");
    edited.logo_path = Some(logo_path);

    // Act
    store
        .update_ride_config(ride_id, &edited)
        .expect("the config updates");

    // Assert
    assert_eq!(
        stored_ride(&store, ride_id).logo_png,
        Some(b"new-logo".to_vec())
    );
}

#[test]
fn test_store_update_ride_config_a_missing_logo_rolls_the_update_back() {
    // Arrange
    let (directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config_named("Before"), None)
        .expect("the ride is created");
    let mut edited = config_named("After");
    edited.logo_path = Some(directory.path().join("nope.png"));

    // Act
    let result = store.update_ride_config(ride_id, &edited);

    // Assert
    assert!(matches!(result, Err(StoreError::Io(_))));
    assert_eq!(stored_ride(&store, ride_id).name, "Before");
}

#[test]
fn test_store_update_ride_config_unknown_ride_returns_ride_not_found() {
    // Arrange
    let (_directory, mut store) = temp_store();

    // Act
    let result = store.update_ride_config(999, &config());

    // Assert
    assert!(matches!(result, Err(StoreError::RideNotFound { id: 999 })));
}

// -------------------------------------------------------- duplicate_ride

#[test]
fn test_store_duplicate_ride_defaults_the_name_to_copy() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let source = store
        .create_ride(&config(), None)
        .expect("the ride is created");

    // Act
    let copy = store
        .duplicate_ride(source, None)
        .expect("the ride duplicates");

    // Assert
    assert_ne!(copy, source);
    assert_eq!(
        store
            .rides()
            .expect("the library reads")
            .iter()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>(),
        ["GORBA EPIC 2026", "GORBA EPIC 2026 (copy)"]
    );
}

#[test]
fn test_store_duplicate_ride_accepts_a_custom_name() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let source = store
        .create_ride(&config(), None)
        .expect("the ride is created");

    // Act
    let copy = store
        .duplicate_ride(source, Some("Winter Loop"))
        .expect("the ride duplicates");

    // Assert
    assert_eq!(stored_ride(&store, copy).name, "Winter Loop");
}

#[test]
fn test_store_duplicate_ride_starts_as_draft_with_no_timing() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let source = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    store
        .conn
        .execute(
            "UPDATE ride SET status = 'running', actual_start = 111, finished_at = 222 WHERE id = ?1",
            [source],
        )
        .expect("the source timing is seeded");

    // Act
    let copy = store
        .duplicate_ride(source, None)
        .expect("the ride duplicates");

    // Assert
    let row = stored_ride(&store, copy);
    assert_eq!(row.status, "draft");
    assert_eq!((row.actual_start, row.finished_at), (None, None));
    assert_eq!(row.event_date, "2026-09-20");
    assert_eq!(row.venue, "Sea to Sky Gondola");
    assert_eq!(row.hold_short_laps, 1);
}

#[test]
fn test_store_duplicate_ride_draws_a_fresh_seed() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let source = store
        .create_ride(&config(), Some(20_260_920))
        .expect("the ride is created");

    // Act
    let copy = store
        .duplicate_ride(source, None)
        .expect("the ride duplicates");

    // Assert
    assert_ne!(
        stored_ride(&store, copy).rng_seed,
        stored_ride(&store, source).rng_seed
    );
}

#[test]
fn test_store_duplicate_ride_copies_the_entries_and_riders() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let source = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    seed_two_entries(&store, source);

    // Act
    let copy = store
        .duplicate_ride(source, None)
        .expect("the ride duplicates");

    // Assert
    assert_eq!(table_count(&store, "entry"), 4);
    assert_eq!(table_count(&store, "rider"), 6);
    assert_eq!(display_names(&store, copy), ["Alice", "Trail Blazers"]);
    assert_eq!(
        rider_names(&store, copy),
        [
            ("Alice".to_owned(), String::new()),
            ("A.".to_owned(), "Roy".to_owned()),
            ("K.".to_owned(), "Singh".to_owned()),
        ]
    );
}

#[test]
fn test_store_duplicate_ride_copies_the_entry_notes_and_logo_card() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let source = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    let entry = insert_entry(
        &store,
        &EntrySeed {
            ride_id: source,
            plate: "12",
            key: "source-key-12",
            display_name: "Alice",
        },
    );
    store
        .conn
        .execute(
            "UPDATE entry SET notes = 'Club night', logo_card = 'club-logo' WHERE id = ?1",
            [entry],
        )
        .expect("the entry extras are seeded");

    // Act
    let copy = store
        .duplicate_ride(source, None)
        .expect("the ride duplicates");

    // Assert
    let (notes, logo_card): (Option<String>, Option<String>) = store
        .conn
        .query_row(
            "SELECT notes, logo_card FROM entry WHERE ride_id = ?1",
            [copy],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("the copied entry extras read");
    assert_eq!(
        (notes.as_deref(), logo_card.as_deref()),
        (Some("Club night"), Some("club-logo"))
    );
}

#[test]
fn test_store_duplicate_ride_gives_every_entry_a_fresh_key() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let source = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    insert_entry(
        &store,
        &EntrySeed {
            ride_id: source,
            plate: "12",
            key: "source-key-12",
            display_name: "Alice",
        },
    );

    // Act
    let copy = store
        .duplicate_ride(source, None)
        .expect("the ride duplicates");

    // Assert
    let copy_key: String = store
        .conn
        .query_row("SELECT key FROM entry WHERE ride_id = ?1", [copy], |row| {
            row.get(0)
        })
        .expect("the copy key reads");
    assert_ne!(copy_key, "source-key-12");
    assert_eq!(copy_key.len(), 32);
}

#[test]
fn test_store_duplicate_ride_copies_retired_entries() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let source = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    let retired = insert_entry(
        &store,
        &EntrySeed {
            ride_id: source,
            plate: "99",
            key: "source-key-99",
            display_name: "Retired",
        },
    );
    store
        .conn
        .execute("UPDATE entry SET retired = 1 WHERE id = ?1", [retired])
        .expect("the retired flag is set");

    // Act
    let copy = store
        .duplicate_ride(source, None)
        .expect("the ride duplicates");

    // Assert
    let copied_retired: i64 = store
        .conn
        .query_row(
            "SELECT retired FROM entry WHERE ride_id = ?1",
            [copy],
            |row| row.get(0),
        )
        .expect("the copied retired flag reads");
    assert_eq!(copied_retired, 1);
}

#[test]
fn test_store_duplicate_ride_writes_no_crossing_card_or_audit_rows() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let source = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    seed_timing_data(&store, source);

    // Act
    let copy = store
        .duplicate_ride(source, None)
        .expect("the ride duplicates");

    // Assert
    assert_eq!(
        (
            table_count(&store, "crossing"),
            table_count(&store, "card"),
            table_count(&store, "audit"),
        ),
        (1, 1, 1),
        "only the source rows remain"
    );
    assert_eq!(
        store
            .conn
            .query_row(
                "SELECT (SELECT COUNT(*) FROM crossing WHERE ride_id = ?1) \
                 + (SELECT COUNT(*) FROM card WHERE ride_id = ?1) \
                 + (SELECT COUNT(*) FROM audit WHERE ride_id = ?1)",
                [copy],
                |row| row.get::<_, i64>(0),
            )
            .expect("the copy counts"),
        0
    );
}

#[test]
fn test_store_duplicate_ride_keeps_the_source_untouched() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let source = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    store
        .conn
        .execute("UPDATE ride SET status = 'running' WHERE id = ?1", [source])
        .expect("the source status is seeded");

    // Act
    store
        .duplicate_ride(source, None)
        .expect("the ride duplicates");

    // Assert
    assert_eq!(stored_ride(&store, source).status, "running");
    assert_eq!(
        store
            .rides()
            .expect("the library reads")
            .iter()
            .map(|row| row.status)
            .collect::<Vec<_>>(),
        [RideStatus::Running, RideStatus::Draft]
    );
}

#[test]
fn test_store_duplicate_ride_unknown_ride_returns_ride_not_found() {
    // Arrange
    let (_directory, mut store) = temp_store();

    // Act
    let result = store.duplicate_ride(999, None);

    // Assert
    assert!(matches!(result, Err(StoreError::RideNotFound { id: 999 })));
}

// ----------------------------------------------------------- delete_ride

#[test]
fn test_store_delete_ride_writes_the_backup_before_deleting() {
    // Arrange
    let (directory, mut store) = temp_store();
    let db_path = directory.path().join("rides.db");
    let ride_id = store
        .create_ride(&config_named("Club poker night"), None)
        .expect("the ride is created");

    // Act
    store
        .delete_ride(ride_id, "Club poker night")
        .expect("the ride deletes");

    // Assert
    assert_eq!(store.rides().expect("the library reads").len(), 0);
    let files = backup_files(&db_path);
    assert_eq!(files.len(), 1);
    let reopened = Store::open(&files[0]).expect("the backup opens");
    assert_eq!(
        reopened.rides().expect("the backup library reads")[0].name,
        "Club poker night"
    );
}

#[test]
fn test_store_delete_ride_a_wrong_name_writes_no_backup() {
    // Arrange
    let (directory, mut store) = temp_store();
    let db_path = directory.path().join("rides.db");
    let ride_id = store
        .create_ride(&config_named("Club poker night"), None)
        .expect("the ride is created");

    // Act
    let result = store.delete_ride(ride_id, "club poker night");

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::RideNameMismatch { id, .. }) if id == ride_id
    ));
    assert_eq!(backup_files(&db_path).len(), 0);
    assert_eq!(store.rides().expect("the library reads").len(), 1);
}

#[test]
fn test_store_delete_ride_an_empty_name_is_a_mismatch() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config_named("Club poker night"), None)
        .expect("the ride is created");

    // Act
    let result = store.delete_ride(ride_id, "");

    // Assert
    assert!(matches!(result, Err(StoreError::RideNameMismatch { .. })));
}

#[test]
fn test_store_delete_ride_a_running_ride_is_refused_without_a_backup() {
    // Arrange
    let (directory, mut store) = temp_store();
    let db_path = directory.path().join("rides.db");
    let ride_id = store
        .create_ride(&config_named("Live"), None)
        .expect("the ride is created");
    store
        .conn
        .execute(
            "UPDATE ride SET status = 'running' WHERE id = ?1",
            [ride_id],
        )
        .expect("the status is seeded");

    // Act
    let result = store.delete_ride(ride_id, "Live");

    // Assert
    assert!(matches!(result, Err(StoreError::RideRunning { id }) if id == ride_id));
    assert_eq!(backup_files(&db_path).len(), 0);
    assert_eq!(store.rides().expect("the library reads").len(), 1);
}

#[test]
fn test_store_delete_ride_a_busy_backup_aborts_before_any_delete() {
    // Arrange
    let (directory, mut store) = temp_store();
    let db_path = directory.path().join("rides.db");
    let ride_id = store
        .create_ride(&config_named("Busy"), None)
        .expect("the ride is created");
    let holder = rusqlite::Connection::open(&db_path).expect("the holder connection opens");
    holder
        .execute_batch("BEGIN IMMEDIATE; CREATE TABLE held (x INTEGER);")
        .expect("the holder takes the write lock");

    // Act
    let result = store.delete_ride(ride_id, "Busy");

    // Assert
    assert!(matches!(result, Err(StoreError::CheckpointBusy { .. })));
    assert_eq!(store.rides().expect("the library reads").len(), 1);
    assert_eq!(backup_files(&db_path).len(), 0);
    drop(holder);
}

#[test]
fn test_store_delete_ride_unknown_ride_returns_ride_not_found() {
    // Arrange
    let (_directory, mut store) = temp_store();

    // Act
    let result = store.delete_ride(999, "any name");

    // Assert
    assert!(matches!(result, Err(StoreError::RideNotFound { id: 999 })));
}

#[test]
fn test_store_delete_ride_removes_every_dependent_row() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config_named("Dependents"), None)
        .expect("the ride is created");
    seed_timing_data(&store, ride_id);
    store
        .conn
        .execute(
            "UPDATE app_session SET active_ride_id = ?1 \
             WHERE id = (SELECT id FROM app_session ORDER BY id DESC LIMIT 1)",
            [ride_id],
        )
        .expect("the session ride is seeded");

    // Act
    store
        .delete_ride(ride_id, "Dependents")
        .expect("the ride deletes");

    // Assert
    assert_eq!(
        (
            table_count(&store, "ride"),
            table_count(&store, "entry"),
            table_count(&store, "rider"),
            table_count(&store, "crossing"),
            table_count(&store, "card"),
            table_count(&store, "audit"),
        ),
        (0, 0, 0, 0, 0, 0)
    );
    let active: Option<i64> = store
        .conn
        .query_row(
            "SELECT active_ride_id FROM app_session ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("the session reads");
    assert_eq!(active, None);
}

#[test]
fn test_store_delete_ride_keeps_other_rides() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let doomed = store
        .create_ride(&config_named("Doomed"), None)
        .expect("the ride is created");
    store
        .create_ride(&config_named("Kept"), None)
        .expect("the ride is created");

    // Act
    store
        .delete_ride(doomed, "Doomed")
        .expect("the ride deletes");

    // Assert
    assert_eq!(
        store
            .rides()
            .expect("the library reads")
            .iter()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>(),
        ["Kept"]
    );
}

// -------------------------------------------------------------- sessions

#[test]
fn test_store_previous_session_a_fresh_database_returns_clean_quit() {
    // Arrange
    let (_directory, store) = temp_store();

    // Act
    let previous = store.previous_session().expect("the session reads");

    // Assert
    assert_eq!(previous, PreviousSession::fresh());
}

#[test]
fn test_store_previous_session_a_clean_quit_uses_closed_at() {
    // Arrange
    let (directory, mut store) = temp_store();
    store.close_session().expect("the session closes");
    drop(store);
    let reopened = Store::open(directory.path().join("rides.db")).expect("the store reopens");

    // Act
    let previous = reopened.previous_session().expect("the session reads");

    // Assert
    let closed_at = reopened
        .conn
        .query_row(
            "SELECT closed_at FROM app_session ORDER BY id DESC LIMIT 1 OFFSET 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("the previous session reads");
    assert_eq!(previous.state, SessionState::CleanQuit);
    assert_eq!(previous.ride_id, None);
    assert_eq!(
        previous.ended_at,
        Some(Timestamp::UNIX_EPOCH + Duration::seconds(closed_at))
    );
}

#[test]
fn test_store_previous_session_a_crash_falls_back_to_opened_at() {
    // Arrange
    let (directory, store) = temp_store();
    drop(store);
    let reopened = Store::open(directory.path().join("rides.db")).expect("the store reopens");

    // Act
    let previous = reopened.previous_session().expect("the session reads");

    // Assert
    let opened_at = reopened
        .conn
        .query_row(
            "SELECT opened_at FROM app_session ORDER BY id DESC LIMIT 1 OFFSET 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("the previous session reads");
    assert_eq!(previous.state, SessionState::Crashed);
    assert_eq!(previous.ride_id, None);
    assert_eq!(
        previous.ended_at,
        Some(Timestamp::UNIX_EPOCH + Duration::seconds(opened_at))
    );
}

#[test]
fn test_store_previous_session_a_crash_uses_the_stored_heartbeat() {
    // Arrange
    let (directory, store) = temp_store();
    store
        .conn
        .execute(
            "UPDATE app_session SET heartbeat_at = 900 \
             WHERE id = (SELECT id FROM app_session ORDER BY id DESC LIMIT 1)",
            [],
        )
        .expect("the heartbeat is seeded");
    drop(store);
    let reopened = Store::open(directory.path().join("rides.db")).expect("the store reopens");

    // Act
    let previous = reopened.previous_session().expect("the session reads");

    // Assert
    assert_eq!(previous.state, SessionState::Crashed);
    assert_eq!(
        previous.ended_at,
        Some(Timestamp::UNIX_EPOCH + Duration::seconds(900))
    );
}

#[test]
fn test_store_previous_session_running_at_exit_carries_the_ride_and_closed_at() {
    // Arrange
    let (directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config_named("Session Ride"), None)
        .expect("the ride is created");
    drop(store);
    let mut reopened = Store::open(directory.path().join("rides.db")).expect("the store reopens");
    reopened
        .set_active_ride(ride_id)
        .expect("the ride is marked");
    reopened.close_session().expect("the session closes");
    let closed_at = newest_closed_at(&reopened).expect("the closed time is stamped");
    drop(reopened);
    let third = Store::open(directory.path().join("rides.db")).expect("the store reopens");

    // Act
    let previous = third.previous_session().expect("the session reads");

    // Assert
    assert_eq!(previous.state, SessionState::RunningAtExit);
    assert_eq!(previous.ride_id, Some(ride_id));
    assert_eq!(
        previous.ended_at,
        Some(Timestamp::UNIX_EPOCH + Duration::seconds(closed_at))
    );
}

#[test]
fn test_store_previous_session_reads_the_second_newest_row() {
    // Arrange
    let (directory, store) = temp_store();
    drop(store);
    let first = Store::open(directory.path().join("rides.db")).expect("the store opens");
    drop(first);
    let mut second = Store::open(directory.path().join("rides.db")).expect("the store opens");
    second.close_session().expect("the session closes");
    drop(second);
    let third = Store::open(directory.path().join("rides.db")).expect("the store opens");

    // Act
    let previous = third.previous_session().expect("the session reads");

    // Assert
    assert_eq!(previous.state, SessionState::CleanQuit);
}

#[test]
fn test_store_session_state_reads_the_previous_state() {
    // Arrange
    let (directory, mut store) = temp_store();
    store.close_session().expect("the session closes");
    drop(store);
    let reopened = Store::open(directory.path().join("rides.db")).expect("the store reopens");

    // Act
    let state = reopened.session_state().expect("the state reads");

    // Assert
    assert_eq!(state, SessionState::CleanQuit);
}

#[test]
fn test_store_close_session_stamps_closed_at() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let opened_at: i64 = store
        .conn
        .query_row(
            "SELECT opened_at FROM app_session ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("the open session reads");

    // Act
    store.close_session().expect("the session closes");

    // Assert
    let closed_at = newest_closed_at(&store).expect("the close stamps closed_at");
    assert!(closed_at >= opened_at);
}

#[test]
fn test_store_close_session_with_no_session_row_is_a_noop() {
    // Arrange
    let (_directory, mut store) = temp_store();
    store
        .conn
        .execute("DELETE FROM app_session", [])
        .expect("the session rows are cleared");

    // Act
    store.close_session().expect("the close is a no-op");

    // Assert
    assert_eq!(table_count(&store, "app_session"), 0);
}

#[test]
fn test_store_set_active_ride_marks_the_open_session() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config_named("Session Ride"), None)
        .expect("the ride is created");

    // Act
    store.set_active_ride(ride_id).expect("the ride is marked");

    // Assert
    let active: Option<i64> = store
        .conn
        .query_row(
            "SELECT active_ride_id FROM app_session ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("the session reads");
    assert_eq!(active, Some(ride_id));
}

#[test]
fn test_store_set_active_ride_an_unknown_id_is_rejected() {
    // Arrange
    let (_directory, mut store) = temp_store();

    // Act
    let result = store.set_active_ride(999);

    // Assert
    assert!(matches!(result, Err(StoreError::Sqlite(_))));
    let active: Option<i64> = store
        .conn
        .query_row(
            "SELECT active_ride_id FROM app_session ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("the session reads");
    assert_eq!(active, None);
}

#[test]
fn test_store_clear_active_ride_clears_the_open_session() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config_named("Session Ride"), None)
        .expect("the ride is created");
    store.set_active_ride(ride_id).expect("the ride is marked");

    // Act
    store.clear_active_ride().expect("the ride is cleared");

    // Assert
    let active: Option<i64> = store
        .conn
        .query_row(
            "SELECT active_ride_id FROM app_session ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("the session reads");
    assert_eq!(active, None);
}

#[test]
fn test_store_clear_active_ride_with_no_session_row_is_a_noop() {
    // Arrange
    let (_directory, mut store) = temp_store();
    store
        .conn
        .execute("DELETE FROM app_session", [])
        .expect("the session rows are cleared");

    // Act
    store.clear_active_ride().expect("the clear is a no-op");

    // Assert
    assert_eq!(table_count(&store, "app_session"), 0);
}

// ------------------------------------------------------------ backup_now

#[test]
fn test_store_backup_now_writes_a_backup_that_reopens_with_the_ride() {
    // Arrange
    let (directory, mut store) = temp_store();
    let db_path = directory.path().join("rides.db");
    let ride_id = store
        .create_ride(&config_named("Back me up"), None)
        .expect("the ride is created");

    // Act
    let target = store.backup_now().expect("the backup runs");

    // Assert
    assert!(target.is_file());
    assert_eq!(target.parent(), Some(backup_dir_for(&db_path).as_path()));
    let reopened = Store::open(&target).expect("the backup opens");
    let rows = reopened.rides().expect("the backup library reads");
    assert_eq!((rows[0].id, rows[0].name.as_str()), (ride_id, "Back me up"));
}

// ------------------------------------ phase 3: roster, event log, audit

/// Build an empty roster under the fixture ride's bounds.
fn roster_of(mode: EntryMode, plate_model: PlateModel) -> Roster {
    Roster::new(mode, 4, plate_model, Some(7)).expect("the fixture roster builds")
}

/// Build a solo entry with one rider on *plate*.
fn solo_entry(key: &str, plate: &str, name: &str) -> Entry {
    let mut rider = Rider {
        first_name: name.to_owned(),
        ..Rider::default()
    };
    rider.plate = Some(plate.to_owned());
    Entry::restored(key, plate, name, EntryType::Solo, vec![rider])
}

/// Build a rider with a first and a last name.
fn rider_named(first: &str, last: &str) -> Rider {
    Rider {
        first_name: first.to_owned(),
        last_name: last.to_owned(),
        ..Rider::default()
    }
}

/// Build the fixture config with the shortest lap that the replay tests need.
fn replay_config() -> RideConfig {
    let mut config = config();
    config.min_lap_s = 1;
    config
}

/// Create a ride with the fixture config and save *roster* onto it.
fn ride_with_roster(store: &mut Store, roster: &Roster) -> i64 {
    let ride_id = store
        .create_ride(&config(), Some(20_260_920))
        .expect("the ride is created");
    store
        .save_roster(ride_id, roster)
        .expect("the roster saves");
    ride_id
}

/// Create a replay ride carrying one solo entry, Alice on plate 12.
fn ride_for_replay(store: &mut Store) -> (i64, String) {
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    let mut alice = solo_entry("key-alice", "12", "Alice");
    alice.riders[0].sex = Some("F".to_owned());
    roster.load_entries(vec![alice]);
    let ride_id = store
        .create_ride(&replay_config(), Some(20_260_920))
        .expect("the ride is created");
    store
        .save_roster(ride_id, &roster)
        .expect("the roster saves");
    (ride_id, "key-alice".to_owned())
}

/// Build one engine event.
fn event(action: EventAction, payload: Value) -> Event {
    Event { action, payload }
}

/// Return the newest audit row of one ride.
fn latest_audit(store: &Store, ride_id: i64) -> (i64, i64, String, String) {
    store
        .conn
        .query_row(
            "SELECT id, at, action, payload_json FROM audit WHERE ride_id = ?1 \
             ORDER BY id DESC LIMIT 1",
            [ride_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("the audit row reads")
}

/// Return one ride's stored `updated_at`.
fn stored_updated_at(store: &Store, ride_id: i64) -> i64 {
    store
        .conn
        .query_row(
            "SELECT updated_at FROM ride WHERE id = ?1",
            [ride_id],
            |row| row.get(0),
        )
        .expect("the updated_at reads")
}

/// Return one ride's stored status spelling.
fn stored_status(store: &Store, ride_id: i64) -> String {
    store
        .conn
        .query_row("SELECT status FROM ride WHERE id = ?1", [ride_id], |row| {
            row.get(0)
        })
        .expect("the status reads")
}

/// Return one ride's stored entry rows as `(plate, type, team_size, retired)`.
fn stored_entries(store: &Store, ride_id: i64) -> Vec<(String, String, i64, i64)> {
    let mut statement = store
        .conn
        .prepare("SELECT plate, type, team_size, retired FROM entry WHERE ride_id = ?1 ORDER BY id")
        .expect("the entry query prepares");
    statement
        .query_map([ride_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .expect("the entry query runs")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("the entry rows decode")
}

/// One stored rider row: first name, last name, plate, sex and sort order.
type StoredRiderRow = (String, String, Option<String>, Option<String>, i64);

/// Return one ride's stored rider rows.
fn stored_riders(store: &Store, ride_id: i64) -> Vec<StoredRiderRow> {
    let mut statement = store
        .conn
        .prepare(
            "SELECT first_name, last_name, plate, sex, sort_order FROM rider \
             WHERE entry_id IN (SELECT id FROM entry WHERE ride_id = ?1) ORDER BY id",
        )
        .expect("the rider query prepares");
    statement
        .query_map([ride_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .expect("the rider query runs")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("the rider rows decode")
}

/// Return the stored id of the entry that carries *plate*.
fn stored_entry_id(store: &Store, ride_id: i64, plate: &str) -> i64 {
    store
        .conn
        .query_row(
            "SELECT id FROM entry WHERE ride_id = ?1 AND plate = ?2",
            params![ride_id, plate],
            |row| row.get(0),
        )
        .expect("the entry id reads")
}

/// Overwrite one ride's integer column.
#[expect(
    clippy::too_many_arguments,
    reason = "the seed names the store, the ride, the column and its value"
)]
fn seed_ride_int(store: &Store, ride_id: i64, column: &str, value: i64) {
    store
        .conn
        .execute(
            &format!("UPDATE ride SET {column} = ?1 WHERE id = ?2"),
            params![value, ride_id],
        )
        .expect("the ride column is seeded");
}

/// Overwrite one ride's text column with the CHECKs off.
#[expect(
    clippy::too_many_arguments,
    reason = "the seed names the store, the ride, the column and its value"
)]
fn seed_unchecked_ride_text(store: &Store, ride_id: i64, column: &str, value: &str) {
    store
        .conn
        .execute_batch("PRAGMA ignore_check_constraints = ON")
        .expect("the check constraint is bypassed");
    store
        .conn
        .execute(
            &format!("UPDATE ride SET {column} = ?1 WHERE id = ?2"),
            params![value, ride_id],
        )
        .expect("the ride column is seeded");
    store
        .conn
        .execute_batch("PRAGMA ignore_check_constraints = OFF")
        .expect("the check constraint returns");
}

/// Overwrite one audit row's payload text.
fn rewrite_audit_payload(store: &Store, row_id: i64, payload_json: &str) {
    store
        .conn
        .execute(
            "UPDATE audit SET payload_json = ?1 WHERE id = ?2",
            params![payload_json, row_id],
        )
        .expect("the payload is rewritten");
}

/// Insert one audit row with a fixed instant.
#[expect(
    clippy::too_many_arguments,
    reason = "the seed names the store, the ride, the action and the payload"
)]
fn insert_audit_payload(store: &Store, ride_id: i64, action: &str, payload: &Value) {
    let payload_json = serde_json::to_string(payload).expect("the payload encodes");
    store
        .conn
        .execute(
            "INSERT INTO audit (ride_id, at, action, payload_json) VALUES (?1, 100, ?2, ?3)",
            params![ride_id, action, payload_json],
        )
        .expect("the audit row inserts");
}

/// Return the fixture instant at *hour*:*minute* on the event date.
fn utc_at(hour: u8, minute: u8) -> Timestamp {
    let time = Time::from_hms(hour, minute, 0).expect("the fixture time is valid");
    OffsetDateTime::new_utc(event_date(), time)
}

/// Return the epoch second of the fixture instant at *hour*:*minute*.
fn utc_epoch(hour: u8, minute: u8) -> i64 {
    utc_at(hour, minute).unix_timestamp()
}

// ---------------------------------------------------------- roster_for

#[test]
fn test_store_roster_for_reads_the_ride_shape() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::TeamRelay);
    roster.load_entries(vec![solo_entry("key-alice", "12", "Alice")]);
    let ride_id = ride_with_roster(&mut store, &roster);

    // Act
    let rebuilt = store.roster_for(ride_id).expect("the roster loads");

    // Assert
    assert_eq!(
        (
            rebuilt.entry_mode(),
            rebuilt.plate_model(),
            rebuilt.max_team_size(),
            rebuilt.team_logo_seed()
        ),
        (
            EntryMode::Mixed,
            PlateModel::RiderPooled,
            4,
            Some(20_260_920)
        )
    );
}

#[test]
fn test_store_roster_for_round_trips_a_solo_entry() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut rider = rider_named("Alice", "Wong");
    rider.plate = Some("12".to_owned());
    rider.sex = Some("F".to_owned());
    rider.sort_order = 2;
    let mut alice = Entry::restored(
        "key-alice",
        "12",
        "Alice Wong",
        EntryType::Solo,
        vec![rider],
    );
    alice.notes = "Club night".to_owned();
    alice.logo_card = Some("As".to_owned());
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![alice]);
    let ride_id = ride_with_roster(&mut store, &roster);

    // Act
    let rebuilt = store.roster_for(ride_id).expect("the roster loads");

    // Assert
    let entry = &rebuilt.entries()[0];
    assert_eq!(
        (
            entry.key.as_str(),
            entry.plate.as_str(),
            entry.display_name.as_str(),
            entry.kind,
            entry.notes.as_str(),
            entry.logo_card.as_deref()
        ),
        (
            "key-alice",
            "12",
            "Alice Wong",
            EntryType::Solo,
            "Club night",
            Some("As")
        )
    );
    let rider = &entry.riders[0];
    assert_eq!(
        (
            rider.first_name.as_str(),
            rider.last_name.as_str(),
            rider.plate.as_deref(),
            rider.sex.as_deref(),
            rider.sort_order
        ),
        ("Alice", "Wong", Some("12"), Some("F"), 2)
    );
}

#[test]
fn test_store_roster_for_round_trips_a_pooled_team() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut first = rider_named("Sarah", "Roy");
    first.plate = Some("45".to_owned());
    first.sex = Some("F".to_owned());
    first.sort_order = 0;
    let mut second = rider_named("Priya", "Nair");
    second.plate = Some("46".to_owned());
    second.sort_order = 1;
    let team = Entry::restored(
        "key-team",
        "45",
        "Dirt Dynamos",
        EntryType::Team,
        vec![first, second],
    );
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![team]);
    let ride_id = ride_with_roster(&mut store, &roster);

    // Act
    let rebuilt = store.roster_for(ride_id).expect("the roster loads");

    // Assert
    let entry = &rebuilt.entries()[0];
    assert_eq!(
        (entry.kind, entry.riders.len(), entry.plate.as_str()),
        (EntryType::Team, 2, "45")
    );
    assert_eq!(
        entry
            .riders
            .iter()
            .map(|rider| rider.plate.clone().unwrap_or_default())
            .collect::<Vec<_>>(),
        ["45", "46"]
    );
}

#[test]
fn test_store_roster_for_round_trips_a_relay_team() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let first = rider_named("Sarah", "Roy");
    let second = rider_named("Priya", "Nair");
    let team = Entry::restored(
        "key-team",
        "77",
        "Trail Blazers",
        EntryType::Team,
        vec![first, second],
    );
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::TeamRelay);
    roster.load_entries(vec![team]);
    let ride_id = ride_with_roster(&mut store, &roster);

    // Act
    let rebuilt = store.roster_for(ride_id).expect("the roster loads");

    // Assert
    let entry = &rebuilt.entries()[0];
    assert_eq!((entry.plate.as_str(), entry.riders.len()), ("77", 2));
    assert_eq!(
        entry
            .riders
            .iter()
            .map(|rider| rider.plate.clone())
            .collect::<Vec<_>>(),
        [None, None]
    );
}

#[test]
fn test_store_roster_for_reads_a_zero_rider_team() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let empty = Entry::restored("key-empty", "9", "Empty Team", EntryType::Team, Vec::new());
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::TeamRelay);
    roster.load_entries(vec![empty]);
    let ride_id = ride_with_roster(&mut store, &roster);

    // Act
    let rebuilt = store.roster_for(ride_id).expect("the roster loads");

    // Assert
    let entry = &rebuilt.entries()[0];
    assert_eq!((entry.riders.len(), entry.kind), (0, EntryType::Team));
}

#[test]
fn test_store_roster_for_sets_the_ride_status() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![solo_entry("key-alice", "12", "Alice")]);
    let ride_id = ride_with_roster(&mut store, &roster);
    store
        .conn
        .execute(
            "UPDATE ride SET status = 'running' WHERE id = ?1",
            [ride_id],
        )
        .expect("the status is seeded");

    // Act
    let rebuilt = store.roster_for(ride_id).expect("the roster loads");

    // Assert
    assert_eq!(rebuilt.status(), RideStatus::Running);
}

#[test]
fn test_store_roster_for_unknown_ride_returns_ride_not_found() {
    // Arrange
    let (_directory, store) = temp_store();

    // Act
    let result = store.roster_for(999);

    // Assert
    assert!(matches!(result, Err(StoreError::RideNotFound { id: 999 })));
}

#[test]
fn test_store_roster_for_reads_a_bad_entry_mode_as_invalid_row() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![solo_entry("key-alice", "12", "Alice")]);
    let ride_id = ride_with_roster(&mut store, &roster);
    seed_unchecked_ride_text(&store, ride_id, "entry_mode", "teams");

    // Act
    let result = store.roster_for(ride_id);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::InvalidRow {
            table: "ride",
            column: "entry_mode",
            ..
        })
    ));
}

#[test]
fn test_store_roster_for_reads_a_bad_plate_model_as_invalid_row() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![solo_entry("key-alice", "12", "Alice")]);
    let ride_id = ride_with_roster(&mut store, &roster);
    seed_unchecked_ride_text(&store, ride_id, "plate_model", "pooled");

    // Act
    let result = store.roster_for(ride_id);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::InvalidRow {
            table: "ride",
            column: "plate_model",
            ..
        })
    ));
}

#[test]
fn test_store_roster_for_reads_a_negative_seed_as_invalid_row() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![solo_entry("key-alice", "12", "Alice")]);
    let ride_id = ride_with_roster(&mut store, &roster);
    seed_ride_int(&store, ride_id, "rng_seed", -1);

    // Act
    let result = store.roster_for(ride_id);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::InvalidRow {
            table: "ride",
            column: "rng_seed",
            ..
        })
    ));
}

#[test]
fn test_store_roster_for_reads_a_bad_max_team_size_as_invalid_roster() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![solo_entry("key-alice", "12", "Alice")]);
    let ride_id = ride_with_roster(&mut store, &roster);
    seed_ride_int(&store, ride_id, "max_team_size", 1);

    // Act
    let result = store.roster_for(ride_id);

    // Assert
    assert!(matches!(result, Err(StoreError::InvalidRoster { .. })));
}

#[rstest]
#[case("tandem", "type")]
#[case("void", "status")]
fn test_store_roster_for_reads_a_bad_entry_column_as_invalid_row(
    #[case] stored: &str,
    #[case] column: &'static str,
) {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![solo_entry("key-alice", "12", "Alice")]);
    let ride_id = ride_with_roster(&mut store, &roster);
    store
        .conn
        .execute_batch("PRAGMA ignore_check_constraints = ON")
        .expect("the check constraint is bypassed");
    store
        .conn
        .execute(
            &format!("UPDATE entry SET {column} = ?1 WHERE ride_id = ?2"),
            params![stored, ride_id],
        )
        .expect("the entry column is seeded");
    store
        .conn
        .execute_batch("PRAGMA ignore_check_constraints = OFF")
        .expect("the check constraint returns");

    // Act
    let result = store.roster_for(ride_id);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::InvalidRow {
            table: "entry",
            column: "type" | "status",
            ..
        })
    ));
}

#[test]
fn test_store_roster_for_derives_has_data_from_a_crossing_row() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![solo_entry("key-alice", "12", "Alice")]);
    let ride_id = ride_with_roster(&mut store, &roster);
    let entry_id = stored_entry_id(&store, ride_id, "12");
    insert_crossing(&store, ride_id, entry_id);

    // Act
    let rebuilt = store.roster_for(ride_id).expect("the roster loads");

    // Assert
    assert!(rebuilt.entries()[0].has_data);
}

#[test]
fn test_store_roster_for_derives_has_data_from_a_card_row() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![solo_entry("key-alice", "12", "Alice")]);
    let ride_id = ride_with_roster(&mut store, &roster);
    let entry_id = stored_entry_id(&store, ride_id, "12");
    let crossing_id = insert_crossing(&store, ride_id, entry_id);
    insert_card(
        &store,
        &CrossingIds {
            ride: ride_id,
            entry: entry_id,
            crossing: crossing_id,
        },
    );

    // Act
    let rebuilt = store.roster_for(ride_id).expect("the roster loads");

    // Assert
    assert!(rebuilt.entries()[0].has_data);
}

#[rstest]
#[case("record_crossing")]
#[case("deal_manual")]
#[case("add_crossing_at")]
#[case("assign_plate_to_miss")]
fn test_store_roster_for_derives_has_data_from_each_data_action(#[case] action: &str) {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![solo_entry("key-alice", "12", "Alice")]);
    let ride_id = ride_with_roster(&mut store, &roster);
    insert_audit_payload(&store, ride_id, action, &json!({"entry_id": "key-alice"}));

    // Act
    let rebuilt = store.roster_for(ride_id).expect("the roster loads");

    // Assert
    assert!(rebuilt.entries()[0].has_data);
}

#[test]
fn test_store_roster_for_ignores_an_audit_row_of_another_action() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![solo_entry("key-alice", "12", "Alice")]);
    let ride_id = ride_with_roster(&mut store, &roster);
    insert_audit_payload(&store, ride_id, "dnf", &json!({"entry_id": "key-alice"}));

    // Act
    let rebuilt = store.roster_for(ride_id).expect("the roster loads");

    // Assert
    assert!(!rebuilt.entries()[0].has_data);
}

#[test]
fn test_store_roster_for_ignores_an_audit_row_for_another_entry() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![solo_entry("key-alice", "12", "Alice")]);
    let ride_id = ride_with_roster(&mut store, &roster);
    insert_audit_payload(
        &store,
        ride_id,
        "record_crossing",
        &json!({"entry_id": "key-bob"}),
    );

    // Act
    let rebuilt = store.roster_for(ride_id).expect("the roster loads");

    // Assert
    assert!(!rebuilt.entries()[0].has_data);
}

#[test]
fn test_store_roster_for_leaves_has_data_false_without_recorded_rows() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![solo_entry("key-alice", "12", "Alice")]);
    let ride_id = ride_with_roster(&mut store, &roster);

    // Act
    let rebuilt = store.roster_for(ride_id).expect("the roster loads");

    // Assert
    assert!(!rebuilt.entries()[0].has_data);
}

// ---------------------------------------------------------- save_roster

#[test]
fn test_store_save_roster_writes_the_keys_verbatim() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![solo_entry("fixed-key-12", "12", "Alice")]);

    // Act
    let ride_id = ride_with_roster(&mut store, &roster);

    // Assert
    let key: String = store
        .conn
        .query_row(
            "SELECT key FROM entry WHERE ride_id = ?1",
            [ride_id],
            |row| row.get(0),
        )
        .expect("the key reads");
    assert_eq!(key, "fixed-key-12");
}

#[test]
fn test_store_save_roster_writes_the_team_size() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let first = rider_named("Sarah", "Roy");
    let second = rider_named("Priya", "Nair");
    let team = Entry::restored(
        "key-team",
        "45",
        "Dirt Dynamos",
        EntryType::Team,
        vec![first, second],
    );
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::TeamRelay);
    roster.load_entries(vec![solo_entry("key-alice", "12", "Alice"), team]);

    // Act
    let ride_id = ride_with_roster(&mut store, &roster);

    // Assert
    assert_eq!(
        stored_entries(&store, ride_id),
        [
            ("12".to_owned(), "solo".to_owned(), 1, 0),
            ("45".to_owned(), "team".to_owned(), 2, 0),
        ]
    );
}

#[test]
fn test_store_save_roster_writes_the_rider_sort_order() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut first = rider_named("Sarah", "Roy");
    first.sort_order = 5;
    let mut second = rider_named("Priya", "Nair");
    second.sort_order = 2;
    let team = Entry::restored(
        "key-team",
        "77",
        "Trail Blazers",
        EntryType::Team,
        vec![first, second],
    );
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::TeamRelay);
    roster.load_entries(vec![team]);

    // Act
    let ride_id = ride_with_roster(&mut store, &roster);
    let rebuilt = store.roster_for(ride_id).expect("the roster loads");

    // Assert
    assert_eq!(
        stored_riders(&store, ride_id)
            .iter()
            .map(|row| (row.0.as_str(), row.4))
            .collect::<Vec<_>>(),
        [("Sarah", 5), ("Priya", 2)]
    );
    assert_eq!(
        rebuilt.entries()[0]
            .riders
            .iter()
            .map(|rider| (rider.first_name.as_str(), rider.sort_order))
            .collect::<Vec<_>>(),
        [("Priya", 2), ("Sarah", 5)]
    );
}

#[test]
fn test_store_save_roster_writes_null_optional_columns() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![solo_entry("key-alice", "12", "Alice")]);

    // Act
    let ride_id = ride_with_roster(&mut store, &roster);

    // Assert
    let dnf_at: Option<i64> = store
        .conn
        .query_row(
            "SELECT dnf_at FROM entry WHERE ride_id = ?1",
            [ride_id],
            |row| row.get(0),
        )
        .expect("the entry row reads");
    let rider: (Option<String>, Option<i64>, Option<String>) = store
        .conn
        .query_row(
            "SELECT emergency_contact, waiver_signed, ccn_reg_id FROM rider LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("the rider row reads");
    assert_eq!((dnf_at, rider), (None, (None, None, None)));
}

#[test]
fn test_store_save_roster_replaces_the_previous_roster() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut first = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    first.load_entries(vec![
        solo_entry("key-alice", "12", "Alice"),
        solo_entry("key-bob", "13", "Bob"),
    ]);
    let ride_id = ride_with_roster(&mut store, &first);
    let mut second = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    second.load_entries(vec![solo_entry("key-carol", "14", "Carol")]);

    // Act
    store
        .save_roster(ride_id, &second)
        .expect("the roster saves");

    // Assert
    assert_eq!(
        stored_entries(&store, ride_id),
        [("14".to_owned(), "solo".to_owned(), 1, 0)]
    );
    assert_eq!(table_count(&store, "rider"), 1);
}

#[test]
fn test_store_save_roster_writes_a_retired_entry_with_the_retired_flag() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![solo_entry("key-live", "1", "Trail Blazers")]);
    roster.load_retired_entries(vec![solo_entry("key-retired", "1", "Bo")]);

    // Act
    let ride_id = ride_with_roster(&mut store, &roster);

    // Assert
    assert_eq!(
        stored_entries(&store, ride_id),
        [
            ("1".to_owned(), "solo".to_owned(), 1, 0),
            ("1".to_owned(), "solo".to_owned(), 0, 1),
        ]
    );
}

#[test]
fn test_store_save_roster_writes_no_riders_for_a_retired_entry() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let retired = solo_entry("key-retired", "9", "Bo");
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_retired_entries(vec![retired]);

    // Act
    let ride_id = ride_with_roster(&mut store, &roster);

    // Assert
    assert_eq!(stored_entries(&store, ride_id).len(), 1);
    assert_eq!(table_count(&store, "rider"), 0);
}

#[test]
fn test_store_save_roster_unknown_ride_returns_ride_not_found() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![solo_entry("key-alice", "12", "Alice")]);

    // Act
    let result = store.save_roster(999, &roster);

    // Assert
    assert!(matches!(result, Err(StoreError::RideNotFound { id: 999 })));
}

#[test]
fn test_store_save_roster_refuses_two_live_entries_on_one_plate() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![
        solo_entry("key-alice", "12", "Alice"),
        solo_entry("key-bob", "12", "Bob"),
    ]);
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");

    // Act
    let result = store.save_roster(ride_id, &roster);

    // Assert
    assert!(matches!(result, Err(StoreError::Sqlite(_))));
    assert_eq!(table_count(&store, "entry"), 0);
}

#[test]
fn test_store_save_roster_round_trips_the_rider_sex() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut rider = rider_named("Alice", "Wong");
    rider.plate = Some("12".to_owned());
    rider.sex = Some("F".to_owned());
    let alice = Entry::restored(
        "key-alice",
        "12",
        "Alice Wong",
        EntryType::Solo,
        vec![rider],
    );
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![alice]);
    let ride_id = ride_with_roster(&mut store, &roster);

    // Act
    let rebuilt = store.roster_for(ride_id).expect("the roster loads");

    // Assert
    assert_eq!(rebuilt.entries()[0].riders[0].sex.as_deref(), Some("F"));
}

// --------------------------------------------------------------- append

#[test]
fn test_store_append_reads_the_actual_start_timestamp() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");

    // Act
    store
        .append(
            ride_id,
            &event(
                EventAction::Start,
                json!({"actual_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the event appends");

    // Assert
    assert_eq!(latest_audit(&store, ride_id).1, utc_epoch(10, 0));
}

#[test]
fn test_store_append_reads_the_crossed_at_timestamp() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");

    // Act
    store
        .append(
            ride_id,
            &event(
                EventAction::RecordCrossing,
                json!({"crossed_at": "2026-09-20T10:02:00"}),
            ),
        )
        .expect("the event appends");

    // Assert
    assert_eq!(latest_audit(&store, ride_id).1, utc_epoch(10, 2));
}

#[test]
fn test_store_append_reads_the_stopped_at_timestamp() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");

    // Act
    store
        .append(
            ride_id,
            &event(
                EventAction::Stop,
                json!({"stopped_at": "2026-09-20T11:00:00"}),
            ),
        )
        .expect("the event appends");

    // Assert
    assert_eq!(latest_audit(&store, ride_id).1, utc_epoch(11, 0));
}

#[test]
fn test_store_append_reads_the_finished_at_timestamp() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");

    // Act
    store
        .append(
            ride_id,
            &event(
                EventAction::Finish,
                json!({"finished_at": "2026-09-20T12:00:00"}),
            ),
        )
        .expect("the event appends");

    // Assert
    assert_eq!(latest_audit(&store, ride_id).1, utc_epoch(12, 0));
}

#[test]
fn test_store_append_reads_the_reopened_at_timestamp() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");

    // Act
    store
        .append(
            ride_id,
            &event(
                EventAction::Reopen,
                json!({"reopened_at": "2026-09-20T12:05:00"}),
            ),
        )
        .expect("the event appends");

    // Assert
    assert_eq!(latest_audit(&store, ride_id).1, utc_epoch(12, 5));
}

#[test]
fn test_store_append_without_a_timestamp_writes_now() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    let before = SystemClock.now().unix_timestamp();

    // Act
    store
        .append(
            ride_id,
            &event(EventAction::DealManual, json!({"plate": "12"})),
        )
        .expect("the event appends");

    // Assert
    let at = latest_audit(&store, ride_id).1;
    assert!(at >= before);
    assert!(at <= SystemClock.now().unix_timestamp());
}

#[test]
fn test_store_append_writes_the_action_text_and_payload_json() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");

    // Act
    store
        .append(
            ride_id,
            &event(
                EventAction::RecordCrossing,
                json!({"plate": "12", "entry_id": "key-alice", "lap": 1}),
            ),
        )
        .expect("the event appends");

    // Assert
    let (_, _, action, payload_json) = latest_audit(&store, ride_id);
    assert_eq!(action, "record_crossing");
    assert_eq!(
        serde_json::from_str::<Value>(&payload_json).expect("the payload decodes"),
        json!({"plate": "12", "entry_id": "key-alice", "lap": 1})
    );
}

#[rstest]
#[case(EventAction::Start, "2026-09-20T10:00:00", "running")]
#[case(EventAction::Continue, "2026-09-20T10:00:00", "running")]
#[case(EventAction::Finish, "2026-09-20T12:00:00", "finished")]
#[case(EventAction::Reopen, "2026-09-20T12:05:00", "reopened")]
fn test_store_append_lifecycle_action_writes_the_mapped_status(
    #[case] action: EventAction,
    #[case] stamped: &str,
    #[case] expected: &str,
) {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    let key = match action {
        EventAction::Finish => "finished_at",
        EventAction::Reopen => "reopened_at",
        _ => "actual_start",
    };

    // Act
    store
        .append(ride_id, &event(action, json!({ key: stamped })))
        .expect("the event appends");

    // Assert
    assert_eq!(stored_status(&store, ride_id), expected);
}

#[test]
fn test_store_append_lifecycle_action_stamps_updated_at() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    seed_ride_int(&store, ride_id, "updated_at", 1);
    let before = SystemClock.now().unix_timestamp();

    // Act
    store
        .append(
            ride_id,
            &event(
                EventAction::Start,
                json!({"actual_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the event appends");

    // Assert
    assert!(stored_updated_at(&store, ride_id) >= before);
}

#[test]
fn test_store_append_record_crossing_leaves_status_and_updated_at() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    seed_ride_int(&store, ride_id, "updated_at", 1);

    // Act
    store
        .append(
            ride_id,
            &event(
                EventAction::RecordCrossing,
                json!({"plate": "12", "entry_id": "key-alice",
                       "crossed_at": "2026-09-20T10:02:00"}),
            ),
        )
        .expect("the event appends");

    // Assert
    assert_eq!(stored_status(&store, ride_id), "draft");
    assert_eq!(stored_updated_at(&store, ride_id), 1);
}

#[test]
fn test_store_append_stop_leaves_the_ride_running() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    store
        .conn
        .execute(
            "UPDATE ride SET status = 'running' WHERE id = ?1",
            [ride_id],
        )
        .expect("the status is seeded");
    seed_ride_int(&store, ride_id, "updated_at", 1);

    // Act
    store
        .append(
            ride_id,
            &event(
                EventAction::Stop,
                json!({"stopped_at": "2026-09-20T11:00:00"}),
            ),
        )
        .expect("the event appends");

    // Assert
    assert_eq!(stored_status(&store, ride_id), "running");
    assert_eq!(stored_updated_at(&store, ride_id), 1);
}

#[test]
fn test_store_append_unknown_ride_returns_a_foreign_key_error() {
    // Arrange
    let (_directory, mut store) = temp_store();

    // Act
    let result = store.append(
        999,
        &event(
            EventAction::Start,
            json!({"actual_start": "2026-09-20T10:00:00"}),
        ),
    );

    // Assert
    match result {
        Err(StoreError::Sqlite(error)) => {
            assert!(error.to_string().contains("FOREIGN KEY"));
        }
        other => panic!("expected a foreign key failure, got {other:?}"),
    }
}

// -------------------------------------------------- append_roster_event

#[test]
fn test_store_append_roster_event_stamps_now() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    let before = SystemClock.now().unix_timestamp();
    let roster_event = AuditEvent {
        action: "change_solo_plate".to_owned(),
        payload: json!({"old_plate": "12", "new_plate": "13"}),
    };

    // Act
    store
        .append_roster_event(ride_id, &roster_event)
        .expect("the roster event appends");

    // Assert
    let (_, at, action, payload_json) = latest_audit(&store, ride_id);
    assert!(at >= before);
    assert!(at <= SystemClock.now().unix_timestamp());
    assert_eq!(action, "change_solo_plate");
    assert_eq!(
        serde_json::from_str::<Value>(&payload_json).expect("the payload decodes"),
        json!({"old_plate": "12", "new_plate": "13"})
    );
}

#[test]
fn test_store_append_roster_event_leaves_status_and_updated_at() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    store
        .conn
        .execute(
            "UPDATE ride SET status = 'running' WHERE id = ?1",
            [ride_id],
        )
        .expect("the status is seeded");
    seed_ride_int(&store, ride_id, "updated_at", 1);
    let roster_event = AuditEvent {
        action: "change_team_plate".to_owned(),
        payload: json!({"new_plate": "9"}),
    };

    // Act
    store
        .append_roster_event(ride_id, &roster_event)
        .expect("the roster event appends");

    // Assert
    assert_eq!(stored_status(&store, ride_id), "running");
    assert_eq!(stored_updated_at(&store, ride_id), 1);
}

#[test]
fn test_store_append_roster_event_unknown_ride_returns_ride_not_found() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let roster_event = AuditEvent {
        action: "change_solo_plate".to_owned(),
        payload: json!({}),
    };

    // Act
    let result = store.append_roster_event(999, &roster_event);

    // Assert
    assert!(matches!(result, Err(StoreError::RideNotFound { id: 999 })));
}

// ----------------------------------------------------------- audit_rows

#[test]
fn test_store_audit_rows_projects_fields_newest_first() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, key) = ride_for_replay(&mut store);
    store
        .append(
            ride_id,
            &event(
                EventAction::Start,
                json!({"actual_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the start appends");
    store
        .append(
            ride_id,
            &event(
                EventAction::RecordCrossing,
                json!({"plate": "12", "entry_id": key, "crossed_at": "2026-09-20T10:02:00"}),
            ),
        )
        .expect("the crossing appends");
    store
        .append(
            ride_id,
            &event(
                EventAction::EditCrossing,
                json!({"entry_id": key, "seq": 1, "crossed_at": "2026-09-20T10:03:00",
                       "reason": "mis-keyed time"}),
            ),
        )
        .expect("the edit appends");

    // Act
    let rows = store.audit_rows(ride_id).expect("the trail reads");

    // Assert
    assert_eq!(
        rows.iter()
            .map(|row| (
                row.at,
                row.action.as_str(),
                row.entry.as_str(),
                row.reason.as_str()
            ))
            .collect::<Vec<_>>(),
        [
            (utc_at(10, 3), "edit_crossing", "12", "mis-keyed time"),
            (utc_at(10, 2), "record_crossing", "12", ""),
            (utc_at(10, 0), "start", "", ""),
        ]
    );
}

#[test]
fn test_store_audit_rows_an_empty_trail_returns_empty() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");

    // Act
    let rows = store.audit_rows(ride_id).expect("the trail reads");

    // Assert
    assert_eq!(rows, []);
}

#[test]
fn test_store_audit_rows_falls_back_to_the_raw_key() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, _key) = ride_for_replay(&mut store);
    insert_audit_payload(
        &store,
        ride_id,
        "record_crossing",
        &json!({"entry_id": "key-unknown"}),
    );

    // Act
    let rows = store.audit_rows(ride_id).expect("the trail reads");

    // Assert
    assert_eq!(rows[0].entry, "key-unknown");
}

#[test]
fn test_store_audit_rows_falls_back_to_the_plate() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    insert_audit_payload(
        &store,
        ride_id,
        "record_crossing",
        &json!({"plate": "77", "lap": 2}),
    );

    // Act
    let rows = store.audit_rows(ride_id).expect("the trail reads");

    // Assert
    assert_eq!(rows[0].entry, "77");
}

#[rstest]
#[case("old_plate", "12")]
#[case("new_plate", "13")]
#[case("display_name", "Alice")]
fn test_store_audit_rows_falls_back_through_the_plate_keys(
    #[case] field: &str,
    #[case] expected: &str,
) {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    insert_audit_payload(
        &store,
        ride_id,
        "change_solo_plate",
        &json!({ field: expected }),
    );

    // Act
    let rows = store.audit_rows(ride_id).expect("the trail reads");

    // Assert
    assert_eq!(rows[0].entry, expected);
}

#[test]
fn test_store_audit_rows_prefers_old_plate_over_new_plate() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    insert_audit_payload(
        &store,
        ride_id,
        "change_solo_plate",
        &json!({"display_name": "Alice", "old_plate": "12", "new_plate": "13"}),
    );

    // Act
    let rows = store.audit_rows(ride_id).expect("the trail reads");

    // Assert
    assert_eq!(rows[0].entry, "12");
}

#[test]
fn test_store_audit_rows_reads_the_dnf_display() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    insert_audit_payload(
        &store,
        ride_id,
        "dnf",
        &json!({"entry_id": "9", "plate": "45", "display": "45 · Sarah"}),
    );

    // Act
    let rows = store.audit_rows(ride_id).expect("the trail reads");

    // Assert
    assert_eq!(rows[0].entry, "45 · Sarah");
}

#[test]
fn test_store_audit_rows_a_dnf_without_display_falls_back() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    insert_audit_payload(
        &store,
        ride_id,
        "dnf",
        &json!({"entry_id": "9", "plate": "45", "rider": true}),
    );

    // Act
    let rows = store.audit_rows(ride_id).expect("the trail reads");

    // Assert
    assert_eq!(rows[0].entry, "9");
}

#[test]
fn test_store_audit_rows_reads_the_tiebreak_summary() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    insert_audit_payload(
        &store,
        ride_id,
        "tiebreak_draw",
        &json!({"draws": [{"entry_id": "12", "card": "5H"}],
                "summary": "12 · 5H, 34 · AH"}),
    );

    // Act
    let rows = store.audit_rows(ride_id).expect("the trail reads");

    // Assert
    assert_eq!(rows[0].entry, "12 · 5H, 34 · AH");
}

#[test]
fn test_store_audit_rows_a_tiebreak_without_summary_is_blank() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    insert_audit_payload(&store, ride_id, "tiebreak_draw", &json!({"draws": []}));

    // Act
    let rows = store.audit_rows(ride_id).expect("the trail reads");

    // Assert
    assert_eq!(rows[0].entry, "");
}

#[test]
fn test_store_audit_rows_reads_the_reason() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    insert_audit_payload(&store, ride_id, "record_miss", &json!({"reason": "bandit"}));

    // Act
    let rows = store.audit_rows(ride_id).expect("the trail reads");

    // Assert
    assert_eq!(rows[0].reason, "bandit");
}

#[test]
fn test_store_audit_rows_scopes_to_the_requested_ride() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let first = store
        .create_ride(&config_named("First"), None)
        .expect("the ride is created");
    let second = store
        .create_ride(&config_named("Second"), None)
        .expect("the ride is created");
    insert_audit_payload(&store, first, "record_crossing", &json!({"plate": "10"}));
    insert_audit_payload(&store, second, "record_crossing", &json!({"plate": "20"}));

    // Act
    let first_rows = store.audit_rows(first).expect("the trail reads");
    let second_rows = store.audit_rows(second).expect("the trail reads");

    // Assert
    assert_eq!((first_rows.len(), second_rows.len()), (1, 1));
    assert_eq!(first_rows[0].entry, "10");
    assert_eq!(second_rows[0].entry, "20");
}

#[test]
fn test_store_audit_rows_unknown_ride_returns_ride_not_found() {
    // Arrange
    let (_directory, store) = temp_store();

    // Act
    let result = store.audit_rows(999);

    // Assert
    assert!(matches!(result, Err(StoreError::RideNotFound { id: 999 })));
}

#[test]
fn test_store_audit_rows_a_malformed_payload_returns_invalid_payload() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    store
        .conn
        .execute(
            "INSERT INTO audit (ride_id, at, action, payload_json) VALUES (?1, 1, 'start', ?2)",
            params![ride_id, "{\"actual_start\": "],
        )
        .expect("the audit row inserts");
    let row_id: i64 = store
        .conn
        .query_row(
            "SELECT id FROM audit WHERE ride_id = ?1",
            [ride_id],
            |row| row.get(0),
        )
        .expect("the row id reads");

    // Act
    let result = store.audit_rows(ride_id);

    // Assert
    assert!(matches!(result, Err(StoreError::InvalidPayload { row_id: id }) if id == row_id));
}

#[test]
fn test_store_audit_rows_a_non_object_payload_returns_invalid_payload() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    store
        .conn
        .execute(
            "INSERT INTO audit (ride_id, at, action, payload_json) VALUES (?1, 1, 'start', '[]')",
            [ride_id],
        )
        .expect("the audit row inserts");
    let row_id: i64 = store
        .conn
        .query_row(
            "SELECT id FROM audit WHERE ride_id = ?1",
            [ride_id],
            |row| row.get(0),
        )
        .expect("the row id reads");

    // Act
    let result = store.audit_rows(ride_id);

    // Assert
    assert!(matches!(result, Err(StoreError::InvalidPayload { row_id: id }) if id == row_id));
}

#[test]
fn test_store_audit_rows_an_out_of_range_at_returns_invalid_row() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    insert_audit_payload(&store, ride_id, "start", &json!({}));
    store
        .conn
        .execute(
            "UPDATE audit SET at = ?1 WHERE ride_id = ?2",
            params![i64::MAX, ride_id],
        )
        .expect("the instant is seeded");

    // Act
    let result = store.audit_rows(ride_id);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::InvalidRow {
            table: "audit",
            column: "at",
            ..
        })
    ));
}

#[test]
fn test_store_roster_for_round_trips_an_entry_status() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut alice = solo_entry("key-alice", "12", "Alice");
    alice.status = EntryStatus::Dnf;
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![alice]);
    let ride_id = ride_with_roster(&mut store, &roster);

    // Act
    let rebuilt = store.roster_for(ride_id).expect("the roster loads");

    // Assert
    assert_eq!(rebuilt.entries()[0].status, EntryStatus::Dnf);
}

// ---------------------------------------------------------- load_engine

#[test]
fn test_store_load_engine_reconstructs_the_config() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), Some(20_260_920))
        .expect("the ride is created");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert_eq!(engine.config(), &config());
}

#[test]
fn test_store_load_engine_builds_the_total_jokers_shoe_from_the_seed() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), Some(20_260_920))
        .expect("the ride is created");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert_eq!(engine.shoe_total(), 8 * 52 + 1);
    assert_eq!(engine.shoe_remaining(), 8 * 52 + 1);
}

#[test]
fn test_store_load_engine_builds_the_per_deck_jokers_shoe() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut config = config();
    config.deck_count = 2;
    config.jokers_per_deck = 2;
    config.jokers_mode = JokersMode::PerDeck;
    let ride_id = store
        .create_ride(&config, Some(20_260_920))
        .expect("the ride is created");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert_eq!(engine.shoe_total(), 2 * (52 + 2));
}

#[test]
fn test_store_load_engine_replays_start_and_crossing_into_running() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, key) = ride_for_replay(&mut store);
    store
        .append(
            ride_id,
            &event(
                EventAction::Start,
                json!({"actual_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the start appends");
    store
        .append(
            ride_id,
            &event(
                EventAction::RecordCrossing,
                json!({"plate": "12", "entry_id": key, "crossed_at": "2026-09-20T10:02:00"}),
            ),
        )
        .expect("the crossing appends");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert_eq!(engine.state(), RideStatus::Running);
    assert_eq!(engine.crossings().len(), 1);
    assert_eq!(engine.crossings()[0].entry_id, EntryId(key.clone()));
    assert_eq!(engine.crossings()[0].crossed_at, utc_at(10, 2));
    assert_eq!(engine.lap_times(&EntryId(key)), [120.0]);
}

#[test]
fn test_store_load_engine_replays_in_append_order_not_at_order() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, key) = ride_for_replay(&mut store);
    store
        .append(
            ride_id,
            &event(
                EventAction::Start,
                json!({"actual_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the start appends");
    store
        .append(
            ride_id,
            &event(
                EventAction::RecordCrossing,
                json!({"plate": "12", "entry_id": key, "crossed_at": "2026-09-20T10:02:00"}),
            ),
        )
        .expect("the crossing appends");
    store
        .append(
            ride_id,
            &event(
                EventAction::SetStartTime,
                json!({"actual_start": "2026-09-20T09:55:00",
                       "previous_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the correction appends");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert_eq!(
        engine
            .events()
            .iter()
            .map(|event| event.action)
            .collect::<Vec<_>>(),
        [
            EventAction::Start,
            EventAction::RecordCrossing,
            EventAction::SetStartTime
        ]
    );
    assert_eq!(engine.lap_times(&EntryId(key)), [420.0]);
}

/// Seed a ride whose log holds Alice's short first lap, and return the held card.
fn short_lap_hold(store: &mut Store) -> (i64, Card) {
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![solo_entry("key-alice", "12", "Alice")]);
    let ride_id = store
        .create_ride(&config(), Some(20_260_920))
        .expect("the ride is created");
    store
        .save_roster(ride_id, &roster)
        .expect("the roster saves");
    store
        .append(
            ride_id,
            &event(
                EventAction::Start,
                json!({"actual_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the start appends");
    store
        .append(
            ride_id,
            &event(
                EventAction::RecordCrossing,
                json!({"plate": "12", "entry_id": "key-alice",
                       "crossed_at": "2026-09-20T10:00:30"}),
            ),
        )
        .expect("the crossing appends");
    let card = store
        .load_engine(ride_id)
        .expect("the held card reads")
        .held_crossings()[0]
        .card
        .clone();
    (ride_id, card)
}

#[test]
fn test_store_load_engine_replays_a_short_lap_hold() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, card) = short_lap_hold(&mut store);
    store
        .append(
            ride_id,
            &event(
                EventAction::ConfirmHeld,
                json!({"entry_id": "key-alice", "seq": 1, "card": card.code()}),
            ),
        )
        .expect("the confirmation appends");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert!(engine.config().hold_short_laps);
    assert_eq!(engine.held_crossings(), []);
    let entry = engine
        .snapshot()
        .into_iter()
        .find(|entry| entry.plate == "12")
        .expect("the entry is live");
    assert_eq!(entry.cards, [card]);
}

#[test]
fn test_store_load_engine_missing_ride_returns_ride_not_found() {
    // Arrange
    let (_directory, store) = temp_store();

    // Act
    let result = store.load_engine(999);

    // Assert
    assert!(matches!(result, Err(StoreError::RideNotFound { id: 999 })));
}

#[rstest]
#[case("null")]
#[case("not json")]
#[case("\"laps\"")]
#[case("[]")]
#[case("[\"laps\"]")]
#[case("[\"laps\", \"high_card\"]")]
#[case("[\"laps\", \"high_card\", \"total_time\", \"laps\"]")]
#[case("[\"laps\", \"high_card\", \"fastest\"]")]
fn test_store_load_engine_an_invalid_tiebreak_order_returns_tiebreak_order(#[case] stored: &str) {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    store
        .conn
        .execute(
            "UPDATE ride SET tiebreak_order = ?1 WHERE id = ?2",
            params![stored, ride_id],
        )
        .expect("the tiebreak order is seeded");

    // Act
    let result = store.load_engine(ride_id);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::TiebreakOrder { ride_id: id, stored: ref text })
            if id == ride_id && text == stored
    ));
}

#[rstest]
#[case("{\"actual_start\": ")]
#[case("{}")]
#[case("{\"actual_start\": \"yesterday\"}")]
#[case("[]")]
fn test_store_load_engine_a_bad_audit_payload_returns_replay(#[case] payload_json: &str) {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, _key) = ride_for_replay(&mut store);
    store
        .append(
            ride_id,
            &event(
                EventAction::Start,
                json!({"actual_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the start appends");
    let (row_id, _, _, _) = latest_audit(&store, ride_id);
    rewrite_audit_payload(&store, row_id, payload_json);

    // Act
    let result = store.load_engine(ride_id);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::Replay { row_id: id, ride_id: ride, .. })
            if id == row_id && ride == ride_id
    ));
}

#[test]
fn test_store_load_engine_a_refused_crossing_returns_replay() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, _key) = ride_for_replay(&mut store);
    store
        .append(
            ride_id,
            &event(
                EventAction::Start,
                json!({"actual_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the start appends");
    store
        .append(
            ride_id,
            &event(
                EventAction::RecordCrossing,
                json!({"plate": "12", "entry_id": "key-nope",
                       "crossed_at": "2026-09-20T10:02:00"}),
            ),
        )
        .expect("the crossing appends");
    let (row_id, _, _, _) = latest_audit(&store, ride_id);

    // Act
    let result = store.load_engine(ride_id);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::Replay { row_id: id, ride_id: ride, detail: ref text })
            if id == row_id && ride == ride_id && text.contains("key-nope")
    ));
}

#[test]
fn test_store_load_engine_skips_an_unknown_action_row() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, _key) = ride_for_replay(&mut store);
    store
        .append(
            ride_id,
            &event(
                EventAction::Start,
                json!({"actual_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the start appends");
    let roster_event = AuditEvent {
        action: "change_team_plate".to_owned(),
        payload: json!({"display_name": "A", "old_plate": "1", "new_plate": "2"}),
    };
    store
        .append_roster_event(ride_id, &roster_event)
        .expect("the roster event appends");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert_eq!(
        engine
            .events()
            .iter()
            .map(|event| event.action)
            .collect::<Vec<_>>(),
        [EventAction::Start]
    );
    assert_eq!(engine.state(), RideStatus::Running);
}

#[rstest]
#[case("max_team_size", 1)]
#[case("deck_count", 0)]
#[case("jokers_per_deck", 11)]
#[case("planned_duration_s", 0)]
fn test_store_load_engine_invalid_config_bounds_return_invalid_config(
    #[case] column: &str,
    #[case] value: i64,
) {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    seed_ride_int(&store, ride_id, column, value);

    // Act
    let result = store.load_engine(ride_id);

    // Assert
    assert!(matches!(result, Err(StoreError::InvalidConfig { .. })));
}

#[rstest]
#[case("deck_count", 300)]
#[case("jokers_per_deck", 300)]
fn test_store_load_engine_a_u8_overflow_returns_out_of_range(
    #[case] column: &str,
    #[case] value: i64,
) {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    seed_ride_int(&store, ride_id, column, value);

    // Act
    let result = store.load_engine(ride_id);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::OutOfRange { field, value: stored, max: 255 })
            if field == column && stored == value
    ));
}

#[test]
fn test_store_load_engine_materialises_the_stored_logo() {
    // Arrange
    let (directory, mut store) = temp_store();
    let logo_path = directory.path().join("logo.png");
    std::fs::write(&logo_path, b"png-bytes").expect("the logo file is written");
    let mut config = config();
    config.logo_path = Some(logo_path);
    let ride_id = store
        .create_ride(&config, None)
        .expect("the ride is created");

    // Act
    let first = store
        .load_engine(ride_id)
        .expect("the first engine rebuilds");
    let second = store
        .load_engine(ride_id)
        .expect("the second engine rebuilds");

    // Assert
    let first_path = first
        .config()
        .logo_path
        .clone()
        .expect("the logo materialises");
    let second_path = second
        .config()
        .logo_path
        .clone()
        .expect("the logo materialises");
    assert_ne!(first_path, second_path);
    assert_eq!(
        std::fs::read(&first_path).expect("the first logo reads"),
        b"png-bytes"
    );
    assert_eq!(
        std::fs::read(&second_path).expect("the second logo reads"),
        b"png-bytes"
    );
    drop(std::fs::remove_file(&first_path));
    drop(std::fs::remove_file(&second_path));
}

#[test]
fn test_store_load_engine_without_a_logo_leaves_logo_path_none() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert_eq!(engine.config().logo_path, None);
}

#[test]
fn test_store_load_engine_an_empty_logo_leaves_logo_path_none() {
    // Arrange
    let (directory, mut store) = temp_store();
    let logo_path = directory.path().join("empty.png");
    std::fs::write(&logo_path, b"").expect("the empty logo file is written");
    let mut config = config();
    config.logo_path = Some(logo_path);
    let ride_id = store
        .create_ride(&config, None)
        .expect("the ride is created");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert_eq!(engine.config().logo_path, None);
}

#[test]
fn test_store_load_engine_reads_a_bad_event_date_as_invalid_row() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    store
        .conn
        .execute(
            "UPDATE ride SET event_date = '20/09/2026' WHERE id = ?1",
            [ride_id],
        )
        .expect("the event date is seeded");

    // Act
    let result = store.load_engine(ride_id);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::InvalidRow {
            table: "ride",
            column: "event_date",
            ..
        })
    ));
}

#[test]
fn test_store_load_engine_reads_a_bad_planned_start_as_invalid_row() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    seed_ride_int(&store, ride_id, "planned_start", i64::MAX);

    // Act
    let result = store.load_engine(ride_id);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::InvalidRow {
            table: "ride",
            column: "planned_start",
            ..
        })
    ));
}

#[test]
fn test_store_load_engine_reads_a_bad_entry_mode_as_invalid_row() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    seed_unchecked_ride_text(&store, ride_id, "entry_mode", "teams");

    // Act
    let result = store.load_engine(ride_id);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::InvalidRow {
            table: "ride",
            column: "entry_mode",
            ..
        })
    ));
}

#[test]
fn test_store_load_engine_reads_a_negative_seed_as_invalid_row() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    seed_ride_int(&store, ride_id, "rng_seed", -1);

    // Act
    let result = store.load_engine(ride_id);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::InvalidRow {
            table: "ride",
            column: "rng_seed",
            ..
        })
    ));
}

// ------------------------------------- self-contained replay actions

#[test]
fn test_store_load_engine_replays_a_start() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, _key) = ride_for_replay(&mut store);
    store
        .append(
            ride_id,
            &event(
                EventAction::Start,
                json!({"actual_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the start appends");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert_eq!(engine.state(), RideStatus::Running);
    assert_eq!(engine.events().len(), 1);
}

#[test]
fn test_store_load_engine_replays_a_continue() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, _key) = ride_for_replay(&mut store);
    store
        .append(ride_id, &event(EventAction::Continue, json!({})))
        .expect("the continue appends");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert_eq!(engine.state(), RideStatus::Running);
}

#[test]
fn test_store_load_engine_replays_a_set_start_time() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, _key) = ride_for_replay(&mut store);
    store
        .append(
            ride_id,
            &event(
                EventAction::Start,
                json!({"actual_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the start appends");
    store
        .append(
            ride_id,
            &event(
                EventAction::SetStartTime,
                json!({"actual_start": "2026-09-20T09:55:00",
                       "previous_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the correction appends");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert_eq!(engine.actual_start(), Some(utc_at(9, 55)));
}

#[test]
fn test_store_load_engine_replays_a_record_crossing() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, key) = ride_for_replay(&mut store);
    store
        .append(
            ride_id,
            &event(
                EventAction::Start,
                json!({"actual_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the start appends");
    store
        .append(
            ride_id,
            &event(
                EventAction::RecordCrossing,
                json!({"plate": "12", "entry_id": key, "crossed_at": "2026-09-20T10:02:00"}),
            ),
        )
        .expect("the crossing appends");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert_eq!(engine.crossings().len(), 1);
}

#[test]
fn test_store_load_engine_replays_a_record_miss() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, _key) = ride_for_replay(&mut store);
    store
        .append(
            ride_id,
            &event(
                EventAction::Start,
                json!({"actual_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the start appends");
    store
        .append(
            ride_id,
            &event(
                EventAction::RecordMiss,
                json!({"crossed_at": "2026-09-20T10:01:00", "reason": "bandit"}),
            ),
        )
        .expect("the miss appends");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert_eq!(engine.pending_misses().len(), 1);
}

#[test]
fn test_store_load_engine_replays_a_deal_manual() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, key) = ride_for_replay(&mut store);
    store
        .append(
            ride_id,
            &event(
                EventAction::Start,
                json!({"actual_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the start appends");
    store
        .append(
            ride_id,
            &event(
                EventAction::DealManual,
                json!({"plate": "12", "entry_id": key, "reason": "bonus"}),
            ),
        )
        .expect("the manual deal appends");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert_eq!(engine.shoe_remaining(), engine.shoe_total() - 1);
}

#[test]
fn test_store_load_engine_replays_an_add_crossing_at() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, key) = ride_for_replay(&mut store);
    store
        .append(
            ride_id,
            &event(
                EventAction::Start,
                json!({"actual_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the start appends");
    store
        .append(
            ride_id,
            &event(
                EventAction::AddCrossingAt,
                json!({"plate": "12", "entry_id": key,
                       "crossed_at": "2026-09-20T10:02:00", "reason": "manual"}),
            ),
        )
        .expect("the crossing appends");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert_eq!(engine.crossings().len(), 1);
}

#[test]
fn test_store_load_engine_replays_a_dnf() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, key) = ride_for_replay(&mut store);
    store
        .append(
            ride_id,
            &event(
                EventAction::Start,
                json!({"actual_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the start appends");
    store
        .append(
            ride_id,
            &event(
                EventAction::Dnf,
                json!({"entry_id": key, "plate": "12", "rider": false,
                       "reason": "mechanical"}),
            ),
        )
        .expect("the dnf appends");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert!(engine.snapshot()[0].dnf);
}

#[test]
fn test_store_load_engine_replays_a_stop() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, _key) = ride_for_replay(&mut store);
    store
        .append(
            ride_id,
            &event(
                EventAction::Start,
                json!({"actual_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the start appends");
    store
        .append(
            ride_id,
            &event(
                EventAction::Stop,
                json!({"stopped_at": "2026-09-20T11:00:00"}),
            ),
        )
        .expect("the stop appends");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert!(engine.stopped());
}

#[test]
fn test_store_load_engine_replays_a_finish() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, _key) = ride_for_replay(&mut store);
    store
        .append(
            ride_id,
            &event(
                EventAction::Start,
                json!({"actual_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the start appends");
    store
        .append(
            ride_id,
            &event(
                EventAction::Finish,
                json!({"finished_at": "2026-09-20T12:00:00"}),
            ),
        )
        .expect("the finish appends");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert_eq!(engine.state(), RideStatus::Finished);
}

#[test]
fn test_store_load_engine_replays_a_reopen() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, _key) = ride_for_replay(&mut store);
    store
        .append(
            ride_id,
            &event(
                EventAction::Start,
                json!({"actual_start": "2026-09-20T10:00:00"}),
            ),
        )
        .expect("the start appends");
    store
        .append(
            ride_id,
            &event(
                EventAction::Finish,
                json!({"finished_at": "2026-09-20T12:00:00"}),
            ),
        )
        .expect("the finish appends");
    store
        .append(
            ride_id,
            &event(
                EventAction::Reopen,
                json!({"reopened_at": "2026-09-20T12:05:00"}),
            ),
        )
        .expect("the reopen appends");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert_eq!(engine.state(), RideStatus::Reopened);
}

#[test]
fn test_store_load_engine_replays_a_tiebreak_draw() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, _key) = ride_for_replay(&mut store);
    store
        .append(
            ride_id,
            &event(EventAction::TiebreakDraw, json!({"draws": []})),
        )
        .expect("the draw appends");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert_eq!(engine.events().len(), 1);
}

#[test]
fn test_store_load_engine_replays_a_shoe_reshuffle() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let (ride_id, _key) = ride_for_replay(&mut store);
    store
        .append(ride_id, &event(EventAction::ShoeReshuffle, json!({})))
        .expect("the reshuffle appends");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert_eq!(engine.state(), RideStatus::Draft);
}

// ---------------------------------------------- the action vocabulary

#[rstest]
#[case("start", EventAction::Start)]
#[case("continue", EventAction::Continue)]
#[case("set_start_time", EventAction::SetStartTime)]
#[case("record_crossing", EventAction::RecordCrossing)]
#[case("confirm_held", EventAction::ConfirmHeld)]
#[case("void_held", EventAction::VoidHeld)]
#[case("return_to_held", EventAction::ReturnToHeld)]
#[case("undo", EventAction::Undo)]
#[case("deal_manual", EventAction::DealManual)]
#[case("edit_crossing", EventAction::EditCrossing)]
#[case("void_crossing", EventAction::VoidCrossing)]
#[case("add_crossing_at", EventAction::AddCrossingAt)]
#[case("record_miss", EventAction::RecordMiss)]
#[case("assign_plate_to_miss", EventAction::AssignPlateToMiss)]
#[case("reassign", EventAction::Reassign)]
#[case("dnf", EventAction::Dnf)]
#[case("void_card", EventAction::VoidCard)]
#[case("move_rider", EventAction::MoveRider)]
#[case("extract_rider_to_solo", EventAction::ExtractRiderToSolo)]
#[case("stop", EventAction::Stop)]
#[case("finish", EventAction::Finish)]
#[case("reopen", EventAction::Reopen)]
#[case("tiebreak_draw", EventAction::TiebreakDraw)]
#[case("shoe_reshuffle", EventAction::ShoeReshuffle)]
fn test_store_event_action_reads_each_spelling(
    #[case] stored: &str,
    #[case] expected: EventAction,
) {
    // Act
    let parsed = serde_json::from_value::<EventAction>(Value::String(stored.to_owned()))
        .expect("the action parses");

    // Assert
    assert_eq!(parsed, expected);
}

// ------------------------------------------------- the v1 chain to the log

/// The released v1 ledger, frozen.
const V1_LEDGER_DDL: &str = "CREATE TABLE schema_version (id INTEGER PRIMARY KEY CHECK (id = 1), \
     version INTEGER NOT NULL)";

/// The released v1 schema, frozen: `entry` carries no `key` or `retired`.
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

/// The v1 seed rows: one RUNNING ride, Alice on plate 12, and its log.
const V1_SEED: [&str; 6] = [
    "
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
    ",
    "
    INSERT INTO entry (id, ride_id, plate, display_name, type, team_size,
                       status, dnf_at, notes, logo_card)
    VALUES (1, 1, '12', 'Alice', 'solo', 1, 'active', NULL, '', NULL)
    ",
    "
    INSERT INTO rider (id, entry_id, first_name, last_name, plate, sort_order,
                       sex, emergency_contact, waiver_signed, ccn_reg_id)
    VALUES (1, 1, 'Alice', 'Wong', '12', 0, 'F', NULL, NULL, NULL)
    ",
    "
    INSERT INTO audit (id, ride_id, at, action, payload_json)
    VALUES (1, 1, 1789898400, 'start_ride', '{\"source\": \"setup\"}')
    ",
    "
    INSERT INTO audit (id, ride_id, at, action, payload_json)
    VALUES (2, 1, 1789898400, 'start', '{\"actual_start\": \"2026-09-20T10:00:00\"}')
    ",
    "
    INSERT INTO audit (id, ride_id, at, action, payload_json)
    VALUES (3, 1, 1789898520, 'record_crossing',
            '{\"plate\": \"12\", \"entry_id\": \"12\", \"lap\": 1,
              \"crossed_at\": \"2026-09-20T10:02:00\"}')
    ",
];

/// Write a released-build v1 database file with the fixture seed.
fn write_v1_engine_file(path: &Path) {
    let conn = Connection::open(path).expect("the v1 file opens");
    conn.execute_batch("PRAGMA foreign_keys = OFF")
        .expect("the foreign keys are disabled");
    for statement in V1_DDL.iter().chain([V1_LEDGER_DDL].iter()) {
        conn.execute_batch(statement).expect("the v1 DDL applies");
    }
    conn.execute_batch("INSERT INTO schema_version (id, version) VALUES (1, 1)")
        .expect("the v1 ledger stamps");
    for statement in V1_SEED {
        conn.execute_batch(statement).expect("the v1 seed applies");
    }
    drop(conn);
}

#[test]
fn test_store_load_engine_a_v1_file_migrates_and_reaches_running() {
    // Arrange
    let directory = tempfile::tempdir().expect("the test directory is created");
    let path = directory.path().join("v1.db");
    write_v1_engine_file(&path);

    // Act
    let store = Store::open(&path).expect("the v1 file opens and migrates");
    let engine = store.load_engine(1).expect("the engine rebuilds");

    // Assert
    let key: String = store
        .conn
        .query_row(
            "SELECT key FROM entry WHERE ride_id = 1 AND plate = '12'",
            [],
            |row| row.get(0),
        )
        .expect("the migrated key reads");
    let payload: String = store
        .conn
        .query_row("SELECT payload_json FROM audit WHERE id = 3", [], |row| {
            row.get(0)
        })
        .expect("the rewritten payload reads");
    let parsed: Value = serde_json::from_str(&payload).expect("the payload decodes");
    assert_eq!(engine.state(), RideStatus::Running);
    assert_eq!(
        parsed.get("entry_id").and_then(Value::as_str),
        Some(key.as_str())
    );
    assert_eq!(
        engine
            .events()
            .iter()
            .map(|event| event.action)
            .collect::<Vec<_>>(),
        [EventAction::Start, EventAction::RecordCrossing]
    );
}

#[test]
fn test_store_load_engine_round_trips_max_cards_and_hold_short_laps() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut config = config();
    config.max_cards = Some(5);
    config.hold_short_laps = false;
    config.event_date =
        Date::from_calendar_date(2026, Month::October, 4).expect("the edited date is valid");
    let ride_id = store
        .create_ride(&config, None)
        .expect("the ride is created");

    // Act
    let engine = store.load_engine(ride_id).expect("the engine rebuilds");

    // Assert
    assert_eq!(
        (
            engine.config().max_cards,
            engine.config().hold_short_laps,
            engine.config().event_date
        ),
        (Some(5), false, config.event_date)
    );
}

#[test]
fn test_store_load_engine_reads_a_negative_max_cards_as_out_of_range() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    seed_ride_int(&store, ride_id, "max_cards", -1);

    // Act
    let result = store.load_engine(ride_id);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::OutOfRange {
            field: "max_cards",
            value: -1,
            ..
        })
    ));
}

#[test]
fn test_store_load_engine_reads_a_count_above_i32_as_out_of_range() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    seed_ride_int(&store, ride_id, "planned_duration_s", i64::MAX);

    // Act
    let result = store.load_engine(ride_id);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::OutOfRange {
            field: "planned_duration_s",
            max: 2_147_483_647,
            ..
        })
    ));
}

#[test]
fn test_store_load_engine_reads_a_max_team_size_above_u32_as_out_of_range() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    seed_ride_int(&store, ride_id, "max_team_size", i64::MAX);

    // Act
    let result = store.load_engine(ride_id);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::OutOfRange {
            field: "max_team_size",
            ..
        })
    ));
}

#[test]
fn test_store_load_engine_reads_a_bad_plate_model_as_invalid_row() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    seed_unchecked_ride_text(&store, ride_id, "plate_model", "pooled");

    // Act
    let result = store.load_engine(ride_id);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::InvalidRow {
            table: "ride",
            column: "plate_model",
            ..
        })
    ));
}

#[test]
fn test_store_roster_for_ignores_a_malformed_data_audit_row() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let mut roster = roster_of(EntryMode::Mixed, PlateModel::RiderPooled);
    roster.load_entries(vec![solo_entry("key-alice", "12", "Alice")]);
    let ride_id = ride_with_roster(&mut store, &roster);
    store
        .conn
        .execute(
            "INSERT INTO audit (ride_id, at, action, payload_json) \
             VALUES (?1, 1, 'record_crossing', '{\"entry_id\": ')",
            [ride_id],
        )
        .expect("the malformed audit row inserts");

    // Act
    let rebuilt = store.roster_for(ride_id).expect("the roster loads");

    // Assert
    assert!(!rebuilt.entries()[0].has_data);
}

#[test]
fn test_store_audit_rows_a_dnf_with_an_empty_display_falls_back_to_the_plate() {
    // Arrange
    let (_directory, mut store) = temp_store();
    let ride_id = store
        .create_ride(&config(), None)
        .expect("the ride is created");
    insert_audit_payload(
        &store,
        ride_id,
        "dnf",
        &json!({"display": "", "plate": "45"}),
    );

    // Act
    let rows = store.audit_rows(ride_id).expect("the trail reads");

    // Assert
    assert_eq!(rows[0].entry, "45");
}
