// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the backup primitives: one copy with rotation, restore and
//! the hourly state machine.
//!
//! Every test drives a real SQLite file in a `TempDir`; the only injected
//! piece is a clock, so the file names and the hour buckets are deterministic.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use pretty_assertions::assert_eq;
use rivercrossing_ride::{Clock, Timestamp};
use rstest::rstest;
use rusqlite::Connection;
use time::{Date, Duration, Month, OffsetDateTime, Time};

use super::{DEFAULT_KEEP, backup_dir_for, restore, run, schedule_hourly};
use crate::error::StoreError;
use crate::schema::{apply_pragmas, ensure_schema};

/// A minimal valid ride row, so a test seeds a database without a `Store`.
const VALID_RIDE_INSERT: &str = "
    INSERT INTO ride (
        name, event_date, venue, course_name, lap_km, organizer, scorer,
        planned_start, planned_duration_s, status, entry_mode, max_team_size,
        plate_model, min_lap_s, deck_count, jokers_per_deck, tiebreak_order,
        rng_seed, created_at, updated_at
    ) VALUES (
        'GORBA EPIC 2026', '2026-09-20', 'Sea to Sky Gondola', 'Sea to Sky Gondola',
        8.0, 'GORBA', 'K. Singh', 1789898400, 21600, 'draft', 'mixed', 4,
        'rider_pooled', 1080, 8, 1, '[\"high_card\",\"laps\",\"total_time\"]',
        20260920, 1789898400, 1789898400
    )
";

/// A clock whose reading a test moves by hand.
#[derive(Debug, Clone)]
struct FakeClock {
    now: Rc<Cell<Timestamp>>,
}

impl FakeClock {
    fn new(now: Timestamp) -> Self {
        Self {
            now: Rc::new(Cell::new(now)),
        }
    }

    fn set(&self, now: Timestamp) {
        self.now.set(now);
    }

    fn advance(&self, step: Duration) {
        self.now.set(self.now.get() + step);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Timestamp {
        self.now.get()
    }
}

/// Return 20 September 2026 at *hour*:*minute* UTC.
fn instant(hour: u8, minute: u8) -> Timestamp {
    let date = Date::from_calendar_date(2026, Month::September, 20).expect("the date is valid");
    let time = Time::from_hms(hour, minute, 0).expect("the time is valid");
    OffsetDateTime::new_utc(date, time)
}

/// Return 20 September 2026 at 10:00:00.123456 UTC.
fn precise_instant() -> Timestamp {
    let date = Date::from_calendar_date(2026, Month::September, 20).expect("the date is valid");
    let time = Time::from_hms_micro(10, 0, 0, 123_456).expect("the time is valid");
    OffsetDateTime::new_utc(date, time)
}

/// Create a current-schema database holding one ride at `db_path`.
fn create_database(db_path: &Path) {
    let mut conn = Connection::open(db_path).expect("the seeded database opens");
    apply_pragmas(&conn).expect("the PRAGMAs apply");
    ensure_schema(&mut conn).expect("the schema is created");
    conn.execute(VALID_RIDE_INSERT, [])
        .expect("the ride inserts");
}

/// Return the ride names stored at `db_path`, oldest first.
fn ride_names(db_path: &Path) -> Vec<String> {
    let conn = Connection::open(db_path).expect("the database opens");
    let mut statement = conn
        .prepare("SELECT name FROM ride ORDER BY id")
        .expect("the ride query prepares");
    let names = statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("the ride query runs");
    names
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("the ride rows decode")
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

/// Return one path's file name as text.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
}

/// The rotation fixture: one database, one clock and one keep count.
struct Rotation<'a> {
    /// The database to back up.
    db_path: &'a Path,
    /// The clock that names each backup.
    clock: &'a FakeClock,
    /// The number of files to retain.
    keep: usize,
}

impl Rotation<'_> {
    /// Advance one minute and write one backup.
    fn advance_and_run(&self) -> PathBuf {
        self.clock.advance(Duration::minutes(1));
        run(self.db_path, self.keep, self.clock).expect("the backup runs")
    }

    /// Write *runs* backups; return the written paths in order.
    ///
    /// The repeated runs are the setup of the rotation tests, so the loop
    /// lives here and the test body keeps one act.
    fn write(&self, runs: usize) -> Vec<PathBuf> {
        let mut written = Vec::with_capacity(runs);
        for _ in 0..runs {
            written.push(self.advance_and_run());
        }
        written
    }
}

// ------------------------------------------------------------------- run

#[test]
fn test_backup_run_writes_a_timestamped_file_beside_the_database() {
    // Arrange
    let directory = tempfile::tempdir().expect("the test directory is created");
    let db_path = directory.path().join("rides.db");
    create_database(&db_path);
    let clock = FakeClock::new(precise_instant());

    // Act
    let target = run(&db_path, DEFAULT_KEEP, &clock).expect("the backup runs");

    // Assert
    let expected_directory = backup_dir_for(&db_path);
    assert_eq!(target.parent(), Some(expected_directory.as_path()));
    assert_eq!(
        target.file_name().expect("the backup has a file name"),
        "rides.20260920-100000-123456.db"
    );
}

#[test]
fn test_backup_run_backup_reopens_with_the_ride() {
    // Arrange
    let directory = tempfile::tempdir().expect("the test directory is created");
    let db_path = directory.path().join("rides.db");
    create_database(&db_path);
    let clock = FakeClock::new(instant(10, 0));

    // Act
    let target = run(&db_path, DEFAULT_KEEP, &clock).expect("the backup runs");

    // Assert
    assert_eq!(ride_names(&target), ["GORBA EPIC 2026".to_owned()]);
}

#[test]
fn test_backup_run_a_live_wal_database_is_checkpointed_before_the_copy() {
    // Arrange
    let directory = tempfile::tempdir().expect("the test directory is created");
    let db_path = directory.path().join("rides.db");
    let mut live = Connection::open(&db_path).expect("the live database opens");
    apply_pragmas(&live).expect("the PRAGMAs apply");
    ensure_schema(&mut live).expect("the schema is created");
    live.execute(VALID_RIDE_INSERT, [])
        .expect("the ride inserts");
    let clock = FakeClock::new(instant(10, 0));

    // Act
    let target = run(&db_path, DEFAULT_KEEP, &clock).expect("the backup runs");

    // Assert
    drop(live);
    assert_eq!(ride_names(&target), ["GORBA EPIC 2026".to_owned()]);
}

#[test]
fn test_backup_run_copies_only_the_main_file() {
    // Arrange
    let directory = tempfile::tempdir().expect("the test directory is created");
    let db_path = directory.path().join("rides.db");
    create_database(&db_path);
    let clock = FakeClock::new(instant(10, 0));

    // Act
    run(&db_path, DEFAULT_KEEP, &clock).expect("the backup runs");

    // Assert
    let names: Vec<String> = std::fs::read_dir(backup_dir_for(&db_path))
        .expect("the backup directory reads")
        .map(|entry| file_name(&entry.expect("the directory entry reads").path()))
        .collect();
    assert_eq!(names, ["rides.20260920-100000-000000.db".to_owned()]);
}

#[test]
fn test_backup_run_leaves_a_non_database_file_alone() {
    // Arrange
    let directory = tempfile::tempdir().expect("the test directory is created");
    let db_path = directory.path().join("rides.db");
    create_database(&db_path);
    let backup_directory = backup_dir_for(&db_path);
    std::fs::create_dir_all(&backup_directory).expect("the backup directory is created");
    let note = backup_directory.join("notes.txt");
    std::fs::write(&note, b"keep me").expect("the note is written");
    let clock = FakeClock::new(instant(10, 0));

    // Act
    run(&db_path, 1, &clock).expect("the backup runs");

    // Assert
    assert!(note.is_file());
    assert_eq!(backup_files(&db_path).len(), 1);
}

#[test]
fn test_backup_run_prunes_at_keep_plus_one() {
    // Arrange
    let directory = tempfile::tempdir().expect("the test directory is created");
    let db_path = directory.path().join("rides.db");
    create_database(&db_path);
    let clock = FakeClock::new(instant(10, 0));
    let rotation = Rotation {
        db_path: &db_path,
        clock: &clock,
        keep: 20,
    };

    // Act
    rotation.write(21);

    // Assert
    let files = backup_files(&db_path);
    assert_eq!(files.len(), 20);
    assert_eq!(
        files.first().map(|path| file_name(path)),
        Some("rides.20260920-102100-000000.db".to_owned())
    );
    assert_eq!(
        files.last().map(|path| file_name(path)),
        Some("rides.20260920-100200-000000.db".to_owned())
    );
}

#[rstest]
#[case(19)]
#[case(20)]
fn test_backup_run_keeps_everything_below_the_cap(#[case] keep: usize) {
    // Arrange
    let directory = tempfile::tempdir().expect("the test directory is created");
    let db_path = directory.path().join("rides.db");
    create_database(&db_path);
    let clock = FakeClock::new(instant(10, 0));
    let rotation = Rotation {
        db_path: &db_path,
        clock: &clock,
        keep,
    };

    // Act
    rotation.write(5);

    // Assert
    assert_eq!(backup_files(&db_path).len(), 5);
}

#[test]
fn test_backup_run_keep_one_keeps_only_the_newest() {
    // Arrange
    let directory = tempfile::tempdir().expect("the test directory is created");
    let db_path = directory.path().join("rides.db");
    create_database(&db_path);
    let clock = FakeClock::new(instant(10, 0));

    // Act
    let first = run(&db_path, 1, &clock).expect("the first backup runs");
    clock.advance(Duration::minutes(1));
    let second = run(&db_path, 1, &clock).expect("the second backup runs");

    // Assert
    assert_eq!(backup_files(&db_path), [second]);
    assert!(!first.exists());
}

#[test]
fn test_backup_run_keep_zero_returns_keep_out_of_range() {
    // Arrange
    let directory = tempfile::tempdir().expect("the test directory is created");
    let db_path = directory.path().join("rides.db");
    create_database(&db_path);
    let clock = FakeClock::new(instant(10, 0));

    // Act
    let result = run(&db_path, 0, &clock);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::KeepOutOfRange { keep: 0 })
    ));
}

#[test]
fn test_backup_run_missing_database_returns_missing_database() {
    // Arrange
    let directory = tempfile::tempdir().expect("the test directory is created");
    let db_path = directory.path().join("never-created.db");
    let clock = FakeClock::new(instant(10, 0));

    // Act
    let result = run(&db_path, DEFAULT_KEEP, &clock);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::MissingDatabase { path }) if path.ends_with("never-created.db")
    ));
}

#[test]
fn test_backup_run_a_busy_checkpoint_writes_nothing_and_returns_busy() {
    // Arrange
    let directory = tempfile::tempdir().expect("the test directory is created");
    let db_path = directory.path().join("rides.db");
    create_database(&db_path);
    let holder = Connection::open(&db_path).expect("the holder connection opens");
    holder
        .execute_batch("BEGIN IMMEDIATE; CREATE TABLE held (x INTEGER);")
        .expect("the holder takes the write lock");
    let clock = FakeClock::new(instant(10, 0));

    // Act
    let result = run(&db_path, DEFAULT_KEEP, &clock);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::CheckpointBusy { frames }) if frames > 0
    ));
    assert_eq!(backup_files(&db_path).len(), 0);
    drop(holder);
}

#[rstest]
#[case(1, 1)]
#[case(3, 1)]
#[case(5, 3)]
#[case(5, 19)]
#[case(4, 8)]
#[case(21, 20)]
fn test_backup_run_rotation_keeps_the_newest_min_runs_keep(
    #[case] runs: usize,
    #[case] keep: usize,
) {
    // Arrange
    let directory = tempfile::tempdir().expect("the test directory is created");
    let db_path = directory.path().join("rides.db");
    create_database(&db_path);
    let clock = FakeClock::new(instant(10, 0));
    let rotation = Rotation {
        db_path: &db_path,
        clock: &clock,
        keep,
    };

    // Act
    let written = rotation.write(runs);

    // Assert
    let kept = runs.min(keep);
    let mut expected: Vec<PathBuf> = written.into_iter().skip(runs - kept).collect();
    expected.reverse();
    assert_eq!(backup_files(&db_path), expected);
}

// --------------------------------------------------------------- restore

#[test]
fn test_backup_restore_round_trips_the_ride() {
    // Arrange
    let directory = tempfile::tempdir().expect("the test directory is created");
    let db_path = directory.path().join("rides.db");
    create_database(&db_path);
    let clock = FakeClock::new(instant(10, 0));
    let backup_path = run(&db_path, DEFAULT_KEEP, &clock).expect("the backup runs");
    let conn = Connection::open(&db_path).expect("the live database opens");
    conn.execute("DELETE FROM ride", [])
        .expect("the ride deletes");
    drop(conn);

    // Act
    restore(&backup_path, &db_path).expect("the backup restores");

    // Assert
    assert_eq!(ride_names(&db_path), ["GORBA EPIC 2026".to_owned()]);
}

#[test]
fn test_backup_restore_missing_source_returns_missing_backup_source() {
    // Arrange
    let directory = tempfile::tempdir().expect("the test directory is created");
    let db_path = directory.path().join("rides.db");
    create_database(&db_path);
    let missing = directory
        .path()
        .join("rides.db.backups")
        .join("nope.20260920-000000-000000.db");

    // Act
    let result = restore(&missing, &db_path);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::MissingBackupSource { path }) if path.ends_with("nope.20260920-000000-000000.db")
    ));
}

#[test]
fn test_backup_restore_clears_the_wal_and_shm_sidecars() {
    // Arrange
    let directory = tempfile::tempdir().expect("the test directory is created");
    let db_path = directory.path().join("rides.db");
    create_database(&db_path);
    let clock = FakeClock::new(instant(10, 0));
    let backup_path = run(&db_path, DEFAULT_KEEP, &clock).expect("the backup runs");
    let wal = directory.path().join("rides.db-wal");
    let shm = directory.path().join("rides.db-shm");
    std::fs::write(&wal, b"stale").expect("the stale wal is planted");
    std::fs::write(&shm, b"stale").expect("the stale shm is planted");

    // Act
    restore(&backup_path, &db_path).expect("the backup restores");

    // Assert
    assert!(!wal.exists());
    assert!(!shm.exists());
}

// -------------------------------------------------------- schedule_hourly

#[test]
fn test_backup_schedule_hourly_first_tick_seeds_without_running() {
    // Arrange
    let clock = FakeClock::new(instant(10, 0));
    let calls = Rc::new(Cell::new(0));
    let counter = Rc::clone(&calls);
    let mut scheduler = schedule_hourly(
        move || {
            counter.set(counter.get() + 1);
            Ok(PathBuf::from("unused"))
        },
        clock,
    );

    // Act
    let ran = scheduler.tick().expect("the tick succeeds");

    // Assert
    assert!(!ran);
    assert_eq!(calls.get(), 0);
}

#[test]
fn test_backup_schedule_hourly_same_hour_does_not_run() {
    // Arrange
    let clock = FakeClock::new(instant(10, 0));
    let calls = Rc::new(Cell::new(0));
    let counter = Rc::clone(&calls);
    let mut scheduler = schedule_hourly(
        move || {
            counter.set(counter.get() + 1);
            Ok(PathBuf::from("unused"))
        },
        clock.clone(),
    );
    scheduler.tick().expect("the seeding tick succeeds");
    clock.set(instant(10, 59));

    // Act
    let ran = scheduler.tick().expect("the same-hour tick succeeds");

    // Assert
    assert!(!ran);
    assert_eq!(calls.get(), 0);
}

#[test]
fn test_backup_schedule_hourly_hour_boundary_runs_once() {
    // Arrange
    let clock = FakeClock::new(instant(10, 0));
    let calls = Rc::new(Cell::new(0));
    let counter = Rc::clone(&calls);
    let mut scheduler = schedule_hourly(
        move || {
            counter.set(counter.get() + 1);
            Ok(PathBuf::from("unused"))
        },
        clock.clone(),
    );
    scheduler.tick().expect("the seeding tick succeeds");
    clock.set(instant(11, 0));

    // Act
    let ran = scheduler.tick().expect("the boundary tick succeeds");
    clock.set(instant(11, 59));
    let again = scheduler.tick().expect("the same-hour tick succeeds");

    // Assert
    assert!(ran);
    assert!(!again);
    assert_eq!(calls.get(), 1);
}

#[test]
fn test_backup_schedule_hourly_two_boundaries_run_twice() {
    // Arrange
    let directory = tempfile::tempdir().expect("the test directory is created");
    let db_path = directory.path().join("rides.db");
    create_database(&db_path);
    let clock = FakeClock::new(instant(10, 0));
    let runner_path = db_path.clone();
    let runner_clock = clock.clone();
    let mut scheduler = schedule_hourly(
        move || run(&runner_path, DEFAULT_KEEP, &runner_clock),
        clock.clone(),
    );
    scheduler.tick().expect("the seeding tick succeeds");
    clock.set(instant(11, 0));
    scheduler.tick().expect("the first boundary tick succeeds");
    clock.set(instant(12, 0));

    // Act
    scheduler.tick().expect("the second boundary tick succeeds");

    // Assert
    assert_eq!(backup_files(&db_path).len(), 2);
}

#[test]
fn test_backup_schedule_hourly_backward_clock_does_not_run() {
    // Arrange
    let clock = FakeClock::new(instant(11, 0));
    let calls = Rc::new(Cell::new(0));
    let counter = Rc::clone(&calls);
    let mut scheduler = schedule_hourly(
        move || {
            counter.set(counter.get() + 1);
            Ok(PathBuf::from("unused"))
        },
        clock.clone(),
    );
    scheduler.tick().expect("the seeding tick succeeds");
    clock.set(instant(10, 0));

    // Act
    let ran = scheduler.tick().expect("the backward tick succeeds");

    // Assert
    assert!(!ran);
    assert_eq!(calls.get(), 0);
}

#[test]
fn test_backup_schedule_hourly_runner_error_is_returned_after_advancing() {
    // Arrange
    let clock = FakeClock::new(instant(10, 0));
    let calls = Rc::new(Cell::new(0));
    let counter = Rc::clone(&calls);
    let mut scheduler = schedule_hourly(
        move || {
            counter.set(counter.get() + 1);
            Err(StoreError::KeepOutOfRange { keep: 0 })
        },
        clock.clone(),
    );
    scheduler.tick().expect("the seeding tick succeeds");
    clock.set(instant(11, 0));

    // Act
    let result = scheduler.tick();

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::KeepOutOfRange { keep: 0 })
    ));
    assert_eq!(calls.get(), 1);
    let again = scheduler.tick().expect("the next same-hour tick succeeds");
    assert!(!again);
    assert_eq!(calls.get(), 1);
}
