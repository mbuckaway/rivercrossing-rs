// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the console's plate-entry and lifecycle decisions.

use pretty_assertions::assert_eq;
use rivercrossing_ride::{
    EntryMode, EventAction, PlateModel, RideConfig, RideConfigParams, RideEngine, RideStatus,
    SystemClock, Timestamp,
};
use rivercrossing_roster::roster::Roster;
use rivercrossing_roster::types::Rider;
use rivercrossing_store::Store;
use rstest::rstest;
use serde_json::json;

use super::{
    ConsoleGates, Cue, RIDE_STARTED, console_gates, finish, miss_entered, plate_entered, reopen,
    start, stop, undo,
};

/// The ride name every fixture uses.
const RIDE_NAME: &str = "GORBA EPIC 2026";

/// The solo plate every fixture uses.
const PLATE: &str = "101";

/// The seed every fixture ride stores.
const SEED: u64 = 7;

/// A lifecycle state that the gate table walks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// A new ride that has not started.
    Draft,
    /// A live running ride.
    Running,
    /// A running ride the operator stopped.
    RunningStopped,
    /// A finished ride.
    Finished,
    /// A finished ride reopened for corrections.
    Reopened,
}

/// A console action that the stop/reopen table walks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operation {
    /// Stop the ride.
    Stop,
    /// Reopen the ride.
    Reopen,
}

// ------------------------------------------------------------- fixtures

/// Return a valid fixture config with the given short-lap policy.
fn fixture_config(min_lap_s: i32, hold_short_laps: bool) -> Result<RideConfig, String> {
    RideConfig::new(RideConfigParams {
        name: RIDE_NAME.to_owned(),
        venue: "Sea to Sky Gondola".to_owned(),
        lap_km: 8.0,
        organizer: "GORBA".to_owned(),
        scorer: "K. Singh".to_owned(),
        entry_mode: EntryMode::Mixed,
        plate_model: PlateModel::RiderPooled,
        min_lap_s,
        hold_short_laps,
        ..RideConfigParams::default()
    })
    .map_err(|error| error.to_string())
}

/// Return a roster holding one solo entry on [`PLATE`].
fn solo_roster() -> Result<Roster, String> {
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(SEED))
        .map_err(|error| error.to_string())?;
    roster
        .create_solo_entry(
            Rider {
                first_name: "Ada".to_owned(),
                last_name: "Lovelace".to_owned(),
                ..Rider::default()
            },
            PLATE,
        )
        .map_err(|error| error.to_string())?;
    Ok(roster)
}

/// Return an empty roster, so the start gate blocks.
fn empty_roster() -> Result<Roster, String> {
    Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(SEED))
        .map_err(|error| error.to_string())
}

/// Build a DRAFT engine from a real temporary store over *config* and *roster*.
fn draft_engine(
    config: &RideConfig,
    roster: &Roster,
) -> Result<RideEngine<Roster, SystemClock>, String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let mut store =
        Store::open(directory.path().join("rides.db")).map_err(|error| error.to_string())?;
    let ride_id = store
        .create_ride(config, Some(SEED))
        .map_err(|error| error.to_string())?;
    store
        .save_roster(ride_id, roster)
        .map_err(|error| error.to_string())?;
    store
        .load_engine(ride_id)
        .map_err(|error| error.to_string())
}

/// Build a RUNNING engine over one solo entry.
fn started_engine(
    min_lap_s: i32,
    hold_short_laps: bool,
) -> Result<(RideEngine<Roster, SystemClock>, Roster), String> {
    let roster = solo_roster()?;
    let config = fixture_config(min_lap_s, hold_short_laps)?;
    let mut engine = draft_engine(&config, &roster)?;
    engine.start(None).map_err(|error| error.to_string())?;
    Ok((engine, roster))
}

/// Build a started engine holding *crossings* recorded laps.
fn engine_in(
    phase: Phase,
    crossings: u32,
) -> Result<(RideEngine<Roster, SystemClock>, Roster), String> {
    let (mut engine, roster) = started_engine(1, false)?;
    let start = engine.actual_start().ok_or("the ride never started")?;
    for lap in 0..crossings {
        let at = shifted(start, 100 * (i64::from(lap) + 1))?;
        let _ = engine.record_crossing(PLATE, Some(at));
    }
    let pair = match phase {
        Phase::Draft => None,
        Phase::Running => Some((engine, roster)),
        Phase::RunningStopped => {
            engine.stop().map_err(|error| error.to_string())?;
            Some((engine, roster))
        }
        Phase::Finished => {
            engine.finish(&[]).map_err(|error| error.to_string())?;
            Some((engine, roster))
        }
        Phase::Reopened => {
            engine.finish(&[]).map_err(|error| error.to_string())?;
            engine.reopen().map_err(|error| error.to_string())?;
            Some((engine, roster))
        }
    };
    pair.ok_or_else(|| "a started engine is never draft".to_owned())
}

/// Build a DRAFT engine over one solo entry.
fn draft_pair() -> Result<(RideEngine<Roster, SystemClock>, Roster), String> {
    let roster = solo_roster()?;
    let config = fixture_config(1, false)?;
    let engine = draft_engine(&config, &roster)?;
    Ok((engine, roster))
}

/// Return *start* moved by *seconds*, negative values included.
fn shifted(start: Timestamp, seconds: i64) -> Result<Timestamp, String> {
    Timestamp::from_unix_timestamp(start.unix_timestamp() + seconds)
        .map_err(|error| error.to_string())
}

// --------------------------------------------------------- plate entry

#[test]
fn test_plate_entered_given_a_known_plate_records_and_buzzes() -> Result<(), String> {
    // Arrange: a back-dated lap keeps the typed lap above the one-second
    // minimum, so this entry records rather than flags.
    let (mut engine, _roster) = started_engine(1, false)?;
    let start = engine.actual_start().ok_or("the ride never started")?;
    let _ = engine.record_crossing(PLATE, Some(shifted(start, -200)?));

    // Act
    let outcome = plate_entered(&mut engine, PLATE);

    // Assert
    assert_eq!(
        outcome,
        super::EntryOutcome {
            notice: Some("Recorded plate 101".to_owned()),
            cue: Some(Cue::Recorded),
            keep_field: false,
            cleared: true,
        }
    );
    assert_eq!(engine.crossings().len(), 2);
    Ok(())
}

#[test]
fn test_plate_entered_given_an_unknown_plate_keeps_the_field_and_buzzes() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = started_engine(1, false)?;

    // Act
    let outcome = plate_entered(&mut engine, "999");

    // Assert
    assert_eq!(
        outcome,
        super::EntryOutcome {
            notice: Some("Unknown plate 999".to_owned()),
            cue: Some(Cue::Error),
            keep_field: true,
            cleared: false,
        }
    );
    assert_eq!(engine.crossings().len(), 0);
    Ok(())
}

#[test]
fn test_plate_entered_given_a_stopped_ride_refuses_and_keeps_the_field() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = started_engine(1, false)?;
    engine.stop().map_err(|error| error.to_string())?;

    // Act
    let outcome = plate_entered(&mut engine, PLATE);

    // Assert
    assert_eq!(outcome.notice.as_deref(), Some("The ride is stopped"));
    assert_eq!(outcome.cue, Some(Cue::Error));
    assert_eq!(outcome.keep_field, true);
    Ok(())
}

#[test]
fn test_plate_entered_given_a_draft_ride_refuses_and_keeps_the_field() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = draft_pair()?;

    // Act
    let outcome = plate_entered(&mut engine, PLATE);

    // Assert
    assert_eq!(outcome.notice.as_deref(), Some("The ride is not running"));
    assert_eq!(outcome.cue, Some(Cue::Error));
    assert_eq!(outcome.keep_field, true);
    Ok(())
}

#[rstest]
#[case("")]
#[case("   ")]
fn test_plate_entered_given_blank_input_is_silent_and_refocuses(
    #[case] text: &str,
) -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = started_engine(1, false)?;

    // Act
    let outcome = plate_entered(&mut engine, text);

    // Assert
    assert_eq!(outcome.notice, None);
    assert_eq!(outcome.cue, None);
    assert_eq!(outcome.keep_field, false);
    assert_eq!(outcome.cleared, false);
    assert_eq!(engine.crossings().len(), 0);
    Ok(())
}

#[test]
fn test_plate_entered_given_a_short_lap_flags_and_buzzes() -> Result<(), String> {
    // Arrange: the first typed lap lands at the gun, under the 60 s minimum.
    let (mut engine, _roster) = started_engine(60, true)?;

    // Act
    let outcome = plate_entered(&mut engine, PLATE);

    // Assert
    assert_eq!(
        outcome.notice.as_deref(),
        Some("Recorded plate 101 — short lap flagged for review")
    );
    assert_eq!(outcome.cue, Some(Cue::Flagged));
    assert_eq!(outcome.cleared, true);
    Ok(())
}

// ------------------------------------------------------------- misses

#[test]
fn test_miss_entered_logs_the_miss() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = started_engine(1, false)?;

    // Act
    let outcome = miss_entered(&mut engine, "-");

    // Assert
    assert_eq!(
        outcome,
        super::EntryOutcome {
            notice: Some("Plate missed — logged for later entry".to_owned()),
            cue: Some(Cue::Error),
            keep_field: false,
            cleared: true,
        }
    );
    assert_eq!(engine.pending_misses().len(), 1);
    let event = engine.events().last().ok_or("no audit event")?;
    assert_eq!(event.action, EventAction::RecordMiss);
    assert_eq!(event.payload.get("reason"), Some(&json!("missed number")));
    Ok(())
}

#[test]
fn test_miss_entered_given_a_draft_ride_refuses_and_keeps_the_field() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = draft_pair()?;

    // Act
    let outcome = miss_entered(&mut engine, "-");

    // Assert
    assert_eq!(
        outcome.notice.as_deref(),
        Some("Miss not recorded: cannot record miss from draft")
    );
    assert_eq!(outcome.cue, Some(Cue::Error));
    assert_eq!(outcome.keep_field, true);
    assert_eq!(engine.pending_misses().len(), 0);
    Ok(())
}

#[test]
fn test_plate_entered_given_a_miss_symbol_routes_to_the_miss_flow() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = started_engine(1, false)?;

    // Act
    let outcome = plate_entered(&mut engine, "=");

    // Assert
    assert_eq!(
        outcome.notice.as_deref(),
        Some("Plate missed — logged for later entry")
    );
    assert_eq!(engine.pending_misses().len(), 1);
    assert_eq!(engine.crossings().len(), 0);
    Ok(())
}

// ------------------------------------------------------ console gates

#[rstest]
#[case(
    Phase::Running,
    0,
    ConsoleGates {
        start: false,
        stop: true,
        undo: false,
    }
)]
#[case(
    Phase::Running,
    1,
    ConsoleGates {
        start: false,
        stop: true,
        undo: true,
    }
)]
#[case(
    Phase::RunningStopped,
    1,
    ConsoleGates {
        start: true,
        stop: false,
        undo: true,
    }
)]
#[case(
    Phase::Finished,
    1,
    ConsoleGates {
        start: false,
        stop: false,
        undo: false,
    }
)]
#[case(
    Phase::Reopened,
    1,
    ConsoleGates {
        start: true,
        stop: false,
        undo: false,
    }
)]
fn test_undo_enabled_only_when_running_with_a_crossing(
    #[case] phase: Phase,
    #[case] crossings: u32,
    #[case] expected: ConsoleGates,
) -> Result<(), String> {
    // Arrange
    let (engine, _roster) = engine_in(phase, crossings)?;

    // Act
    let gates = console_gates(&engine);

    // Assert
    assert_eq!(gates, expected);
    Ok(())
}

#[test]
fn test_console_gates_on_a_draft_ride_offer_start_alone() -> Result<(), String> {
    // Arrange
    let (engine, _roster) = draft_pair()?;

    // Act
    let gates = console_gates(&engine);

    // Assert
    assert_eq!(
        gates,
        ConsoleGates {
            start: true,
            stop: false,
            undo: false,
        }
    );
    Ok(())
}

// --------------------------------------------------------------- undo

#[test]
fn test_undo_removes_the_last_crossing() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = engine_in(Phase::Running, 1)?;

    // Act
    let outcome = undo(&mut engine);

    // Assert
    assert_eq!(outcome.notice.as_deref(), Some("Last crossing undone"));
    assert_eq!(outcome.cue, None);
    assert_eq!(engine.crossings().len(), 0);
    Ok(())
}

#[test]
fn test_undo_given_nothing_to_undo_only_notices() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = engine_in(Phase::Running, 0)?;

    // Act
    let outcome = undo(&mut engine);

    // Assert
    assert_eq!(
        outcome.notice.as_deref(),
        Some("Undo unavailable: nothing to undo")
    );
    assert_eq!(outcome.keep_field, false);
    Ok(())
}

// --------------------------------------------------------- start ride

#[test]
fn test_start_reports_the_blocked_reasons() -> Result<(), String> {
    // Arrange: an empty roster blocks the DRAFT start.
    let roster = empty_roster()?;
    let config = fixture_config(1, false)?;
    let mut engine = draft_engine(&config, &roster)?;

    // Act
    let result = start(&mut engine);

    // Assert
    assert_eq!(result, Ok(Some(vec!["roster has no riders".to_owned()])));
    assert_eq!(engine.state(), RideStatus::Draft);
    Ok(())
}

#[test]
fn test_start_reports_success_with_the_started_notice() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = draft_pair()?;

    // Act
    let result = start(&mut engine);

    // Assert
    assert_eq!(result, Ok(None));
    assert_eq!(engine.state(), RideStatus::Running);
    assert_eq!(RIDE_STARTED, "Ride started");
    Ok(())
}

#[test]
fn test_start_given_a_finished_ride_reports_the_refusal() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = engine_in(Phase::Finished, 0)?;

    // Act
    let result = start(&mut engine);

    // Assert
    assert_eq!(
        result,
        Err("Cannot start: cannot start from finished".to_owned())
    );
    Ok(())
}

// -------------------------------------------------- stop and reopen

#[rstest]
#[case(
    Operation::Stop,
    Phase::Running,
    Ok("Ride stopped — continue to resume".to_owned())
)]
#[case(
    Operation::Stop,
    Phase::Draft,
    Err("Cannot stop: cannot stop a draft ride".to_owned())
)]
#[case(
    Operation::Reopen,
    Phase::Finished,
    Ok("Ride reopened for corrections".to_owned())
)]
#[case(
    Operation::Reopen,
    Phase::Draft,
    Err("Cannot reopen: cannot reopen from draft".to_owned())
)]
fn test_stop_and_reopen_messages(
    #[case] operation: Operation,
    #[case] phase: Phase,
    #[case] expected: Result<String, String>,
) -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = match phase {
        Phase::Draft => draft_pair()?,
        other => engine_in(other, 0)?,
    };

    // Act
    let result = match operation {
        Operation::Stop => stop(&mut engine),
        Operation::Reopen => reopen(&mut engine),
    };

    // Assert
    assert_eq!(result, expected);
    Ok(())
}

// ------------------------------------------------------------- finish

#[test]
fn test_finish_records_the_failed_checks() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = engine_in(Phase::Running, 1)?;
    let checks = vec!["Whole-field 180×12 timing".to_owned()];

    // Act
    let notice = finish(&mut engine, &checks);

    // Assert
    assert_eq!(
        notice,
        Ok("Ride is finished. Results are available".to_owned())
    );
    assert_eq!(engine.state(), RideStatus::Finished);
    assert_eq!(engine.self_test_unverified(), true);
    let event = engine.events().last().ok_or("no audit event")?;
    assert_eq!(event.action, EventAction::Finish);
    assert_eq!(
        event.payload.get("self_test_failed_checks"),
        Some(&json!(["Whole-field 180×12 timing"]))
    );
    Ok(())
}

#[test]
fn test_finish_given_a_finished_ride_reports_the_refusal() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = engine_in(Phase::Finished, 0)?;

    // Act
    let notice = finish(&mut engine, &[]);

    // Assert
    assert_eq!(
        notice,
        Err("Cannot finish: cannot finish from finished".to_owned())
    );
    Ok(())
}

#[test]
fn test_rejection_notice_given_an_unknown_reason_names_the_plate() {
    // Arrange: a reason the engine does not emit stays honest.
    let plate = "12";
    let reason = "mystery_reason";

    // Act
    let notice = super::rejection_notice(plate, reason);

    // Assert
    assert_eq!(notice, "Plate rejected: mystery_reason");
}
