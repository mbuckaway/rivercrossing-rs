// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the correction dialogs' submissions and the Crossing
//! Detail view-model.

use pretty_assertions::assert_eq;
use rivercrossing_ride::{
    Crossing, EntryId, EntryMode, EventAction, PlateModel, RideConfig, RideConfigParams,
    RideEngine, SystemClock, Timestamp,
};
use rivercrossing_roster::roster::{EntryEdit, Roster, TeamFields};
use rivercrossing_roster::types::{Entry, Rider};
use rivercrossing_store::Store;
use rstest::rstest;
use time::{Date, Month, OffsetDateTime, Time};

use super::{
    CorrectionActions, add_crossing_at, assign_plate_to_miss, can_edit, card_text,
    crossing_detail_fields, deal_manual, dnf_target, edit_crossing, mark_dnf, set_start, void_card,
};

/// The ride name every fixture uses.
const RIDE_NAME: &str = "GORBA EPIC 2026";

/// The solo plate every fixture uses.
const SOLO_PLATE: &str = "101";

/// The pooled team member's plate every fixture uses.
const TEAM_PLATE: &str = "201";

/// The seed every fixture ride stores.
const SEED: u64 = 7;

/// The fixture engine type.
type Engine = RideEngine<Roster, SystemClock>;

// ------------------------------------------------------------- fixtures

/// Return the fixture date.
fn event_date() -> Result<Date, String> {
    Date::from_calendar_date(2026, Month::September, 20).map_err(|error| error.to_string())
}

/// Return the fixture instant at *hour*:*minute*:*second*.
fn instant(hour: u8, minute: u8, second: u8) -> Result<Timestamp, String> {
    let time = Time::from_hms(hour, minute, second).map_err(|error| error.to_string())?;
    Ok(OffsetDateTime::new_utc(event_date()?, time))
}

/// Return a valid fixture config with the given short-lap policy.
fn fixture_config(min_lap_s: i32, hold_short_laps: bool) -> Result<RideConfig, String> {
    let date = event_date()?;
    RideConfig::new(RideConfigParams {
        name: RIDE_NAME.to_owned(),
        event_date: date,
        venue: "Sea to Sky Gondola".to_owned(),
        lap_km: 8.0,
        organizer: "GORBA".to_owned(),
        scorer: "K. Singh".to_owned(),
        planned_start: OffsetDateTime::new_utc(
            date,
            Time::from_hms(9, 0, 0).map_err(|e| e.to_string())?,
        ),
        entry_mode: EntryMode::Mixed,
        plate_model: PlateModel::RiderPooled,
        min_lap_s,
        hold_short_laps,
        ..RideConfigParams::default()
    })
    .map_err(|error| error.to_string())
}

/// Return a MIXED rider-pooled roster: one solo entry and one pooled team.
fn fixture_roster() -> Result<Roster, String> {
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(SEED))
        .map_err(|error| error.to_string())?;
    roster
        .create_solo_entry(
            Rider {
                first_name: "Ada".to_owned(),
                last_name: "Lovelace".to_owned(),
                ..Rider::default()
            },
            SOLO_PLATE,
        )
        .map_err(|error| error.to_string())?;
    roster
        .create_team_entry(TeamFields {
            display_name: "Trail Blazers",
            riders: vec![
                Rider {
                    first_name: "Bea".to_owned(),
                    last_name: "Chen".to_owned(),
                    plate: Some(TEAM_PLATE.to_owned()),
                    ..Rider::default()
                },
                Rider {
                    first_name: "Cy".to_owned(),
                    last_name: "Diaz".to_owned(),
                    plate: Some("202".to_owned()),
                    ..Rider::default()
                },
            ],
            ..TeamFields::default()
        })
        .map_err(|error| error.to_string())?;
    Ok(roster)
}

/// Build a DRAFT engine from a real temporary store over *config* and *roster*.
fn draft_engine(config: &RideConfig, roster: &Roster) -> Result<Engine, String> {
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

/// Build a RUNNING engine whose gun is 10:00 on the event date.
fn running_engine(min_lap_s: i32, hold_short_laps: bool) -> Result<(Engine, Roster), String> {
    let config = fixture_config(min_lap_s, hold_short_laps)?;
    let roster = fixture_roster()?;
    let engine = running_engine_over(&config, &roster)?;
    Ok((engine, roster))
}

/// Build a RUNNING engine over *config* and *roster*, its gun at 10:00.
fn running_engine_over(config: &RideConfig, roster: &Roster) -> Result<Engine, String> {
    let mut engine = draft_engine(config, roster)?;
    engine.start(None).map_err(|error| error.to_string())?;
    engine
        .set_start_time(instant(10, 0, 0)?)
        .map_err(|error| error.to_string())?;
    Ok(engine)
}

/// Build a DRAFT engine over the fixture roster.
fn draft_pair() -> Result<(Engine, Roster), String> {
    let config = fixture_config(1, false)?;
    let roster = fixture_roster()?;
    let engine = draft_engine(&config, &roster)?;
    Ok((engine, roster))
}

/// Record one crossing for *plate* at *at* and return it.
fn record(engine: &mut Engine, plate: &str, at: Timestamp) -> Result<Crossing, String> {
    let result = engine.record_crossing(plate, Some(at));
    if !result.accepted {
        return Err(result.reason.unwrap_or_default());
    }
    engine
        .crossings()
        .last()
        .cloned()
        .ok_or_else(|| "the crossing was not recorded".to_owned())
}

// ------------------------------------------------------- edit_crossing

#[test]
fn test_corrections_edit_crossing_given_a_recorded_lap_retimes_it() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(11, 0, 0)?)?;

    // Act
    let outcome = edit_crossing(
        &mut engine,
        &crossing.entry_id,
        crossing.seq,
        "11:30:00",
        "mis-keyed time",
    )?;

    // Assert
    assert_eq!(outcome.notice, "Crossing edited");
    assert_eq!(outcome.event.action, EventAction::EditCrossing);
    assert_eq!(
        outcome
            .event
            .payload
            .get("crossed_at")
            .and_then(|value| value.as_str()),
        Some("2026-09-20T11:30:00")
    );
    assert_eq!(
        engine.crossings().first().map(|item| item.crossed_at),
        Some(instant(11, 30, 0)?)
    );
    Ok(())
}

#[test]
fn test_corrections_edit_crossing_given_an_h_mm_time_accepts_it() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(11, 0, 0)?)?;

    // Act
    let outcome = edit_crossing(
        &mut engine,
        &crossing.entry_id,
        crossing.seq,
        "11:30",
        "mis-keyed time",
    )?;

    // Assert
    assert_eq!(
        outcome
            .event
            .payload
            .get("crossed_at")
            .and_then(|value| value.as_str()),
        Some("2026-09-20T11:30:00")
    );
    Ok(())
}

#[rstest]
#[case("")]
#[case("   ")]
fn test_corrections_edit_crossing_given_a_blank_reason_refuses(
    #[case] reason: &str,
) -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(11, 0, 0)?)?;

    // Act
    let outcome = edit_crossing(
        &mut engine,
        &crossing.entry_id,
        crossing.seq,
        "11:30:00",
        reason,
    );

    // Assert
    assert_eq!(outcome, Err("Enter a reason.".to_owned()));
    Ok(())
}

#[test]
fn test_corrections_edit_crossing_given_a_malformed_time_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(11, 0, 0)?)?;

    // Act
    let outcome = edit_crossing(
        &mut engine,
        &crossing.entry_id,
        crossing.seq,
        "half past",
        "mis-keyed time",
    );

    // Assert
    assert_eq!(
        outcome,
        Err("Time must be H:MM:SS, got 'half past'".to_owned())
    );
    Ok(())
}

#[test]
fn test_corrections_edit_crossing_given_a_blank_time_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(11, 0, 0)?)?;

    // Act
    let outcome = edit_crossing(
        &mut engine,
        &crossing.entry_id,
        crossing.seq,
        "  ",
        "reason",
    );

    // Assert
    assert_eq!(
        outcome,
        Err("Time is empty and must be completed".to_owned())
    );
    Ok(())
}

#[test]
fn test_corrections_edit_crossing_given_a_draft_ride_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, roster) = draft_pair()?;
    let entry = roster
        .resolve_plate(SOLO_PLATE)
        .map(rivercrossing_roster::types::Entry::id)
        .ok_or_else(|| "the fixture roster holds no solo entry".to_owned())?;

    // Act
    let outcome = edit_crossing(&mut engine, &entry, 1, "11:30:00", "mis-keyed time");

    // Assert
    assert_eq!(
        outcome,
        Err("Could not edit crossing: cannot edit crossing from draft".to_owned())
    );
    Ok(())
}

#[test]
fn test_corrections_edit_crossing_given_no_such_lap_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(11, 0, 0)?)?;
    let expected = format!(
        "Could not edit crossing: no crossing with entry_id {} seq 9",
        crossing.entry_id.0
    );

    // Act
    let outcome = edit_crossing(&mut engine, &crossing.entry_id, 9, "11:30:00", "reason");

    // Assert
    assert_eq!(outcome, Err(expected));
    Ok(())
}

#[test]
fn test_corrections_edit_crossing_given_a_time_before_the_predecessor_refuses() -> Result<(), String>
{
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(11, 0, 0)?)?;

    // Act
    let outcome = edit_crossing(
        &mut engine,
        &crossing.entry_id,
        crossing.seq,
        "09:30:00",
        "mis-keyed time",
    );

    // Assert
    assert_eq!(
        outcome.map_err(
            |message| message.starts_with("Could not edit crossing: lap time must be positive")
        ),
        Err(true)
    );
    Ok(())
}

// ---------------------------------------------------- add_crossing_at

#[test]
fn test_corrections_add_crossing_at_given_a_recorded_lap_appends_one() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;
    let _first = record(&mut engine, SOLO_PLATE, instant(11, 0, 0)?)?;

    // Act
    let outcome = add_crossing_at(&mut engine, SOLO_PLATE, "12:00:00", "missed lap");

    // Assert
    assert_eq!(
        outcome.as_ref().map(|item| item.notice.as_str()),
        Ok("Crossing added")
    );
    assert_eq!(
        outcome.as_ref().map(|item| item.event.action),
        Ok(EventAction::AddCrossingAt)
    );
    assert_eq!(engine.crossings().len(), 2);
    Ok(())
}

#[test]
fn test_corrections_add_crossing_at_given_a_blank_reason_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;

    // Act
    let outcome = add_crossing_at(&mut engine, SOLO_PLATE, "12:00:00", " ");

    // Assert
    assert_eq!(outcome, Err("Enter a reason.".to_owned()));
    assert_eq!(engine.crossings().len(), 0);
    Ok(())
}

#[test]
fn test_corrections_add_crossing_at_given_a_malformed_time_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;

    // Act
    let outcome = add_crossing_at(&mut engine, SOLO_PLATE, "12:99:00", "missed lap");

    // Assert
    assert_eq!(
        outcome,
        Err("Time must be H:MM:SS, got '12:99:00'".to_owned())
    );
    Ok(())
}

#[test]
fn test_corrections_add_crossing_at_given_an_unknown_plate_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;

    // Act
    let outcome = add_crossing_at(&mut engine, "999", "12:00:00", "missed lap");

    // Assert
    assert_eq!(
        outcome,
        Err("Correction refused: unknown plate: 999".to_owned())
    );
    Ok(())
}

#[test]
fn test_corrections_add_crossing_at_given_a_draft_ride_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = draft_pair()?;

    // Act
    let outcome = add_crossing_at(&mut engine, SOLO_PLATE, "12:00:00", "missed lap");

    // Assert
    assert_eq!(
        outcome,
        Err("Correction refused: cannot add crossing from draft".to_owned())
    );
    Ok(())
}

#[test]
fn test_corrections_add_crossing_at_given_a_time_before_the_latest_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;
    let _first = record(&mut engine, SOLO_PLATE, instant(12, 0, 0)?)?;

    // Act
    let outcome = add_crossing_at(&mut engine, SOLO_PLATE, "11:00:00", "missed lap");

    // Assert
    assert_eq!(
        outcome.map_err(|message| message.starts_with("Correction refused: lap time must be positive")),
        Err(true)
    );
    Ok(())
}

// ------------------------------------------------------- deal_manual

#[test]
fn test_corrections_deal_manual_given_a_live_ride_deals_one_card() -> Result<(), String> {
    // Arrange
    let (mut engine, roster) = running_engine(1, false)?;
    let entry = solo_entry_id(&roster)?;
    let before = engine.credited_cards(&entry).len();

    // Act
    let outcome = deal_manual(&mut engine, SOLO_PLATE, "bonus card")?;

    // Assert
    assert_eq!(outcome.notice, "Card dealt");
    assert_eq!(outcome.event.action, EventAction::DealManual);
    assert_eq!(engine.credited_cards(&entry).len(), before + 1);
    Ok(())
}

#[test]
fn test_corrections_deal_manual_given_a_blank_reason_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;

    // Act
    let outcome = deal_manual(&mut engine, SOLO_PLATE, "");

    // Assert
    assert_eq!(outcome, Err("Enter a reason.".to_owned()));
    Ok(())
}

#[test]
fn test_corrections_deal_manual_given_an_unknown_plate_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;

    // Act
    let outcome = deal_manual(&mut engine, "999", "bonus card");

    // Assert
    assert_eq!(
        outcome,
        Err("Correction refused: unknown plate: 999".to_owned())
    );
    Ok(())
}

#[test]
fn test_corrections_deal_manual_given_a_draft_ride_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = draft_pair()?;

    // Act
    let outcome = deal_manual(&mut engine, SOLO_PLATE, "bonus card");

    // Assert
    assert_eq!(
        outcome,
        Err("Correction refused: cannot deal manually from draft".to_owned())
    );
    Ok(())
}

// --------------------------------------------------------- mark_dnf

#[test]
fn test_corrections_mark_dnf_given_a_solo_plate_names_the_rider() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;

    // Act
    let outcome = mark_dnf(&mut engine, SOLO_PLATE, "mechanical failure")?;

    // Assert
    assert_eq!(outcome.notice, "101 · Ada Lovelace — DNF");
    assert_eq!(outcome.event.action, EventAction::Dnf);
    Ok(())
}

#[test]
fn test_corrections_mark_dnf_given_a_pooled_member_plate_names_the_rider() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;

    // Act
    let outcome = mark_dnf(&mut engine, TEAM_PLATE, "mechanical failure")?;

    // Assert
    assert_eq!(outcome.notice, "201 · Bea Chen — DNF");
    assert_eq!(
        outcome
            .event
            .payload
            .get("rider")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    Ok(())
}

#[rstest]
#[case("")]
#[case("   ")]
fn test_corrections_mark_dnf_given_a_blank_plate_refuses(
    #[case] plate: &str,
) -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;

    // Act
    let outcome = mark_dnf(&mut engine, plate, "mechanical failure");

    // Assert
    assert_eq!(outcome, Err("Enter the rider plate.".to_owned()));
    Ok(())
}

#[rstest]
#[case("20A")]
#[case("²")]
fn test_corrections_mark_dnf_given_a_non_digit_plate_refuses(
    #[case] plate: &str,
) -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;

    // Act
    let outcome = mark_dnf(&mut engine, plate, "mechanical failure");

    // Assert
    assert_eq!(outcome, Err("The plate must be digits only.".to_owned()));
    Ok(())
}

#[test]
fn test_corrections_mark_dnf_given_a_blank_reason_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;

    // Act
    let outcome = mark_dnf(&mut engine, SOLO_PLATE, "  ");

    // Assert
    assert_eq!(outcome, Err("Enter a reason for the DNF.".to_owned()));
    Ok(())
}

#[test]
fn test_corrections_mark_dnf_given_an_unknown_plate_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;

    // Act
    let outcome = mark_dnf(&mut engine, "999", "mechanical failure");

    // Assert
    assert_eq!(
        outcome,
        Err("Correction refused: unknown plate: 999".to_owned())
    );
    Ok(())
}

#[test]
fn test_corrections_mark_dnf_given_a_draft_ride_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = draft_pair()?;

    // Act
    let outcome = mark_dnf(&mut engine, SOLO_PLATE, "mechanical failure");

    // Assert
    assert_eq!(
        outcome,
        Err("Correction refused: cannot mark DNF from draft".to_owned())
    );
    Ok(())
}

// ----------------------------------------------------------- void_card

#[test]
fn test_corrections_void_card_given_a_credited_card_retires_it() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(11, 0, 0)?)?;
    let card = engine
        .card_for(&crossing)
        .ok_or("the crossing dealt no card")?;

    // Act
    let outcome = void_card(&mut engine, SOLO_PLATE, &card, "wrong card")?;

    // Assert
    assert_eq!(outcome.notice, "Card voided");
    assert_eq!(outcome.event.action, EventAction::VoidCard);
    assert_eq!(engine.is_card_voided(&crossing), true);
    Ok(())
}

#[test]
fn test_corrections_void_card_given_a_blank_reason_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(11, 0, 0)?)?;
    let card = engine
        .card_for(&crossing)
        .ok_or("the crossing dealt no card")?;

    // Act
    let outcome = void_card(&mut engine, SOLO_PLATE, &card, "");

    // Assert
    assert_eq!(outcome, Err("Enter a reason.".to_owned()));
    assert_eq!(engine.is_card_voided(&crossing), false);
    Ok(())
}

#[test]
fn test_corrections_void_card_given_a_held_card_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(60, true)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(10, 0, 30)?)?;
    let held = engine
        .held_card_for(&crossing)
        .ok_or("the short lap held no card")?;

    // Act
    let outcome = void_card(&mut engine, SOLO_PLATE, &held, "wrong card");

    // Assert
    assert_eq!(
        outcome,
        Err(
            "Could not void card: card is held; confirm or void it through the review panel"
                .to_owned()
        )
    );
    Ok(())
}

#[test]
fn test_corrections_void_card_given_an_unknown_entry_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(11, 0, 0)?)?;
    let card = engine
        .card_for(&crossing)
        .ok_or("the crossing dealt no card")?;

    // Act
    let outcome = void_card(&mut engine, "999", &card, "wrong card");

    // Assert
    assert_eq!(
        outcome,
        Err("Could not void card: unknown plate: 999".to_owned())
    );
    Ok(())
}

// ----------------------------------------------------------- set_start

#[test]
fn test_corrections_set_start_given_a_running_ride_back_dates_the_gun() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;
    let moved = instant(9, 30, 0)?;

    // Act
    let outcome = set_start(&mut engine, "2026-09-20", "09:30:00")?;

    // Assert
    assert_eq!(outcome.notice, "Start time set");
    assert_eq!(outcome.event.action, EventAction::SetStartTime);
    assert_eq!(engine.actual_start(), Some(moved));
    Ok(())
}

#[test]
fn test_corrections_set_start_given_an_h_mm_time_accepts_it() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;

    // Act
    let outcome = set_start(&mut engine, "2026-09-20", "09:30")?;

    // Assert
    assert_eq!(engine.actual_start(), Some(instant(9, 30, 0)?));
    assert_eq!(
        outcome
            .event
            .payload
            .get("actual_start")
            .and_then(|value| value.as_str()),
        Some("2026-09-20T09:30:00")
    );
    Ok(())
}

#[rstest]
#[case("", "09:30:00", "Date is empty and must be completed")]
#[case("20-09-2026", "09:30:00", "Date must be YYYY-MM-DD, got '20-09-2026'")]
#[case("2026-09-20", "", "Time is empty and must be completed")]
#[case("2026-09-20", "09", "Time must be H:MM:SS, got '09'")]
#[case("2026-09-20", "09:30:xx", "Time must be H:MM:SS, got '09:30:xx'")]
#[case("2026-09-20", "1:2:3:4", "Time must be H:MM:SS, got '1:2:3:4'")]
fn test_corrections_set_start_given_a_malformed_field_refuses(
    #[case] date: &str,
    #[case] time: &str,
    #[case] expected: &str,
) -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;

    // Act
    let outcome = set_start(&mut engine, date, time);

    // Assert
    assert_eq!(outcome, Err(expected.to_owned()));
    Ok(())
}

#[test]
fn test_corrections_set_start_given_a_draft_ride_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = draft_pair()?;

    // Act
    let outcome = set_start(&mut engine, "2026-09-20", "09:30:00");

    // Assert
    assert_eq!(
        outcome,
        Err("Correction refused: cannot set start time from draft".to_owned())
    );
    Ok(())
}

#[test]
fn test_corrections_set_start_given_a_finished_ride_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;
    engine.finish(&[]).map_err(|error| error.to_string())?;

    // Act
    let outcome = set_start(&mut engine, "2026-09-20", "09:30:00");

    // Assert
    assert_eq!(
        outcome,
        Err("Correction refused: cannot set start time from finished".to_owned())
    );
    Ok(())
}

// ----------------------------------------------- assign_plate_to_miss

#[test]
fn test_corrections_assign_plate_to_miss_scores_the_miss() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;
    let miss = engine
        .record_miss(instant(11, 0, 0)?, "missed number")
        .map_err(|error| error.to_string())?;
    let miss_seq = miss
        .payload
        .get("miss_seq")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| "the miss event named no miss".to_owned())?;

    // Act
    let outcome = assign_plate_to_miss(&mut engine, miss_seq, "202", "scored from the sheet")?;

    // Assert
    assert_eq!(outcome.notice, "Miss scored");
    assert_eq!(outcome.event.action, EventAction::AssignPlateToMiss);
    assert_eq!(engine.pending_misses().len(), 0);
    assert_eq!(engine.crossings().len(), 1);
    Ok(())
}

#[test]
fn test_corrections_assign_plate_to_miss_given_a_blank_plate_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;

    // Act
    let outcome = assign_plate_to_miss(&mut engine, 1, "  ", "scored from the sheet");

    // Assert
    assert_eq!(
        outcome,
        Err("Enter a plate to assign to this miss.".to_owned())
    );
    Ok(())
}

#[test]
fn test_corrections_assign_plate_to_miss_given_a_blank_reason_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;

    // Act
    let outcome = assign_plate_to_miss(&mut engine, 1, SOLO_PLATE, "");

    // Assert
    assert_eq!(outcome, Err("Enter a reason.".to_owned()));
    Ok(())
}

#[test]
fn test_corrections_assign_plate_to_miss_given_no_such_miss_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;

    // Act
    let outcome = assign_plate_to_miss(&mut engine, 9, SOLO_PLATE, "scored from the sheet");

    // Assert
    assert_eq!(
        outcome,
        Err("Could not assign: no pending miss with miss_seq 9".to_owned())
    );
    Ok(())
}

#[test]
fn test_corrections_assign_plate_to_miss_given_an_unknown_plate_refuses() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;
    let _miss = engine
        .record_miss(instant(11, 0, 0)?, "missed number")
        .map_err(|error| error.to_string())?;

    // Act
    let outcome = assign_plate_to_miss(&mut engine, 1, "999", "scored from the sheet");

    // Assert
    assert_eq!(
        outcome,
        Err("Could not assign: unknown plate: 999".to_owned())
    );
    Ok(())
}

// --------------------------------------------------- crossing_detail

#[test]
fn test_corrections_crossing_detail_fields_given_a_solo_lap_renders_every_box() -> Result<(), String>
{
    // Arrange
    let (mut engine, roster) = running_engine(1, false)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(10, 1, 40)?)?;

    // Act
    let model = crossing_detail_fields(&crossing, &engine, &roster);

    // Assert
    assert_eq!(model.rider, "Ada Lovelace");
    assert_eq!(model.team, "solo");
    assert_eq!(model.plate, SOLO_PLATE);
    assert_eq!(model.lap, "1");
    assert_eq!(model.time, "10:01:40");
    assert_eq!(model.lap_time, "1:40");
    assert_eq!(model.total, "0:01:40");
    assert_eq!(
        model.card,
        card_text(&engine.card_for(&crossing).ok_or("no card")?.code())
    );
    assert_eq!(model.held, "Credited");
    Ok(())
}

#[test]
fn test_corrections_crossing_detail_fields_given_a_pooled_member_names_rider_and_team()
-> Result<(), String> {
    // Arrange
    let (mut engine, roster) = running_engine(1, false)?;
    let crossing = record(&mut engine, TEAM_PLATE, instant(10, 1, 40)?)?;

    // Act
    let model = crossing_detail_fields(&crossing, &engine, &roster);

    // Assert
    assert_eq!(model.rider, "Bea Chen");
    assert_eq!(model.team, "Trail Blazers");
    assert_eq!(model.plate, TEAM_PLATE);
    Ok(())
}

#[test]
fn test_corrections_crossing_detail_fields_given_a_second_lap_sums_the_total() -> Result<(), String>
{
    // Arrange
    let (mut engine, roster) = running_engine(1, false)?;
    let _first = record(&mut engine, SOLO_PLATE, instant(10, 1, 40)?)?;
    let second = record(&mut engine, SOLO_PLATE, instant(10, 3, 20)?)?;

    // Act
    let model = crossing_detail_fields(&second, &engine, &roster);

    // Assert
    assert_eq!(model.lap, "2");
    assert_eq!(model.lap_time, "1:40");
    assert_eq!(model.total, "0:03:20");
    Ok(())
}

#[test]
fn test_corrections_crossing_detail_fields_given_a_short_lap_reads_held() -> Result<(), String> {
    // Arrange
    let (mut engine, roster) = running_engine(60, true)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(10, 0, 30)?)?;
    let held = engine.held_card_for(&crossing).ok_or("no held card")?;

    // Act
    let model = crossing_detail_fields(&crossing, &engine, &roster);

    // Assert
    assert_eq!(model.card, card_text(&held.code()));
    assert_eq!(model.held, "Held - Review");
    Ok(())
}

#[test]
fn test_corrections_crossing_detail_fields_given_a_voided_card_reads_void() -> Result<(), String> {
    // Arrange
    let (mut engine, roster) = running_engine(1, false)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(10, 1, 40)?)?;
    let card = engine.card_for(&crossing).ok_or("no card")?;
    let _voided = void_card(&mut engine, SOLO_PLATE, &card, "wrong card")?;

    // Act
    let model = crossing_detail_fields(&crossing, &engine, &roster);

    // Assert
    assert_eq!(model.card, "Void");
    assert_eq!(model.held, "Void");
    Ok(())
}

#[test]
fn test_corrections_crossing_detail_fields_given_a_duplicate_pair_reads_duplicate()
-> Result<(), String> {
    // Arrange
    let (mut engine, roster) = running_engine(1, false)?;
    let _first = record(&mut engine, SOLO_PLATE, instant(10, 1, 40)?)?;
    let second = record(&mut engine, SOLO_PLATE, instant(10, 1, 40)?)?;

    // Act
    let model = crossing_detail_fields(&second, &engine, &roster);

    // Assert
    assert_eq!(model.held, "Duplicate");
    assert_eq!(
        model.card,
        card_text(&engine.card_for(&second).ok_or("no card")?.code())
    );
    Ok(())
}

#[test]
fn test_corrections_crossing_detail_fields_given_an_entry_out_of_the_roster_falls_back()
-> Result<(), String> {
    // Arrange
    let (engine, roster) = running_engine(1, false)?;
    let orphan = Crossing {
        entry_id: EntryId("missing".to_owned()),
        seq: 1,
        crossed_at: instant(11, 0, 0)?,
        rider_plate: Some("999".to_owned()),
        deal_seq: 999,
    };

    // Act
    let model = crossing_detail_fields(&orphan, &engine, &roster);

    // Assert
    assert_eq!(model.rider, "999");
    assert_eq!(model.team, "999");
    assert_eq!(model.plate, "999");
    assert_eq!(model.lap_time, "0:00");
    assert_eq!(model.total, "0:00:00");
    assert_eq!(model.card, "Void");
    assert_eq!(model.held, "Void");
    Ok(())
}

#[test]
fn test_corrections_crossing_detail_fields_given_a_stale_lap_renders_zero_times()
-> Result<(), String> {
    // Arrange
    let (mut engine, roster) = running_engine(1, false)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(11, 0, 0)?)?;
    let stale = Crossing { seq: 9, ..crossing };

    // Act
    let model = crossing_detail_fields(&stale, &engine, &roster);

    // Assert
    assert_eq!(model.lap_time, "0:00");
    assert_eq!(model.total, "0:00:00");
    Ok(())
}

#[test]
fn test_corrections_crossing_detail_fields_given_a_zero_lap_renders_zero_times()
-> Result<(), String> {
    // Arrange
    let (mut engine, roster) = running_engine(1, false)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(11, 0, 0)?)?;
    let zero = Crossing { seq: 0, ..crossing };

    // Act
    let model = crossing_detail_fields(&zero, &engine, &roster);

    // Assert
    assert_eq!(model.lap, "0");
    assert_eq!(model.lap_time, "0:00");
    assert_eq!(model.total, "0:00:00");
    Ok(())
}

#[test]
fn test_corrections_crossing_detail_fields_given_an_hour_lap_renders_h_mm_ss() -> Result<(), String>
{
    // Arrange: a lap at the gun plus exactly one hour.
    let (mut engine, roster) = running_engine(1, false)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(11, 0, 0)?)?;

    // Act
    let model = crossing_detail_fields(&crossing, &engine, &roster);

    // Assert
    assert_eq!(model.lap_time, "1:00:00");
    assert_eq!(model.total, "1:00:00");
    Ok(())
}

#[test]
fn test_corrections_crossing_detail_fields_given_no_typed_plate_renders_the_entry_plate()
-> Result<(), String> {
    // Arrange
    let (mut engine, roster) = running_engine(1, false)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(10, 1, 40)?)?;
    let untyped = Crossing {
        rider_plate: None,
        ..crossing
    };

    // Act
    let model = crossing_detail_fields(&untyped, &engine, &roster);

    // Assert
    assert_eq!(model.plate, SOLO_PLATE);
    assert_eq!(model.rider, "Ada Lovelace");
    Ok(())
}

#[test]
fn test_corrections_crossing_detail_fields_given_a_blank_rider_name_falls_back_to_the_entry()
-> Result<(), String> {
    // Arrange: a solo rider with no names, whose entry is renamed.
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(SEED))
        .map_err(|error| error.to_string())?;
    roster
        .create_solo_entry(Rider::default(), "303")
        .map_err(|error| error.to_string())?;
    let entry = roster
        .resolve_plate("303")
        .map(Entry::id)
        .ok_or_else(|| "the fixture roster holds no solo entry".to_owned())?;
    roster
        .update_entry(
            &entry,
            EntryEdit {
                display_name: Some("Mystery"),
                ..EntryEdit::default()
            },
        )
        .map_err(|error| error.to_string())?;
    let config = fixture_config(1, false)?;
    let mut engine = running_engine_over(&config, &roster)?;
    let crossing = record(&mut engine, "303", instant(10, 1, 40)?)?;

    // Act
    let model = crossing_detail_fields(&crossing, &engine, &roster);

    // Assert
    assert_eq!(model.rider, "Mystery");
    Ok(())
}

// ------------------------------------------------------------- can_edit

#[test]
fn test_corrections_can_edit_given_a_running_credited_crossing_offers_every_action()
-> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(11, 0, 0)?)?;

    // Act
    let actions = can_edit(&engine, &crossing);

    // Assert
    assert_eq!(
        actions,
        CorrectionActions {
            edit: true,
            edit_time: true,
            void_card: true,
            delete: true,
        }
    );
    Ok(())
}

#[test]
fn test_corrections_can_edit_given_a_held_card_disables_only_void_card() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(60, true)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(10, 0, 30)?)?;

    // Act
    let actions = can_edit(&engine, &crossing);

    // Assert
    assert_eq!(actions.void_card, false);
    assert_eq!(actions.edit, true);
    assert_eq!(actions.edit_time, true);
    assert_eq!(actions.delete, true);
    Ok(())
}

#[test]
fn test_corrections_can_edit_given_a_held_duplicate_offers_void_card() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(60, true)?;
    let _first = record(&mut engine, SOLO_PLATE, instant(10, 0, 30)?)?;
    let second = record(&mut engine, SOLO_PLATE, instant(10, 0, 30)?)?;

    // Act
    let actions = can_edit(&engine, &second);

    // Assert
    assert_eq!(actions.void_card, true);
    Ok(())
}

#[test]
fn test_corrections_can_edit_given_a_finished_ride_disables_delete() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(11, 0, 0)?)?;
    engine.finish(&[]).map_err(|error| error.to_string())?;

    // Act
    let actions = can_edit(&engine, &crossing);

    // Assert
    assert_eq!(actions.delete, false);
    assert_eq!(actions.edit, true);
    assert_eq!(actions.void_card, true);
    Ok(())
}

#[test]
fn test_corrections_can_edit_given_a_reopened_ride_offers_delete() -> Result<(), String> {
    // Arrange
    let (mut engine, _roster) = running_engine(1, false)?;
    let crossing = record(&mut engine, SOLO_PLATE, instant(11, 0, 0)?)?;
    engine.finish(&[]).map_err(|error| error.to_string())?;
    engine.reopen().map_err(|error| error.to_string())?;

    // Act
    let actions = can_edit(&engine, &crossing);

    // Assert
    assert_eq!(actions.delete, true);
    Ok(())
}

#[test]
fn test_corrections_can_edit_given_an_orphan_crossing_disables_void_card() -> Result<(), String> {
    // Arrange
    let (engine, _roster) = running_engine(1, false)?;
    let orphan = Crossing {
        entry_id: EntryId("missing".to_owned()),
        seq: 1,
        crossed_at: instant(11, 0, 0)?,
        rider_plate: Some("999".to_owned()),
        deal_seq: 999,
    };

    // Act
    let actions = can_edit(&engine, &orphan);

    // Assert
    assert_eq!(actions.void_card, false);
    assert_eq!(actions.edit, true);
    assert_eq!(actions.delete, true);
    Ok(())
}

// ------------------------------------------------------------ card_text

#[rstest]
#[case("KS", "K♠")]
#[case("AH", "A♥")]
#[case("10D", "10♦")]
#[case("QC", "Q♣")]
#[case("JK", "JK★")]
#[case("X", "X")]
#[case("", "")]
fn test_corrections_card_text_given_a_code_renders_its_glyph(
    #[case] code: &str,
    #[case] expected: &str,
) {
    // Arrange, Act
    let text = card_text(code);

    // Assert
    assert_eq!(text, expected);
}

// -------------------------------------------------------------- helpers

/// Return the fixture solo entry's stable id.
fn solo_entry_id(roster: &Roster) -> Result<EntryId, String> {
    roster
        .resolve_plate(SOLO_PLATE)
        .map(rivercrossing_roster::types::Entry::id)
        .ok_or_else(|| "the fixture roster holds no solo entry".to_owned())
}

#[rstest]
#[case::blank_entry_is_the_plate("101", "", "101")]
#[case::sole_entry_plate_marks_it("101", "", "101")]
#[case::team_name_resolves_to_its_plate("", "Trail Blazers", "T1")]
#[case::rider_name_resolves_to_the_rider_plate("", "Sam Ellis", "201")]
#[case::typed_plate_wins_when_unresolved("", "999", "999")]
fn test_dnf_target_resolves_the_entry_the_operator_typed(
    #[case] plate: &str,
    #[case] entry: &str,
    #[case] expected: &str,
) -> Result<(), String> {
    // Arrange: a solo entry and a pooled team with a team plate.
    let roster = dnf_roster()?;

    // Act
    let target = dnf_target(&roster, plate, entry);

    // Assert
    assert_eq!(target, expected);
    Ok(())
}

/// Return a roster with a solo entry and a team whose plate is `T1`.
fn dnf_roster() -> Result<Roster, String> {
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(SEED))
        .map_err(|error| error.to_string())?;
    roster.load_entries(vec![
        Entry::restored(
            "k-ada",
            SOLO_PLATE,
            "Ada Lovelace",
            rivercrossing_roster::enums::EntryType::Solo,
            vec![Rider {
                first_name: "Ada".to_owned(),
                last_name: "Lovelace".to_owned(),
                plate: Some(SOLO_PLATE.to_owned()),
                ..Rider::default()
            }],
        ),
        Entry::restored(
            "k-trail",
            "T1",
            "Trail Blazers",
            rivercrossing_roster::enums::EntryType::Team,
            vec![Rider {
                first_name: "Sam".to_owned(),
                last_name: "Ellis".to_owned(),
                plate: Some(TEAM_PLATE.to_owned()),
                ..Rider::default()
            }],
        ),
    ]);
    Ok(roster)
}

#[rstest]
#[case::entry_plate_names_it("T1", "T1")]
#[case::entry_key_names_it("k-trail", "T1")]
#[case::entry_name_names_it("Trail Blazers", "T1")]
fn test_dnf_target_resolves_a_team_by_any_of_its_names(
    #[case] typed: &str,
    #[case] expected: &str,
) -> Result<(), String> {
    // Arrange
    let roster = dnf_roster()?;

    // Act
    let target = dnf_target(&roster, "", typed);

    // Assert
    assert_eq!(target, expected);
    Ok(())
}

#[test]
fn test_dnf_target_given_a_relay_rider_without_a_plate_names_the_entry() -> Result<(), String> {
    // Arrange: a relay team's members carry no plates of their own.
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::TeamRelay, Some(SEED))
        .map_err(|error| error.to_string())?;
    roster.load_entries(vec![Entry::restored(
        "k-relay",
        "T2",
        "Moss Ridge Riders",
        rivercrossing_roster::enums::EntryType::Team,
        vec![Rider {
            first_name: "Sam".to_owned(),
            last_name: "Ellis".to_owned(),
            ..Rider::default()
        }],
    )]);

    // Act
    let target = dnf_target(&roster, "", "Sam Ellis");

    // Assert
    assert_eq!(target, "T2");
    Ok(())
}
