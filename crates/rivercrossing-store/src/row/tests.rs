// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the stored ride columns and the library summary row.

use pretty_assertions::assert_eq;
use rivercrossing_ride::RideStatus;
use rstest::rstest;
use time::{Date, Month};

use super::{RideRow, event_date_from_str, live_entries_from_i64, ride_status_from_str};
use crate::error::StoreError;

/// Return the fixture event date, 20 September 2026.
fn event_date() -> Date {
    Date::from_calendar_date(2026, Month::September, 20).expect("the fixture date is valid")
}

#[test]
fn test_row_event_date_reads_iso_text() {
    // Arrange
    let stored = "2026-09-20";

    // Act
    let parsed = event_date_from_str(stored).expect("the ISO date parses");

    // Assert
    assert_eq!(parsed, event_date());
}

#[test]
fn test_row_event_date_rejects_other_text_with_invalid_row() {
    // Arrange
    let stored = "20/09/2026";

    // Act
    let result = event_date_from_str(stored);

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

#[rstest]
#[case("draft", RideStatus::Draft)]
#[case("running", RideStatus::Running)]
#[case("finished", RideStatus::Finished)]
#[case("reopened", RideStatus::Reopened)]
fn test_row_ride_status_reads_each_spelling(#[case] stored: &str, #[case] expected: RideStatus) {
    // Act
    let parsed = ride_status_from_str(stored).expect("the status spelling parses");

    // Assert
    assert_eq!(parsed, expected);
}

#[test]
fn test_row_ride_status_rejects_other_text_with_invalid_row() {
    // Arrange
    let stored = "paused";

    // Act
    let result = ride_status_from_str(stored);

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

#[test]
fn test_row_live_entries_reads_the_count() {
    // Act
    let entries = live_entries_from_i64(3).expect("the count fits u32");

    // Assert
    assert_eq!(entries, 3);
}

#[test]
fn test_row_live_entries_reads_zero() {
    // Act
    let entries = live_entries_from_i64(0).expect("zero fits u32");

    // Assert
    assert_eq!(entries, 0);
}

#[test]
fn test_row_live_entries_rejects_a_negative_count_with_out_of_range() {
    // Act
    let result = live_entries_from_i64(-1);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::OutOfRange {
            field: "entry_count",
            ..
        })
    ));
}

#[test]
fn test_row_live_entries_rejects_a_count_above_u32_with_out_of_range() {
    // Arrange
    let stored = i64::from(u32::MAX) + 1;

    // Act
    let result = live_entries_from_i64(stored);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::OutOfRange {
            field: "entry_count",
            ..
        })
    ));
}

#[test]
fn test_row_ride_row_holds_every_field() {
    // Arrange
    let row = RideRow {
        id: 7,
        name: "GORBA EPIC 2026".to_owned(),
        event_date: event_date(),
        status: RideStatus::Draft,
        entries: 2,
    };

    // Assert
    assert_eq!(
        (
            row.id,
            row.name.as_str(),
            row.event_date,
            row.status,
            row.entries
        ),
        (7, "GORBA EPIC 2026", event_date(), RideStatus::Draft, 2)
    );
}

#[test]
fn test_row_ride_row_clones_equal() {
    // Arrange
    let row = RideRow {
        id: 1,
        name: "Club night".to_owned(),
        event_date: event_date(),
        status: RideStatus::Finished,
        entries: 0,
    };

    // Act
    let copy = row.clone();

    // Assert
    assert_eq!(copy, row);
}
