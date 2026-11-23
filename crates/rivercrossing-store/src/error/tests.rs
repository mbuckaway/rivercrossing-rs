// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the store's error type: one rendering test per variant,
//! plus a transparency check for the three wrapped source errors.

use std::path::PathBuf;

use pretty_assertions::assert_eq;
use rivercrossing_cards::CardsError;
use rivercrossing_ride::RideError;
use rivercrossing_roster::error::RosterError;

use super::StoreError;

#[test]
fn test_error_schema_version_mismatch_renders_the_oracle_message() {
    let error = StoreError::SchemaVersionMismatch {
        current: 4,
        supported: 3,
    };

    assert_eq!(
        error.to_string(),
        "Database schema version 4 does not match this build's schema version 3. \
         Rename or delete the database file to continue."
    );
}

#[test]
fn test_error_unknown_migration_renders_the_version() {
    let error = StoreError::UnknownMigration { version: 0 };

    assert_eq!(error.to_string(), "no schema migration from version 0");
}

#[test]
fn test_error_migration_renders_the_versions_and_the_source() {
    let source = rusqlite::Error::InvalidQuery;
    let expected = format!("schema migration from version 1 to version 2 failed: {source}");

    let rendered = StoreError::Migration {
        from: 1,
        to: 2,
        source,
    }
    .to_string();

    assert_eq!(rendered, expected);
}

#[test]
fn test_error_foreign_keys_renders_the_tables() {
    let error = StoreError::ForeignKeys {
        tables: "rider".to_owned(),
    };

    assert_eq!(
        error.to_string(),
        "v1 -> v2 schema migration left foreign key violations in rider"
    );
}

#[test]
fn test_error_ride_not_found_renders_the_id() {
    let error = StoreError::RideNotFound { id: 7 };

    assert_eq!(error.to_string(), "no ride with id 7");
}

#[test]
fn test_error_ride_name_mismatch_renders_both_names() {
    let error = StoreError::RideNameMismatch {
        id: 7,
        typed: "gorba".to_owned(),
        name: "GORBA EPIC 2026".to_owned(),
    };

    assert_eq!(
        error.to_string(),
        "typed name \"gorba\" does not match ride 7 name \"GORBA EPIC 2026\""
    );
}

#[test]
fn test_error_ride_running_renders_the_id() {
    let error = StoreError::RideRunning { id: 7 };

    assert_eq!(error.to_string(), "ride 7 is RUNNING and cannot be deleted");
}

#[test]
fn test_error_tiebreak_order_renders_the_stored_value() {
    let error = StoreError::TiebreakOrder {
        ride_id: 7,
        stored: "laps,coffee".to_owned(),
    };

    assert_eq!(
        error.to_string(),
        "ride 7 stores a tiebreak order this build does not know: laps,coffee"
    );
}

#[test]
fn test_error_invalid_config_renders_the_source() {
    let error = StoreError::InvalidConfig {
        source: RideError::UnknownPlate {
            plate: "12".to_owned(),
        },
    };

    assert_eq!(error.to_string(), "invalid ride config: unknown plate: 12");
}

#[test]
fn test_error_invalid_shoe_renders_the_source() {
    let error = StoreError::InvalidShoe {
        source: CardsError::ShoeEmpty,
    };

    assert_eq!(error.to_string(), "invalid shoe: shoe is empty");
}

#[test]
fn test_error_out_of_range_renders_the_field_value_and_max() {
    let error = StoreError::OutOfRange {
        field: "deck_count",
        value: 300,
        max: 255,
    };

    assert_eq!(
        error.to_string(),
        "deck_count value 300 exceeds the maximum 255"
    );
}

#[test]
fn test_error_replay_renders_the_row_the_ride_and_the_source() {
    let error = StoreError::Replay {
        row_id: 4,
        ride_id: 1,
        detail: "unknown entry: 12".to_owned(),
    };

    assert_eq!(
        error.to_string(),
        "cannot replay audit row 4 for ride 1: unknown entry: 12"
    );
}

#[test]
fn test_error_replay_renders_a_decoded_payload_failure() {
    let error = StoreError::Replay {
        row_id: 4,
        ride_id: 1,
        detail: "expected value at line 1 column 1".to_owned(),
    };

    assert_eq!(
        error.to_string(),
        "cannot replay audit row 4 for ride 1: expected value at line 1 column 1"
    );
}

#[test]
fn test_error_invalid_roster_renders_the_source() {
    let error = StoreError::InvalidRoster {
        source: RosterError::TeamSize {
            message: "max_team_size must be between 2 and 10, got 1".to_owned(),
        },
    };

    assert_eq!(
        error.to_string(),
        "invalid roster: max_team_size must be between 2 and 10, got 1"
    );
}

#[test]
fn test_error_invalid_payload_renders_the_row() {
    let error = StoreError::InvalidPayload { row_id: 4 };

    assert_eq!(
        error.to_string(),
        "cannot replay audit row 4: the payload is not a JSON object"
    );
}

#[test]
fn test_error_invalid_row_renders_the_table_the_column_and_the_value() {
    let error = StoreError::InvalidRow {
        table: "ride",
        column: "status",
        stored: "paused".to_owned(),
    };

    assert_eq!(
        error.to_string(),
        "ride.status holds a value this build does not know: paused"
    );
}

#[test]
fn test_error_checkpoint_busy_renders_the_oracle_message() {
    let error = StoreError::CheckpointBusy { frames: 12 };

    assert_eq!(
        error.to_string(),
        "wal_checkpoint busy (12 frames not checkpointed); \
         another connection holds a transaction on the database"
    );
}

#[test]
fn test_error_keep_out_of_range_renders_the_oracle_message() {
    let error = StoreError::KeepOutOfRange { keep: 0 };

    assert_eq!(error.to_string(), "keep must be at least 1");
}

#[test]
fn test_error_missing_database_renders_the_path() {
    let error = StoreError::MissingDatabase {
        path: PathBuf::from("/tmp/rides.db"),
    };

    assert_eq!(
        error.to_string(),
        "no database file to back up: /tmp/rides.db"
    );
}

#[test]
fn test_error_missing_backup_source_renders_the_path() {
    let error = StoreError::MissingBackupSource {
        path: PathBuf::from("/tmp/rides.20260101.db"),
    };

    assert_eq!(
        error.to_string(),
        "backup file not found: /tmp/rides.20260101.db"
    );
}

#[test]
fn test_error_sqlite_renders_the_source_message() {
    let source = rusqlite::Error::InvalidQuery;
    let expected = source.to_string();

    assert_eq!(StoreError::Sqlite(source).to_string(), expected);
}

#[test]
fn test_error_io_renders_the_source_message() {
    let source = std::io::Error::other("disk went away");
    let expected = source.to_string();

    assert_eq!(StoreError::Io(source).to_string(), expected);
}

#[test]
fn test_error_json_renders_the_source_message() {
    let source = serde_json::Error::io(std::io::Error::other("truncated"));
    let expected = source.to_string();

    assert_eq!(StoreError::Json(source).to_string(), expected);
}

#[test]
fn test_error_debug_names_the_variant() {
    let error = StoreError::RideNotFound { id: 7 };

    assert!(format!("{error:?}").starts_with("RideNotFound"));
}
