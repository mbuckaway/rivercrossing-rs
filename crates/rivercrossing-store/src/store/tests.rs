// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the `Store`: open, the ride library, the config writes,
//! duplication, the delete guard and the session bookkeeping.
//!
//! Every test drives a real SQLite file in a `TempDir`. The store's own
//! connection is used for the raw seeds that the oracle tests also write
//! directly, such as a RUNNING status or a heartbeat.

use std::path::{Path, PathBuf};

use pretty_assertions::assert_eq;
use rivercrossing_cards::JokersMode;
use rivercrossing_ride::{
    Clock, EntryMode, PlateModel, RideConfig, RideConfigParams, RideStatus, SystemClock, TieBreak,
    Timestamp,
};
use rstest::rstest;
use rusqlite::params;
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
