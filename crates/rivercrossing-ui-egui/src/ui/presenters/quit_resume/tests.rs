// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the quit-confirm and resume-dialog decision cores.

use pretty_assertions::assert_eq;
use rivercrossing_ride::{RideStatus, Timestamp};
use rivercrossing_store::{PreviousSession, SessionState};
use rstest::rstest;
use time::{Date, Month, OffsetDateTime, Time};

use super::{
    EXIT_CONFIRM_CANCEL_LABEL, EXIT_CONFIRM_MESSAGE, EXIT_CONFIRM_OK_LABEL, EXIT_CONFIRM_TITLE,
    LaunchChoice, QuitChoice, QuitDialog, QuitOutcome, continue_message, dialog_for_status,
    launch_choice, outcome_for, replay_message, running_exit_message,
};

// ------------------------------------------------------------- fixtures

/// Return the fixture instant at *hour*:*minute* on the event date.
fn instant(hour: u8, minute: u8) -> Result<Timestamp, String> {
    let date =
        Date::from_calendar_date(2026, Month::September, 20).map_err(|error| error.to_string())?;
    let time = Time::from_hms(hour, minute, 0).map_err(|error| error.to_string())?;
    Ok(OffsetDateTime::new_utc(date, time))
}

/// Build one previous-session record.
fn session(state: SessionState, ride_id: Option<i64>) -> Result<PreviousSession, String> {
    Ok(PreviousSession {
        state,
        ride_id,
        ended_at: Some(instant(12, 41)?),
    })
}

// ---------------------------------------------------- dialog_for_status

#[test]
fn test_quit_dialog_for_status_given_running_returns_the_running_dialog() {
    // Arrange, Act
    let dialog = dialog_for_status(RideStatus::Running, false);

    // Assert
    assert_eq!(dialog, QuitDialog::Running);
}

#[test]
fn test_quit_dialog_for_status_given_a_stopped_running_ride_keeps_the_running_dialog() {
    // Arrange: a stopped ride is still RUNNING, and quitting does not stop it.
    // Act
    let dialog = dialog_for_status(RideStatus::Running, true);

    // Assert
    assert_eq!(dialog, QuitDialog::Running);
}

#[rstest]
#[case(RideStatus::Draft)]
#[case(RideStatus::Finished)]
#[case(RideStatus::Reopened)]
fn test_quit_dialog_for_status_given_any_other_status_returns_the_normal_dialog(
    #[case] status: RideStatus,
) {
    // Arrange, Act
    let dialog = dialog_for_status(status, false);

    // Assert
    assert_eq!(dialog, QuitDialog::Normal);
}

// ------------------------------------------------------------ outcome

#[test]
fn test_quit_outcome_for_given_quit_returns_quit() {
    // Arrange, Act
    let outcome = outcome_for(QuitChoice::Quit);

    // Assert
    assert_eq!(outcome, QuitOutcome::Quit);
}

#[test]
fn test_quit_outcome_for_given_finish_first_returns_finish_first() {
    // Arrange, Act
    let outcome = outcome_for(QuitChoice::FinishFirst);

    // Assert
    assert_eq!(outcome, QuitOutcome::FinishFirst);
}

#[test]
fn test_quit_outcome_for_given_cancel_returns_cancel() {
    // Arrange, Act
    let outcome = outcome_for(QuitChoice::Cancel);

    // Assert
    assert_eq!(outcome, QuitOutcome::Cancel);
}

// ------------------------------------------------------------- messages

#[test]
fn test_quit_running_exit_message_names_the_ride_and_the_wall_clock() {
    // Arrange, Act
    let message = running_exit_message("GORBA EPIC 2026");

    // Assert
    assert_eq!(
        message,
        "GORBA EPIC 2026 is still running. Quitting won't stop the ride — \
         it keeps timing on the wall clock, and you'll be asked to continue when you reopen."
    );
}

#[test]
fn test_quit_running_exit_message_given_two_rides_differs_per_ride() {
    // Arrange, Act
    let first = running_exit_message("GORBA EPIC 2026");
    let second = running_exit_message("Club poker night");

    // Assert
    assert_ne!(first, second);
    assert_eq!(second.starts_with("Club poker night"), true);
    assert_eq!(second.contains("GORBA EPIC 2026"), false);
}

#[test]
fn test_quit_normal_confirm_copy_is_frozen() {
    // Arrange, Act
    let title = EXIT_CONFIRM_TITLE;
    let message = EXIT_CONFIRM_MESSAGE;
    let ok = EXIT_CONFIRM_OK_LABEL;
    let cancel = EXIT_CONFIRM_CANCEL_LABEL;

    // Assert
    assert_eq!(title, "Quit RiverCrossing?");
    assert_eq!(
        message,
        "Are you sure you want to quit? No ride is running."
    );
    assert_eq!(ok, "Quit");
    assert_eq!(cancel, "Cancel");
}

// ---------------------------------------------------- continue_message

#[test]
fn test_quit_continue_message_given_running_at_exit_names_the_ride_and_quit_time()
-> Result<(), String> {
    // Arrange
    let at = instant(12, 41)?;

    // Act
    let message = continue_message("GORBA EPIC 2026", SessionState::RunningAtExit, at);

    // Assert
    assert_eq!(
        message,
        Ok("\"GORBA EPIC 2026\" is still running. You quit at 12:41 — \
             the ride kept running on the wall clock."
            .to_owned())
    );
    Ok(())
}

#[test]
fn test_quit_continue_message_given_crashed_names_the_closed_unexpectedly_time()
-> Result<(), String> {
    // Arrange
    let at = instant(12, 41)?;

    // Act
    let message = continue_message("GORBA EPIC 2026", SessionState::Crashed, at);

    // Assert
    assert_eq!(
        message,
        Ok(
            "\"GORBA EPIC 2026\" is still running. The app closed unexpectedly at 12:41 — \
             the ride kept running on the wall clock."
                .to_owned()
        )
    );
    Ok(())
}

#[test]
fn test_quit_continue_message_given_a_single_digit_hour_zero_pads_the_time() -> Result<(), String> {
    // Arrange
    let at = instant(9, 5)?;

    // Act
    let message = continue_message("Ride", SessionState::RunningAtExit, at);

    // Assert
    assert_eq!(
        message,
        Ok("\"Ride\" is still running. You quit at 09:05 — \
             the ride kept running on the wall clock."
            .to_owned())
    );
    Ok(())
}

#[test]
fn test_quit_continue_message_given_clean_quit_refuses_with_its_state() -> Result<(), String> {
    // Arrange
    let at = instant(12, 41)?;

    // Act
    let message = continue_message("Ride", SessionState::CleanQuit, at);

    // Assert
    assert_eq!(message, Err("no resume copy for clean_quit".to_owned()));
    Ok(())
}

#[test]
fn test_quit_continue_message_given_quit_and_crash_renders_different_copy() -> Result<(), String> {
    // Arrange
    let at = instant(12, 41)?;

    // Act
    let quit = continue_message("Ride", SessionState::RunningAtExit, at);
    let crash = continue_message("Ride", SessionState::Crashed, at);

    // Assert
    assert_ne!(quit, crash);
    assert_eq!(
        quit.map(|text| text.contains("You quit at 12:41")),
        Ok(true)
    );
    assert_eq!(
        crash.map(|text| text.contains("closed unexpectedly at 12:41")),
        Ok(true)
    );
    Ok(())
}

// -------------------------------------------------------- launch_choice

#[rstest]
#[case((SessionState::RunningAtExit, Some(7), true), LaunchChoice::Resume)]
#[case((SessionState::RunningAtExit, Some(7), false), LaunchChoice::Resume)]
#[case((SessionState::RunningAtExit, None, true), LaunchChoice::Resume)]
#[case((SessionState::Crashed, Some(7), true), LaunchChoice::Resume)]
#[case((SessionState::Crashed, Some(7), false), LaunchChoice::Resume)]
#[case((SessionState::Crashed, None, true), LaunchChoice::Library)]
#[case((SessionState::Crashed, None, false), LaunchChoice::Nothing)]
#[case((SessionState::CleanQuit, Some(7), true), LaunchChoice::Library)]
#[case((SessionState::CleanQuit, None, true), LaunchChoice::Library)]
#[case((SessionState::CleanQuit, None, false), LaunchChoice::Nothing)]
fn test_quit_launch_choice_given_a_session_and_rides_returns_the_prompt(
    #[case] input: (SessionState, Option<i64>, bool),
    #[case] expected: LaunchChoice,
) -> Result<(), String> {
    // Arrange
    let (state, ride_id, has_rides) = input;
    let previous = session(state, ride_id)?;

    // Act
    let choice = launch_choice(&previous, has_rides);

    // Assert
    assert_eq!(choice, expected);
    Ok(())
}

// -------------------------------------------------------- replay_message

#[test]
fn test_quit_replay_message_names_the_ride_and_the_replay_error() {
    // Arrange, Act
    let message = replay_message("GORBA EPIC 2026", "no crossing with entry_id abc seq 3");

    // Assert
    assert_eq!(
        message,
        "cannot open ride GORBA EPIC 2026: no crossing with entry_id abc seq 3"
    );
}

#[test]
fn test_quit_replay_message_given_two_rides_differs_per_ride() {
    // Arrange, Act
    let first = replay_message("GORBA EPIC 2026", "a drifted roster");
    let second = replay_message("Club poker night", "a drifted roster");

    // Assert
    assert_ne!(first, second);
    assert_eq!(
        second.starts_with("cannot open ride Club poker night: "),
        true
    );
}
