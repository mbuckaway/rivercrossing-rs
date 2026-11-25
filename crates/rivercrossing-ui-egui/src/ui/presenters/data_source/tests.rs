// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the console's read-only view models and display helpers.

use pretty_assertions::assert_eq;
use rivercrossing_ride::{
    Crossing, EntryId, EntryMode, PlateModel, RideConfig, RideConfigParams, RideEngine, RideStatus,
    SystemClock, Timestamp,
};
use rivercrossing_roster::roster::{Roster, TeamFields};
use rivercrossing_roster::types::Rider;
use rivercrossing_store::Store;
use rstest::rstest;

use super::{
    FeedRow, StopLight, clock_fraction, counters, current_lap, feed_rows, filter_rows,
    format_duration, is_miss, review_rows, status_text, stop_light_mode,
};

/// The ride name every fixture uses.
const RIDE_NAME: &str = "GORBA EPIC 2026";

/// The solo plate every fixture uses.
const PLATE: &str = "101";

/// The seed every fixture ride stores.
const SEED: u64 = 7;

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

/// Return a roster holding one solo entry and one two-rider team.
fn mixed_roster() -> Result<Roster, String> {
    let mut roster = solo_roster()?;
    roster
        .create_team_entry(TeamFields {
            display_name: "Team Rocket",
            riders: vec![
                Rider {
                    first_name: "Grace".to_owned(),
                    last_name: "Hopper".to_owned(),
                    plate: Some("201".to_owned()),
                    ..Rider::default()
                },
                Rider {
                    first_name: "Alan".to_owned(),
                    last_name: "Turing".to_owned(),
                    plate: Some("202".to_owned()),
                    ..Rider::default()
                },
            ],
            plate: None,
            logo_card: None,
        })
        .map_err(|error| error.to_string())?;
    Ok(roster)
}

/// Build a DRAFT engine from a real temporary store over *config* and *roster*.
///
/// The store mints the seeded shoe; the engine is rebuilt from the stored log,
/// exactly as the app loads a ride.
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

/// Build a RUNNING engine over *config* and *roster*.
fn running_engine(
    config: &RideConfig,
    roster: &Roster,
) -> Result<RideEngine<Roster, SystemClock>, String> {
    let mut engine = draft_engine(config, roster)?;
    engine.start(None).map_err(|error| error.to_string())?;
    Ok(engine)
}

/// Return *start* moved by *seconds*, negative values included.
fn shifted(start: Timestamp, seconds: i64) -> Result<Timestamp, String> {
    Timestamp::from_unix_timestamp(start.unix_timestamp() + seconds)
        .map_err(|error| error.to_string())
}

/// Return a crossing on [`PLATE`] with the given per-entry lap number.
fn crossing(seq: u32) -> Result<Crossing, String> {
    Ok(Crossing {
        entry_id: EntryId("entry".to_owned()),
        seq,
        crossed_at: Timestamp::from_unix_timestamp(i64::from(seq))
            .map_err(|error| error.to_string())?,
        rider_plate: Some(PLATE.to_owned()),
        deal_seq: u64::from(seq),
    })
}

/// Return a feed row for the filter tests.
fn row(plate: &str, name: &str, card: Option<&str>) -> FeedRow {
    FeedRow {
        plate: plate.to_owned(),
        name: name.to_owned(),
        lap: 1,
        lap_time: 100.0,
        card: card.map(str::to_owned),
        flagged: false,
        edited: false,
        dnf: false,
        held: false,
    }
}

// ------------------------------------------------- display helpers

#[rstest]
#[case(0.0, "0:00:00")]
#[case(59.9, "0:00:59")]
#[case(65.0, "0:01:05")]
#[case(3661.0, "1:01:01")]
#[case(21600.0, "6:00:00")]
#[case(-5.0, "0:00:00")]
fn test_format_duration_given_positive_zero_and_negative(
    #[case] seconds: f64,
    #[case] expected: &str,
) {
    // Arrange
    // (the case supplies seconds and the expected clock text)

    // Act
    let text = format_duration(seconds);

    // Assert
    assert_eq!(text, expected);
}

#[rstest]
#[case(-100.0, 0.0)]
#[case(0.0, 0.0)]
#[case(10800.0, 0.5)]
#[case(21600.0, 1.0)]
#[case(43200.0, 1.0)]
fn test_clock_fraction_clamps(#[case] seconds: f64, #[case] expected: f32) {
    // Arrange
    let total = 21600.0;

    // Act
    let fraction = clock_fraction(seconds, total);

    // Assert: bitwise equality keeps the check exact without a float compare.
    assert_eq!(fraction.to_bits(), expected.to_bits());
}

#[rstest]
#[case("=", true)]
#[case("/", true)]
#[case("+", true)]
#[case("-", true)]
#[case(".", true)]
#[case("=+-.", true)]
#[case("  =  ", true)]
#[case("", false)]
#[case("   ", false)]
#[case("101", false)]
#[case("=1", false)]
fn test_is_miss_accepts_the_five_symbols_and_rejects_blank_and_plates(
    #[case] text: &str,
    #[case] expected: bool,
) {
    // Arrange
    // (the case supplies the field text)

    // Act
    let verdict = is_miss(text);

    // Assert
    assert_eq!(verdict, expected);
}

#[rstest]
#[case(RideStatus::Draft, false, "DRAFT")]
#[case(RideStatus::Running, false, "RUNNING")]
#[case(RideStatus::Running, true, "STOPPED")]
#[case(RideStatus::Finished, false, "FINISHED")]
#[case(RideStatus::Reopened, false, "REOPENED")]
#[case(RideStatus::Reopened, true, "REOPENED")]
fn test_status_text_names_the_stopped_state(
    #[case] status: RideStatus,
    #[case] stopped: bool,
    #[case] expected: &str,
) {
    // Arrange
    // (the case supplies the status and the stop guard)

    // Act
    let text = status_text(status, stopped);

    // Assert
    assert_eq!(text, expected);
}

#[rstest]
#[case(None, false, StopLight::Off)]
#[case(Some(RideStatus::Running), false, StopLight::Green)]
#[case(Some(RideStatus::Running), true, StopLight::Yellow)]
#[case(Some(RideStatus::Draft), false, StopLight::Red)]
#[case(Some(RideStatus::Draft), true, StopLight::Red)]
#[case(Some(RideStatus::Finished), false, StopLight::Red)]
#[case(Some(RideStatus::Reopened), false, StopLight::Yellow)]
#[case(Some(RideStatus::Reopened), true, StopLight::Yellow)]
fn test_stop_light_mode_per_state(
    #[case] status: Option<RideStatus>,
    #[case] stopped: bool,
    #[case] expected: StopLight,
) {
    // Arrange
    // (the case supplies the ride status and the stop guard)

    // Act
    let light = stop_light_mode(status, stopped);

    // Assert
    assert_eq!(light, expected);
}

#[test]
fn test_current_lap_is_the_highest_sequence() -> Result<(), String> {
    // Arrange
    let crossings = vec![crossing(1)?, crossing(3)?, crossing(2)?];

    // Act
    let lap = current_lap(&crossings);

    // Assert
    assert_eq!(lap, 3);
    assert_eq!(current_lap(&[]), 0);
    Ok(())
}

// ------------------------------------------------------ feed rows

#[test]
fn test_feed_rows_are_newest_first_and_include_misses() -> Result<(), String> {
    // Arrange: one miss slots between the crossings, and one older miss
    // appends at the end.
    let roster = solo_roster()?;
    let config = fixture_config(1, false)?;
    let mut engine = running_engine(&config, &roster)?;
    let start = engine.actual_start().ok_or("the ride never started")?;
    let _ = engine.record_crossing(PLATE, Some(shifted(start, 100)?));
    let _ = engine.record_crossing(PLATE, Some(shifted(start, 200)?));
    engine
        .record_miss(shifted(start, 150)?, "missed number")
        .map_err(|error| error.to_string())?;
    engine
        .record_miss(shifted(start, 50)?, "missed number")
        .map_err(|error| error.to_string())?;

    // Act
    let rows = feed_rows(&engine, &roster);

    // Assert
    let summary: Vec<(String, String)> = rows
        .iter()
        .map(|row| (row.plate.clone(), row.name.clone()))
        .collect();
    assert_eq!(
        summary,
        vec![
            (PLATE.to_owned(), "Ada Lovelace".to_owned()),
            ("-".to_owned(), "missed".to_owned()),
            (PLATE.to_owned(), "Ada Lovelace".to_owned()),
            ("-".to_owned(), "missed".to_owned()),
        ]
    );
    assert_eq!(rows.get(1).map(|row| row.lap), Some(0));
    assert_eq!(rows.get(1).and_then(|row| row.card.clone()), None);
    Ok(())
}

#[test]
fn test_feed_rows_flag_a_short_lap() -> Result<(), String> {
    // Arrange
    let roster = solo_roster()?;
    let config = fixture_config(60, true)?;
    let mut engine = running_engine(&config, &roster)?;
    let start = engine.actual_start().ok_or("the ride never started")?;
    let _ = engine.record_crossing(PLATE, Some(shifted(start, 100)?));
    let _ = engine.record_crossing(PLATE, Some(shifted(start, 150)?));
    let second = engine
        .crossings()
        .get(1)
        .ok_or("the second lap is missing")?;
    let dealt = engine.held_card_for(second).map(|card| card.code());

    // Act
    let rows = feed_rows(&engine, &roster);

    // Assert
    assert_eq!(
        rows.first().map(|row| (row.lap, row.flagged, row.held)),
        Some((2, true, true))
    );
    assert_eq!(
        rows.get(1).map(|row| (row.lap, row.flagged)),
        Some((1, false))
    );
    assert_eq!(rows.first().map(|row| row.lap_time), Some(50.0));
    assert_eq!(rows.first().and_then(|row| row.card.clone()), dealt);
    Ok(())
}

#[test]
fn test_filter_rows_matches_plate_name_and_card_case_insensitively() {
    // Arrange: the third row dealt no card, so a search reads a blank cell.
    let rows = vec![
        row("101", "Ada Lovelace", Some("9H")),
        row("202", "Grace Hopper", Some("KC")),
        row("303", "Annie Easley", None),
    ];

    // Act
    let by_plate = filter_rows(&rows, "10");
    let by_name = filter_rows(&rows, "ada");
    let by_card = filter_rows(&rows, "9h");
    let blank = filter_rows(&rows, "   ");
    let missing = filter_rows(&rows, "zzz");

    // Assert
    assert_eq!(by_plate, vec![rows[0].clone()]);
    assert_eq!(by_name, vec![rows[0].clone()]);
    assert_eq!(by_card, vec![rows[0].clone()]);
    assert_eq!(blank, rows);
    assert_eq!(missing, Vec::new());
}

// -------------------------------------------------------- counters

#[test]
fn test_counters_count_crossings_cards_riders_and_teams() -> Result<(), String> {
    // Arrange
    let roster = mixed_roster()?;
    let config = fixture_config(1, false)?;
    let mut engine = running_engine(&config, &roster)?;
    let start = engine.actual_start().ok_or("the ride never started")?;
    let _ = engine.record_crossing(PLATE, Some(shifted(start, 100)?));

    // Act
    let counts = counters(&engine, &roster);

    // Assert
    assert_eq!(counts.crossings, 1);
    assert_eq!(counts.cards_dealt, 1);
    assert_eq!(counts.on_course, 1);
    assert_eq!(counts.riders, 3);
    assert_eq!(counts.teams, 1);
    assert_eq!(counts.shoe_remaining + 1, counts.shoe_total);
    Ok(())
}

// ----------------------------------------------------- review rows

#[test]
fn test_review_rows_given_held_only() -> Result<(), String> {
    // Arrange: an always-deal ride credits the short lap, so it is flagged
    // for review but never held.
    let roster = solo_roster()?;
    let config = fixture_config(60, false)?;
    let mut engine = running_engine(&config, &roster)?;
    let start = engine.actual_start().ok_or("the ride never started")?;
    let _ = engine.record_crossing(PLATE, Some(shifted(start, 100)?));
    let _ = engine.record_crossing(PLATE, Some(shifted(start, 150)?));

    // Act
    let held_only = review_rows(&engine, &roster, true);
    let all = review_rows(&engine, &roster, false);

    // Assert
    assert_eq!(held_only, Vec::new());
    assert_eq!(all.len(), 1);
    assert_eq!(all.first().map(|row| row.issue.as_str()), Some("Short lap"));
    assert_eq!(all.first().map(|row| row.card.as_str()), Some("Credited"));
    assert_eq!(all.first().map(|row| row.plate.as_str()), Some(PLATE));
    assert_eq!(all.first().map(|row| row.lap), Some(2));
    assert_eq!(all.first().map(|row| row.lap_time), Some(50.0));
    assert_eq!(
        all.first().map(|row| row.rider.as_str()),
        Some("Ada Lovelace")
    );
    Ok(())
}

#[test]
fn test_review_rows_keep_a_held_short_lap_under_the_held_filter() -> Result<(), String> {
    // Arrange
    let roster = solo_roster()?;
    let config = fixture_config(60, true)?;
    let mut engine = running_engine(&config, &roster)?;
    let start = engine.actual_start().ok_or("the ride never started")?;
    let _ = engine.record_crossing(PLATE, Some(shifted(start, 100)?));
    let _ = engine.record_crossing(PLATE, Some(shifted(start, 150)?));

    // Act
    let held_only = review_rows(&engine, &roster, true);

    // Assert
    assert_eq!(held_only.len(), 1);
    assert_eq!(held_only.first().map(|row| row.card.as_str()), Some("Held"));
    Ok(())
}

// -------------------------------------------- correction markers

/// Return a roster holding two solo entries on [`PLATE`] and `303`.
fn two_solo_roster() -> Result<Roster, String> {
    let mut roster = solo_roster()?;
    roster
        .create_solo_entry(
            Rider {
                first_name: "Grace".to_owned(),
                last_name: "Hopper".to_owned(),
                ..Rider::default()
            },
            "303",
        )
        .map_err(|error| error.to_string())?;
    Ok(roster)
}

#[test]
fn test_feed_rows_mark_an_edited_crossing() -> Result<(), String> {
    // Arrange
    let roster = solo_roster()?;
    let config = fixture_config(1, false)?;
    let mut engine = running_engine(&config, &roster)?;
    let start = engine.actual_start().ok_or("the ride never started")?;
    let _ = engine.record_crossing(PLATE, Some(shifted(start, 100)?));
    let entry_id = engine
        .crossings()
        .first()
        .ok_or("the crossing is missing")?
        .entry_id
        .clone();
    let moved = shifted(start, 150)?;
    engine
        .edit_crossing(&entry_id, 1, moved, "fix the time")
        .map_err(|error| error.to_string())?;

    // Act
    let rows = feed_rows(&engine, &roster);

    // Assert
    assert_eq!(rows.len(), 1);
    assert_eq!(rows.first().map(|row| row.edited), Some(true));
    assert_eq!(
        rows.first().map(|row| row.lap_time),
        Some((moved - start).as_seconds_f64())
    );
    Ok(())
}

#[test]
fn test_feed_rows_mark_an_added_crossing() -> Result<(), String> {
    // Arrange
    let roster = solo_roster()?;
    let config = fixture_config(1, false)?;
    let mut engine = running_engine(&config, &roster)?;
    let start = engine.actual_start().ok_or("the ride never started")?;
    let _ = engine.record_crossing(PLATE, Some(shifted(start, 100)?));
    engine
        .add_crossing_at(PLATE, shifted(start, 300)?, "added")
        .map_err(|error| error.to_string())?;

    // Act
    let rows = feed_rows(&engine, &roster);

    // Assert
    assert_eq!(
        rows.first().map(|row| (row.lap, row.edited)),
        Some((2, true))
    );
    assert_eq!(rows.get(1).map(|row| row.edited), Some(false));
    Ok(())
}

#[test]
fn test_feed_rows_mark_a_reassigned_crossing() -> Result<(), String> {
    // Arrange
    let roster = two_solo_roster()?;
    let config = fixture_config(1, false)?;
    let mut engine = running_engine(&config, &roster)?;
    let start = engine.actual_start().ok_or("the ride never started")?;
    let _ = engine.record_crossing(PLATE, Some(shifted(start, 100)?));
    let _ = engine.record_crossing(PLATE, Some(shifted(start, 200)?));
    engine
        .reassign_crossing(2, "303", "moved")
        .map_err(|error| error.to_string())?;

    // Act
    let rows = feed_rows(&engine, &roster);

    // Assert
    assert_eq!(
        rows.first().map(|row| (row.plate.as_str(), row.edited)),
        Some(("303", true))
    );
    assert_eq!(rows.get(1).map(|row| row.edited), Some(false));
    Ok(())
}

#[test]
fn test_feed_rows_mark_an_assigned_miss() -> Result<(), String> {
    // Arrange
    let roster = solo_roster()?;
    let config = fixture_config(1, false)?;
    let mut engine = running_engine(&config, &roster)?;
    let start = engine.actual_start().ok_or("the ride never started")?;
    engine
        .record_miss(shifted(start, 150)?, "missed number")
        .map_err(|error| error.to_string())?;
    engine
        .assign_plate_to_miss(1, PLATE, "found")
        .map_err(|error| error.to_string())?;

    // Act
    let rows = feed_rows(&engine, &roster);

    // Assert
    assert_eq!(rows.len(), 1);
    assert_eq!(rows.first().map(|row| row.plate.as_str()), Some(PLATE));
    assert_eq!(rows.first().map(|row| row.edited), Some(true));
    Ok(())
}

#[test]
fn test_review_rows_word_a_voided_card() -> Result<(), String> {
    // Arrange: an always-deal ride credits the short lap's card, so voiding
    // that card leaves the flagged crossing live with a void disposition.
    let roster = solo_roster()?;
    let config = fixture_config(60, false)?;
    let mut engine = running_engine(&config, &roster)?;
    let start = engine.actual_start().ok_or("the ride never started")?;
    let _ = engine.record_crossing(PLATE, Some(shifted(start, 100)?));
    let _ = engine.record_crossing(PLATE, Some(shifted(start, 150)?));
    let short = engine
        .crossings()
        .get(1)
        .ok_or("the short lap is missing")?
        .clone();
    let card = engine.card_for(&short).ok_or("the lap dealt no card")?;
    engine
        .void_card(PLATE, &card, "duplicate card")
        .map_err(|error| error.to_string())?;

    // Act
    let rows = review_rows(&engine, &roster, false);

    // Assert
    assert_eq!(rows.len(), 1);
    assert_eq!(rows.first().map(|row| row.card.as_str()), Some("Void"));
    Ok(())
}

/// Return a relay roster holding one two-rider team on plate `201`.
fn relay_team_roster() -> Result<Roster, String> {
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::TeamRelay, Some(SEED))
        .map_err(|error| error.to_string())?;
    roster
        .create_team_entry(TeamFields {
            display_name: "Team Rocket",
            riders: vec![
                Rider {
                    first_name: "Grace".to_owned(),
                    last_name: "Hopper".to_owned(),
                    ..Rider::default()
                },
                Rider {
                    first_name: "Alan".to_owned(),
                    last_name: "Turing".to_owned(),
                    ..Rider::default()
                },
            ],
            plate: Some("201"),
            logo_card: None,
        })
        .map_err(|error| error.to_string())?;
    Ok(roster)
}

#[test]
fn test_feed_rows_name_a_relay_entry_by_its_display_name() -> Result<(), String> {
    // Arrange: a relay team's riders carry no plate, so the crossing names
    // the entry rather than a rider.
    let roster = relay_team_roster()?;
    let config = RideConfig::new(RideConfigParams {
        name: RIDE_NAME.to_owned(),
        venue: "Sea to Sky Gondola".to_owned(),
        lap_km: 8.0,
        organizer: "GORBA".to_owned(),
        scorer: "K. Singh".to_owned(),
        entry_mode: EntryMode::Mixed,
        plate_model: PlateModel::TeamRelay,
        ..RideConfigParams::default()
    })
    .map_err(|error| error.to_string())?;
    let mut engine = running_engine(&config, &roster)?;
    let start = engine.actual_start().ok_or("the ride never started")?;
    let _ = engine.record_crossing("201", Some(shifted(start, 100)?));

    // Act
    let rows = feed_rows(&engine, &roster);

    // Assert
    assert_eq!(rows.first().map(|row| row.plate.as_str()), Some("201"));
    assert_eq!(
        rows.first().map(|row| row.name.as_str()),
        Some("Team Rocket")
    );
    Ok(())
}

#[test]
fn test_review_rows_word_a_team_overlap() -> Result<(), String> {
    // Arrange: a five-second lap on a team entry reads as its riders
    // overlapping, under the 60-second minimum.
    let roster = mixed_roster()?;
    let config = fixture_config(60, false)?;
    let mut engine = running_engine(&config, &roster)?;
    let start = engine.actual_start().ok_or("the ride never started")?;
    let _ = engine.record_crossing("201", Some(shifted(start, 5)?));

    // Act
    let rows = review_rows(&engine, &roster, false);

    // Assert
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows.first().map(|row| row.issue.as_str()),
        Some("Team overlap")
    );
    Ok(())
}

#[test]
fn test_review_rows_word_a_duplicate_crossing() -> Result<(), String> {
    // Arrange: two laps of one entry at one instant are a live pair.
    let roster = solo_roster()?;
    let config = fixture_config(1, false)?;
    let mut engine = running_engine(&config, &roster)?;
    let start = engine.actual_start().ok_or("the ride never started")?;
    let same = shifted(start, 100)?;
    let _ = engine.record_crossing(PLATE, Some(same));
    let _ = engine.record_crossing(PLATE, Some(same));

    // Act
    let rows = review_rows(&engine, &roster, true);

    // Assert
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows.iter()
            .map(|row| row.issue.as_str())
            .collect::<Vec<_>>(),
        vec!["Duplicate crossing", "Duplicate crossing"]
    );
    Ok(())
}
