// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the reconstruction helpers: the payload instant, the
//! lifecycle map, the action spelling and the tie-break order.
//!
//! The helpers that read a database are driven through `Store` in
//! `store/tests.rs`; these tests keep the pure conversions in one place.

use pretty_assertions::assert_eq;
use rivercrossing_ride::{EventAction, RideStatus, TieBreak, Timestamp};
use rstest::rstest;
use serde_json::{Value, json};
use time::{Date, Month, OffsetDateTime, Time};

use super::{action_text, event_timestamp, lifecycle_status, parse_tiebreak_order};
use crate::error::StoreError;

/// Return the fixture instant, 20 September 2026 at 10:02 UTC.
fn crossing_at() -> Timestamp {
    let date = Date::from_calendar_date(2026, Month::September, 20).expect("the date is valid");
    let time = Time::from_hms(10, 2, 0).expect("the time is valid");
    OffsetDateTime::new_utc(date, time)
}

#[test]
fn test_engine_load_event_timestamp_reads_the_actual_start() {
    // Arrange
    let payload = json!({"actual_start": "2026-09-20T10:02:00"});

    // Act
    let found = event_timestamp(&payload);

    // Assert
    assert_eq!(found, Some(crossing_at()));
}

#[test]
fn test_engine_load_event_timestamp_prefers_the_earlier_key() {
    // Arrange
    let payload = json!({
        "actual_start": "2026-09-20T10:02:00",
        "crossed_at": "2026-09-20T11:00:00",
    });

    // Act
    let found = event_timestamp(&payload);

    // Assert
    assert_eq!(found, Some(crossing_at()));
}

#[test]
fn test_engine_load_event_timestamp_reads_the_crossed_at() {
    // Arrange
    let payload = json!({"crossed_at": "2026-09-20T10:02:00"});

    // Act
    let found = event_timestamp(&payload);

    // Assert
    assert_eq!(found, Some(crossing_at()));
}

#[test]
fn test_engine_load_event_timestamp_reads_the_stopped_at() {
    // Arrange
    let payload = json!({"stopped_at": "2026-09-20T10:02:00"});

    // Act
    let found = event_timestamp(&payload);

    // Assert
    assert_eq!(found, Some(crossing_at()));
}

#[test]
fn test_engine_load_event_timestamp_reads_the_finished_at() {
    // Arrange
    let payload = json!({"finished_at": "2026-09-20T10:02:00"});

    // Act
    let found = event_timestamp(&payload);

    // Assert
    assert_eq!(found, Some(crossing_at()));
}

#[test]
fn test_engine_load_event_timestamp_reads_the_reopened_at() {
    // Arrange
    let payload = json!({"reopened_at": "2026-09-20T10:02:00"});

    // Act
    let found = event_timestamp(&payload);

    // Assert
    assert_eq!(found, Some(crossing_at()));
}

#[test]
fn test_engine_load_event_timestamp_reads_an_offset_instant() {
    // Arrange
    let payload = json!({"actual_start": "2026-09-20T12:02:00+02:00"});

    // Act
    let found = event_timestamp(&payload);

    // Assert
    assert_eq!(
        found.map(Timestamp::unix_timestamp),
        Some(crossing_at().unix_timestamp())
    );
}

#[test]
fn test_engine_load_event_timestamp_skips_an_unparsable_key() {
    // Arrange
    let payload = json!({
        "actual_start": "yesterday",
        "crossed_at": "2026-09-20T10:02:00",
    });

    // Act
    let found = event_timestamp(&payload);

    // Assert
    assert_eq!(found, Some(crossing_at()));
}

#[test]
fn test_engine_load_event_timestamp_skips_a_non_string_value() {
    // Arrange
    let payload = json!({"actual_start": 42});

    // Act
    let found = event_timestamp(&payload);

    // Assert
    assert_eq!(found, None);
}

#[test]
fn test_engine_load_event_timestamp_without_a_known_key_returns_none() {
    // Arrange
    let payload = json!({"plate": "12", "reason": "manual"});

    // Act
    let found = event_timestamp(&payload);

    // Assert
    assert_eq!(found, None);
}

#[rstest]
#[case(EventAction::Start, RideStatus::Running)]
#[case(EventAction::Continue, RideStatus::Running)]
#[case(EventAction::Finish, RideStatus::Finished)]
#[case(EventAction::Reopen, RideStatus::Reopened)]
fn test_engine_load_lifecycle_status_maps_the_lifecycle_actions(
    #[case] action: EventAction,
    #[case] expected: RideStatus,
) {
    // Act
    let status = lifecycle_status(action);

    // Assert
    assert_eq!(status, Some(expected));
}

#[rstest]
#[case(EventAction::Stop)]
#[case(EventAction::RecordCrossing)]
#[case(EventAction::ShoeReshuffle)]
fn test_engine_load_lifecycle_status_of_an_audit_only_action_is_none(#[case] action: EventAction) {
    // Act
    let status = lifecycle_status(action);

    // Assert
    assert_eq!(status, None);
}

#[rstest]
#[case(EventAction::Start, "start")]
#[case(EventAction::Continue, "continue")]
#[case(EventAction::RecordCrossing, "record_crossing")]
#[case(EventAction::AssignPlateToMiss, "assign_plate_to_miss")]
#[case(EventAction::ShoeReshuffle, "shoe_reshuffle")]
#[case(EventAction::Reopen, "reopen")]
fn test_engine_load_action_text_returns_the_stored_spelling(
    #[case] action: EventAction,
    #[case] expected: &str,
) {
    // Act
    let text = action_text(action).expect("the action encodes");

    // Assert
    assert_eq!(text, expected);
}

#[test]
fn test_engine_load_tiebreak_order_reads_three_known_spellings() {
    // Arrange
    let stored = "[\"laps\",\"high_card\",\"total_time\"]";

    // Act
    let order = parse_tiebreak_order(7, stored).expect("the order parses");

    // Assert
    assert_eq!(
        order,
        [TieBreak::Laps, TieBreak::HighCard, TieBreak::TotalTime]
    );
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
fn test_engine_load_tiebreak_order_rejects_an_invalid_order(#[case] stored: &str) {
    // Act
    let result = parse_tiebreak_order(7, stored);

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::TiebreakOrder { ride_id: 7, stored: ref text }) if text == stored
    ));
}

#[test]
fn test_engine_load_event_timestamp_reads_a_json_object_without_a_value() {
    // Arrange
    let payload = Value::Object(serde_json::Map::new());

    // Act
    let found = event_timestamp(&payload);

    // Assert
    assert_eq!(found, None);
}
