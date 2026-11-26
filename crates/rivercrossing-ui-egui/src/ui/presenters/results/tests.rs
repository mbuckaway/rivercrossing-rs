// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the results window's standings model and stale flag.
//!
//! Every test builds a real ride engine through a temporary store, so the
//! ranking, the draw column, the DNS tail and the stale flag all run over the
//! same snapshot the console shows.

#![expect(
    clippy::float_cmp,
    reason = "the asserted readings are whole seconds that the engine copies unchanged"
)]

use pretty_assertions::assert_eq;
use rivercrossing_ride::{
    EntryId, EntryMode, Event, EventAction, PlateModel, RideConfig, RideConfigParams, RideEngine,
    RideStatus, SnapshotKind, SystemClock, TieBreak, Timestamp,
};
use rivercrossing_roster::enums::EntryType;
use rivercrossing_roster::roster::{Roster, TeamFields};
use rivercrossing_roster::types::Rider;
use rivercrossing_store::Store;
use rstest::rstest;
use serde_json::json;

use super::{CORRECTION_ACTIONS, ExportWatermark, has_correction_since, standings_model};

/// The ride name every fixture uses.
const RIDE_NAME: &str = "GORBA EPIC 2026";

/// The seed every fixture ride stores.
const SEED: u64 = 7;

/// The default tie-break order.
const ORDER: [TieBreak; 3] = [TieBreak::HighCard, TieBreak::Laps, TieBreak::TotalTime];

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

/// Return one solo rider carrying *plate*.
fn solo_rider(first: &str, plate: &str) -> Rider {
    Rider {
        first_name: first.to_owned(),
        last_name: "Rider".to_owned(),
        plate: Some(plate.to_owned()),
        ..Rider::default()
    }
}

/// Return a mixed roster: one two-rider team and two solo entries.
fn mixed_roster() -> Result<Roster, String> {
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(SEED))
        .map_err(|error| error.to_string())?;
    roster
        .create_team_entry(TeamFields {
            display_name: "Trail Blazers",
            riders: vec![solo_rider("Ada", "30"), solo_rider("Grace", "31")],
            plate: None,
            logo_card: None,
        })
        .map_err(|error| error.to_string())?;
    for (first, plate) in [("Sam", "10"), ("Robin", "11")] {
        roster
            .create_solo_entry(solo_rider(first, plate), plate)
            .map_err(|error| error.to_string())?;
    }
    Ok(roster)
}

/// Return a roster holding two solo entries on plates `10` and `11`.
fn two_solo_roster() -> Result<Roster, String> {
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(SEED))
        .map_err(|error| error.to_string())?;
    for (first, plate) in [("Sam", "10"), ("Robin", "11")] {
        roster
            .create_solo_entry(solo_rider(first, plate), plate)
            .map_err(|error| error.to_string())?;
    }
    Ok(roster)
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

/// Return a FINISHED engine with one stored tie-break draw, then reload it.
///
/// Both solo entries ride one short lap, whose card the hold policy keeps, so
/// the two hands tie on no credited card. The draw rows name each entry's card
/// in roster order, so a test can hand the ranking a card the venue draw never
/// deals.
fn finished_engine_with_manual_draw(
    roster: &Roster,
    cards: [&str; 2],
) -> Result<RideEngine<Roster, SystemClock>, String> {
    let config = fixture_config(1_080, true)?;
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let mut store =
        Store::open(directory.path().join("rides.db")).map_err(|error| error.to_string())?;
    let ride_id = store
        .create_ride(&config, Some(SEED))
        .map_err(|error| error.to_string())?;
    store
        .save_roster(ride_id, roster)
        .map_err(|error| error.to_string())?;
    let mut engine = store
        .load_engine(ride_id)
        .map_err(|error| error.to_string())?;
    start(&mut engine)?;
    lap(&mut engine, "10", 60)?;
    lap(&mut engine, "11", 60)?;
    engine.finish(&[]).map_err(|error| error.to_string())?;
    for event in engine.events().to_vec() {
        store
            .append(ride_id, &event)
            .map_err(|error| error.to_string())?;
    }
    let draws: Vec<serde_json::Value> = roster
        .entries()
        .iter()
        .zip(cards)
        .map(|(entry, card)| json!({"entry_id": entry.key.clone(), "card": card}))
        .collect();
    store
        .append(
            ride_id,
            &Event {
                action: EventAction::TiebreakDraw,
                payload: json!({"draws": draws}),
            },
        )
        .map_err(|error| error.to_string())?;
    store
        .load_engine(ride_id)
        .map_err(|error| error.to_string())
}

/// The fixed gun every fixture ride starts from, in whole seconds.
const GUN_EPOCH: i64 = 1_790_000_000;

/// Start *engine* at the fixed gun and return that gun.
fn start(engine: &mut RideEngine<Roster, SystemClock>) -> Result<Timestamp, String> {
    let gun = Timestamp::from_unix_timestamp(GUN_EPOCH).map_err(|error| error.to_string())?;
    engine.start(Some(gun)).map_err(|error| error.to_string())?;
    Ok(gun)
}

/// Return *gun* moved by *seconds*.
fn shifted(gun: Timestamp, seconds: i64) -> Result<Timestamp, String> {
    Timestamp::from_unix_timestamp(gun.unix_timestamp() + seconds)
        .map_err(|error| error.to_string())
}

/// Record one lap for *plate* at `gun + seconds`.
fn lap(
    engine: &mut RideEngine<Roster, SystemClock>,
    plate: &str,
    seconds: i64,
) -> Result<(), String> {
    let gun = engine
        .actual_start()
        .ok_or_else(|| "the ride has no start instant".to_owned())?;
    let at = shifted(gun, seconds)?;
    let _crossing = engine.record_crossing(plate, Some(at));
    Ok(())
}

/// Return the rows' plates, in order.
fn plates(rows: &[super::StandingsRowModel]) -> Vec<&str> {
    rows.iter().map(|row| row.plate.as_str()).collect()
}

// ----------------------------------------------------- teams and solo

#[test]
fn test_standings_model_given_a_mixed_ride_splits_teams_and_solo() -> Result<(), String> {
    // Arrange
    let roster = mixed_roster()?;
    let mut engine = draft_engine(&fixture_config(1, false)?, &roster)?;
    start(&mut engine)?;
    lap(&mut engine, "30", 60)?;
    lap(&mut engine, "10", 60)?;

    // Act
    let model = standings_model(&engine, ORDER, true, None)?;

    // Assert
    assert_eq!(model.teams.len(), 1);
    assert_eq!(model.solo.len(), 2);
    let kinds: Vec<SnapshotKind> = model
        .teams
        .iter()
        .chain(model.solo.iter())
        .map(|row| row.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![SnapshotKind::Team, SnapshotKind::Solo, SnapshotKind::Solo]
    );
    Ok(())
}

#[test]
fn test_standings_model_given_a_mixed_ride_numbers_each_section_from_one() -> Result<(), String> {
    // Arrange
    let roster = mixed_roster()?;
    let mut engine = draft_engine(&fixture_config(1, false)?, &roster)?;
    start(&mut engine)?;
    lap(&mut engine, "30", 60)?;
    lap(&mut engine, "10", 60)?;
    lap(&mut engine, "10", 120)?;
    lap(&mut engine, "11", 60)?;

    // Act
    let model = standings_model(&engine, ORDER, true, None)?;

    // Assert
    let places: Vec<Option<u32>> = model.solo.iter().map(|row| row.place).collect();
    assert_eq!(places, vec![Some(1), Some(2)]);
    assert_eq!(plates(&model.solo), vec!["10", "11"]);
    assert_eq!(model.teams.first().map(|row| row.place), Some(Some(1)));
    Ok(())
}

#[test]
fn test_standings_model_given_a_live_ride_ranks_by_laps_before_the_stored_order()
-> Result<(), String> {
    // Arrange: the stored order puts total time first, yet a live board keeps
    // the live order, so the two-lap rider leads.
    let roster = mixed_roster()?;
    let mut engine = draft_engine(&fixture_config(1, false)?, &roster)?;
    start(&mut engine)?;
    lap(&mut engine, "11", 60)?;
    lap(&mut engine, "10", 600)?;
    lap(&mut engine, "10", 1200)?;
    let stored = [TieBreak::TotalTime, TieBreak::Laps, TieBreak::HighCard];

    // Act
    let model = standings_model(&engine, stored, true, None)?;

    // Assert
    assert_eq!(plates(&model.solo).first().copied(), Some("10"));
    assert_eq!(model.solo.first().map(|row| row.laps), Some(2));
    Ok(())
}

// ---------------------------------------------------------------- DNS

#[test]
fn test_standings_model_given_a_finished_ride_lists_a_zero_lap_entry_as_dns() -> Result<(), String>
{
    // Arrange
    let roster = two_solo_roster()?;
    let mut engine = draft_engine(&fixture_config(1, false)?, &roster)?;
    start(&mut engine)?;
    lap(&mut engine, "10", 60)?;
    engine.finish(&[]).map_err(|error| error.to_string())?;

    // Act
    let model = standings_model(&engine, ORDER, true, None)?;

    // Assert
    assert_eq!(plates(&model.solo), vec!["10", "11"]);
    assert_eq!(model.solo.last().map(|row| row.place), Some(None));
    Ok(())
}

#[test]
fn test_standings_model_given_a_finished_ride_hides_the_dns_row_when_the_toggle_is_off()
-> Result<(), String> {
    // Arrange
    let roster = two_solo_roster()?;
    let mut engine = draft_engine(&fixture_config(1, false)?, &roster)?;
    start(&mut engine)?;
    lap(&mut engine, "10", 60)?;
    engine.finish(&[]).map_err(|error| error.to_string())?;

    // Act
    let model = standings_model(&engine, ORDER, false, None)?;

    // Assert
    assert_eq!(plates(&model.solo), vec!["10"]);
    Ok(())
}

#[test]
fn test_standings_model_given_a_live_ride_places_its_zero_lap_entry() -> Result<(), String> {
    // Arrange
    let roster = two_solo_roster()?;
    let mut engine = draft_engine(&fixture_config(1, false)?, &roster)?;
    start(&mut engine)?;
    lap(&mut engine, "10", 60)?;

    // Act
    let model = standings_model(&engine, ORDER, false, None)?;

    // Assert
    assert_eq!(model.solo.len(), 2);
    assert_eq!(model.solo.last().map(|row| row.place), Some(Some(2)));
    Ok(())
}

// ---------------------------------------------------- draw and blank

#[test]
fn test_standings_model_given_a_drawn_tie_reads_the_card_and_places_by_it() -> Result<(), String> {
    // Arrange: two equal empty hands, drawn the ace of spades and the three of
    // diamonds; the ace's draw key is the higher one.
    let roster = two_solo_roster()?;
    let engine = finished_engine_with_manual_draw(&roster, ["AS", "3D"])?;

    // Act
    let model = standings_model(&engine, ORDER, true, None)?;

    // Assert
    let draws: Vec<&str> = model.solo.iter().map(|row| row.draw.as_str()).collect();
    assert_eq!(draws, vec!["AS", "3D"]);
    assert_eq!(model.solo.first().map(|row| row.place), Some(Some(1)));
    Ok(())
}

#[test]
fn test_standings_model_given_a_dns_row_leaves_the_draw_blank() -> Result<(), String> {
    // Arrange: both zero-lap entries tie at the finish, so both draw a card,
    // yet a did-not-start row must never show one.
    let roster = two_solo_roster()?;
    let mut engine = draft_engine(&fixture_config(1, false)?, &roster)?;
    start(&mut engine)?;
    engine.finish(&[]).map_err(|error| error.to_string())?;

    // Act
    let model = standings_model(&engine, ORDER, true, None)?;

    // Assert
    let draws: Vec<&str> = model.solo.iter().map(|row| row.draw.as_str()).collect();
    assert_eq!(draws, vec!["", ""]);
    Ok(())
}

#[test]
fn test_standings_model_given_a_no_card_entry_leaves_the_hand_blank() -> Result<(), String> {
    // Arrange
    let roster = two_solo_roster()?;
    let mut engine = draft_engine(&fixture_config(1, false)?, &roster)?;
    start(&mut engine)?;

    // Act
    let model = standings_model(&engine, ORDER, true, None)?;

    // Assert
    assert_eq!(
        model.solo.first().map(|row| row.hand.clone()),
        Some(String::new())
    );
    Ok(())
}

#[test]
fn test_standings_model_given_recorded_laps_reads_the_hand_and_the_best_lap() -> Result<(), String>
{
    // Arrange
    let roster = two_solo_roster()?;
    let mut engine = draft_engine(&fixture_config(1, false)?, &roster)?;
    start(&mut engine)?;
    lap(&mut engine, "10", 60)?;
    lap(&mut engine, "10", 150)?;

    // Act
    let model = standings_model(&engine, ORDER, true, None)?;

    // Assert
    let row = model
        .solo
        .iter()
        .find(|row| row.plate == "10")
        .ok_or_else(|| "no row for plate 10".to_owned())?;
    assert_ne!(row.hand.as_str(), "");
    assert_eq!(row.best_lap, 60.0);
    assert_eq!(row.total_time, 150.0);
    Ok(())
}

#[test]
fn test_standings_model_given_a_solo_and_a_team_reads_the_sex_of_the_solo_only()
-> Result<(), String> {
    // Arrange
    let roster = mixed_roster()?;
    let mut engine = draft_engine(&fixture_config(1, false)?, &roster)?;
    start(&mut engine)?;
    let solo_plate = roster
        .entries()
        .iter()
        .find(|entry| entry.kind == EntryType::Solo)
        .map(|entry| entry.plate.clone())
        .ok_or_else(|| "no solo entry".to_owned())?;
    lap(&mut engine, &solo_plate, 60)?;

    // Act
    let model = standings_model(&engine, ORDER, true, None)?;

    // Assert
    assert_eq!(model.solo.first().and_then(|row| row.sex.clone()), None);
    assert_eq!(model.teams.first().and_then(|row| row.sex.clone()), None);
    Ok(())
}

// ------------------------------------------------------ stale flag

#[test]
fn test_standings_model_given_no_export_watermark_is_never_stale() -> Result<(), String> {
    // Arrange
    let roster = mixed_roster()?;
    let mut engine = draft_engine(&fixture_config(1, false)?, &roster)?;
    start(&mut engine)?;

    // Act
    let model = standings_model(&engine, ORDER, true, None)?;

    // Assert
    assert!(!model.stale);
    Ok(())
}

#[test]
fn test_standings_model_given_a_correction_after_the_watermark_is_stale() -> Result<(), String> {
    // Arrange
    let roster = two_solo_roster()?;
    let mut engine = draft_engine(&fixture_config(1, false)?, &roster)?;
    start(&mut engine)?;
    lap(&mut engine, "10", 60)?;
    let watermark = engine.events().len();
    let entry = roster
        .entries()
        .iter()
        .find(|entry| entry.plate == "10")
        .ok_or_else(|| "no entry".to_owned())?;
    let gun = engine
        .actual_start()
        .ok_or_else(|| "the ride never started".to_owned())?;
    engine
        .edit_crossing(&EntryId(entry.key.clone()), 1, shifted(gun, 90)?, "mis-key")
        .map_err(|error| error.to_string())?;

    // Act
    let model = standings_model(&engine, ORDER, true, Some(watermark))?;

    // Assert
    assert!(model.stale);
    Ok(())
}

#[test]
fn test_standings_model_given_no_correction_after_the_watermark_is_clean() -> Result<(), String> {
    // Arrange
    let roster = two_solo_roster()?;
    let mut engine = draft_engine(&fixture_config(1, false)?, &roster)?;
    start(&mut engine)?;
    lap(&mut engine, "10", 60)?;
    let watermark = engine.events().len();

    // Act
    let model = standings_model(&engine, ORDER, true, Some(watermark))?;

    // Assert
    assert!(!model.stale);
    Ok(())
}

#[test]
fn test_standings_model_given_a_joker_draw_returns_the_standings_refusal() -> Result<(), String> {
    // Arrange: the venue draw never deals a joker, so a stored joker draw can
    // only arrive from a damaged log; the ranker refuses it.
    let roster = two_solo_roster()?;
    let engine = finished_engine_with_manual_draw(&roster, ["JK", "AS"])?;

    // Act
    let result = standings_model(&engine, ORDER, true, None);

    // Assert
    assert_eq!(result.err(), Some("a joker has no draw key".to_owned()));
    Ok(())
}

// ------------------------------------------------ self-test flag

#[test]
fn test_standings_model_given_a_clean_finish_is_verified() -> Result<(), String> {
    // Arrange
    let roster = two_solo_roster()?;
    let mut engine = draft_engine(&fixture_config(1, false)?, &roster)?;
    start(&mut engine)?;
    engine.finish(&[]).map_err(|error| error.to_string())?;

    // Act
    let model = standings_model(&engine, ORDER, true, None)?;

    // Assert
    assert!(!model.self_test_unverified);
    Ok(())
}

#[test]
fn test_standings_model_given_an_override_finish_is_unverified() -> Result<(), String> {
    // Arrange
    let roster = two_solo_roster()?;
    let mut engine = draft_engine(&fixture_config(1, false)?, &roster)?;
    start(&mut engine)?;
    engine
        .finish(&["7,462 distinct ranks".to_owned()])
        .map_err(|error| error.to_string())?;

    // Act
    let model = standings_model(&engine, ORDER, true, None)?;

    // Assert
    assert!(model.self_test_unverified);
    Ok(())
}

// ------------------------------------------------- export watermark

#[test]
fn test_export_watermark_defaults_to_no_export() {
    // Arrange
    let watermark = ExportWatermark::default();

    // Act
    let value = watermark.value();

    // Assert
    assert_eq!(value, None);
}

#[test]
fn test_export_watermark_mark_exported_records_the_event_count() {
    // Arrange
    let mut watermark = ExportWatermark::default();

    // Act
    watermark.mark_exported(6);

    // Assert
    assert_eq!(watermark.value(), Some(6));
}

#[test]
fn test_export_watermark_mark_exported_clears_a_stale_model() -> Result<(), String> {
    // Arrange: a correction after the export makes the model stale, then a
    // fresh export advances the watermark past it.
    let roster = two_solo_roster()?;
    let mut engine = draft_engine(&fixture_config(1, false)?, &roster)?;
    start(&mut engine)?;
    lap(&mut engine, "10", 60)?;
    let entry = roster
        .entries()
        .iter()
        .find(|entry| entry.plate == "10")
        .ok_or_else(|| "no entry".to_owned())?;
    let gun = engine
        .actual_start()
        .ok_or_else(|| "the ride never started".to_owned())?;
    engine
        .edit_crossing(&EntryId(entry.key.clone()), 1, shifted(gun, 90)?, "mis-key")
        .map_err(|error| error.to_string())?;
    let mut watermark = ExportWatermark::default();
    watermark.mark_exported(2);
    let stale_before = standings_model(&engine, ORDER, true, watermark.value())?.stale;

    // Act
    watermark.mark_exported(engine.events().len());
    let stale_after = standings_model(&engine, ORDER, true, watermark.value())?.stale;

    // Assert
    assert!(stale_before);
    assert!(!stale_after);
    Ok(())
}

// --------------------------------------------- correction vocabulary

#[rstest]
#[case(EventAction::AddCrossingAt)]
#[case(EventAction::AssignPlateToMiss)]
#[case(EventAction::EditCrossing)]
#[case(EventAction::Reassign)]
#[case(EventAction::DealManual)]
#[case(EventAction::Dnf)]
#[case(EventAction::VoidCard)]
#[case(EventAction::VoidCrossing)]
fn test_correction_actions_carry_the_eight_audited_corrections(#[case] action: EventAction) {
    // Arrange
    let events = vec![Event {
        action,
        payload: json!({}),
    }];

    // Act
    let stale = has_correction_since(&events, Some(0));

    // Assert
    assert!(stale);
    assert!(CORRECTION_ACTIONS.contains(&action));
}

#[rstest]
#[case(EventAction::RecordCrossing)]
#[case(EventAction::RecordMiss)]
#[case(EventAction::Undo)]
#[case(EventAction::SetStartTime)]
fn test_correction_actions_exclude_the_live_events(#[case] action: EventAction) {
    // Arrange
    let events = vec![Event {
        action,
        payload: json!({}),
    }];

    // Act
    let stale = has_correction_since(&events, Some(0));

    // Assert
    assert!(!stale);
    assert!(!CORRECTION_ACTIONS.contains(&action));
}

#[test]
fn test_has_correction_since_without_a_watermark_is_never_stale() {
    // Arrange
    let events = vec![
        Event {
            action: EventAction::Start,
            payload: json!({}),
        },
        Event {
            action: EventAction::EditCrossing,
            payload: json!({}),
        },
    ];

    // Act
    let stale = has_correction_since(&events, None);

    // Assert
    assert!(!stale);
}

#[test]
fn test_has_correction_since_given_a_correction_at_the_watermark_is_true() {
    // Arrange: the edit sits at index 2, so an export that captured two events
    // still predates it.
    let events = vec![
        Event {
            action: EventAction::Start,
            payload: json!({}),
        },
        Event {
            action: EventAction::RecordCrossing,
            payload: json!({}),
        },
        Event {
            action: EventAction::EditCrossing,
            payload: json!({}),
        },
    ];

    // Act
    let stale = has_correction_since(&events, Some(2));

    // Assert
    assert!(stale);
}

#[test]
fn test_has_correction_since_given_the_watermark_at_the_event_count_is_clean() {
    // Arrange
    let events = vec![
        Event {
            action: EventAction::Start,
            payload: json!({}),
        },
        Event {
            action: EventAction::EditCrossing,
            payload: json!({}),
        },
    ];

    // Act
    let at_count = has_correction_since(&events, Some(2));
    let past_count = has_correction_since(&events, Some(9));

    // Assert
    assert!(!at_count);
    assert!(!past_count);
}

#[test]
fn test_has_correction_since_given_a_non_correction_after_the_watermark_is_clean() {
    // Arrange: a live crossing after the edit is not a correction.
    let events = vec![
        Event {
            action: EventAction::Start,
            payload: json!({}),
        },
        Event {
            action: EventAction::EditCrossing,
            payload: json!({}),
        },
        Event {
            action: EventAction::RecordCrossing,
            payload: json!({}),
        },
    ];

    // Act
    let stale = has_correction_since(&events, Some(2));

    // Assert
    assert!(!stale);
}

#[test]
fn test_standings_model_given_a_reopened_ride_uses_the_live_order() -> Result<(), String> {
    // Arrange
    let roster = two_solo_roster()?;
    let mut engine = draft_engine(&fixture_config(1, false)?, &roster)?;
    start(&mut engine)?;
    lap(&mut engine, "10", 60)?;
    engine.finish(&[]).map_err(|error| error.to_string())?;
    engine.reopen().map_err(|error| error.to_string())?;
    assert_eq!(engine.state(), RideStatus::Reopened);

    // Act
    let model = standings_model(&engine, ORDER, false, None)?;

    // Assert
    assert_eq!(model.solo.len(), 2);
    assert_eq!(model.solo.first().map(|row| row.place), Some(Some(1)));
    Ok(())
}
