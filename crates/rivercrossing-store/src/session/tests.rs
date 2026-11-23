// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the session reading and its stored-row conversion.

use pretty_assertions::assert_eq;
use rivercrossing_ride::Timestamp;
use rstest::rstest;
use time::Duration;

use super::{PreviousSession, SessionRow, SessionState, previous_session_from_row};
use crate::error::StoreError;

/// Return the instant *epoch* seconds after 1970-01-01T00:00:00Z.
fn timestamp(epoch: i64) -> Timestamp {
    Timestamp::UNIX_EPOCH + Duration::seconds(epoch)
}

#[rstest]
#[case(SessionState::CleanQuit, "clean_quit")]
#[case(SessionState::Crashed, "crashed")]
#[case(SessionState::RunningAtExit, "running_at_exit")]
fn test_session_state_as_str_matches_each_member(
    #[case] state: SessionState,
    #[case] expected: &str,
) {
    // Act
    let spelling = state.as_str();

    // Assert
    assert_eq!(spelling, expected);
}

#[test]
fn test_session_previous_session_fresh_has_no_ride_or_end() {
    // Act
    let previous = PreviousSession::fresh();

    // Assert
    assert_eq!(
        previous,
        PreviousSession {
            state: SessionState::CleanQuit,
            ride_id: None,
            ended_at: None,
        }
    );
}

#[test]
fn test_session_previous_session_clean_quit_uses_closed_at() {
    // Act
    let previous = previous_session_from_row(SessionRow {
        closed_at: Some(1_000),
        active_ride_id: None,
        heartbeat_at: None,
        opened_at: 500,
    })
    .expect("the row converts");

    // Assert
    assert_eq!(
        previous,
        PreviousSession {
            state: SessionState::CleanQuit,
            ride_id: None,
            ended_at: Some(timestamp(1_000)),
        }
    );
}

#[test]
fn test_session_previous_session_running_at_exit_carries_ride_and_closed_at() {
    // Act
    let previous = previous_session_from_row(SessionRow {
        closed_at: Some(1_000),
        active_ride_id: Some(7),
        heartbeat_at: None,
        opened_at: 500,
    })
    .expect("the row converts");

    // Assert
    assert_eq!(
        previous,
        PreviousSession {
            state: SessionState::RunningAtExit,
            ride_id: Some(7),
            ended_at: Some(timestamp(1_000)),
        }
    );
}

#[test]
fn test_session_previous_session_crash_with_heartbeat_uses_the_heartbeat() {
    // Act
    let previous = previous_session_from_row(SessionRow {
        closed_at: None,
        active_ride_id: Some(7),
        heartbeat_at: Some(900),
        opened_at: 500,
    })
    .expect("the row converts");

    // Assert
    assert_eq!(
        previous,
        PreviousSession {
            state: SessionState::Crashed,
            ride_id: Some(7),
            ended_at: Some(timestamp(900)),
        }
    );
}

#[test]
fn test_session_previous_session_crash_without_heartbeat_uses_opened_at() {
    // Act
    let previous = previous_session_from_row(SessionRow {
        closed_at: None,
        active_ride_id: None,
        heartbeat_at: None,
        opened_at: 500,
    })
    .expect("the row converts");

    // Assert
    assert_eq!(
        previous,
        PreviousSession {
            state: SessionState::Crashed,
            ride_id: None,
            ended_at: Some(timestamp(500)),
        }
    );
}

#[test]
fn test_session_previous_session_rejects_an_out_of_range_closed_at() {
    // Act
    let result = previous_session_from_row(SessionRow {
        closed_at: Some(i64::MAX),
        active_ride_id: None,
        heartbeat_at: None,
        opened_at: 500,
    });

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::InvalidRow {
            table: "app_session",
            column: "closed_at",
            ..
        })
    ));
}

#[test]
fn test_session_previous_session_rejects_an_out_of_range_heartbeat() {
    // Act
    let result = previous_session_from_row(SessionRow {
        closed_at: None,
        active_ride_id: None,
        heartbeat_at: Some(i64::MAX),
        opened_at: 500,
    });

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::InvalidRow {
            table: "app_session",
            column: "heartbeat_at",
            ..
        })
    ));
}

#[test]
fn test_session_previous_session_rejects_an_out_of_range_opened_at() {
    // Act
    let result = previous_session_from_row(SessionRow {
        closed_at: None,
        active_ride_id: None,
        heartbeat_at: None,
        opened_at: i64::MAX,
    });

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::InvalidRow {
            table: "app_session",
            column: "opened_at",
            ..
        })
    ));
}
