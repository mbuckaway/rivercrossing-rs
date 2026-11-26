// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the Rider Simulator's decisions and its scripted race.
//!
//! The four pure helpers and the three generators are pinned directly; the
//! scripted run drives a real ride engine built through a temporary store, so
//! every crossing is the same crossing the console records.

#![expect(
    clippy::float_cmp,
    reason = "the asserted lap times are whole seconds that the engine copies unchanged"
)]
#![expect(
    clippy::too_many_arguments,
    reason = "rstest expands a case table's own columns into the generated test function's arguments"
)]

use std::collections::HashMap;

use pretty_assertions::assert_eq;
use rivercrossing_ride::{
    Clock, EntryId, EntryMode, PlateModel, RideConfig, RideConfigParams, RideEngine, RideStatus,
    SystemClock, Timestamp,
};
use rivercrossing_roster::enums::EntryType;
use rivercrossing_roster::roster::{Roster, TeamFields};
use rivercrossing_roster::types::Rider;
use rivercrossing_store::Store;
use rstest::rstest;

use super::{
    DEFAULT_MAX_TEAM_SIZE, GeneratorInput, INTERVAL_MAX_MINUTES, INTERVAL_MIN_MINUTES, SimOutcome,
    SimulationSettings, check_message, default_interval_minutes, generate_riders,
    generate_solo_riders, generate_teams, resolve_solo, run_simulation, short_lap_gap, validate,
};

/// The ride name every fixture uses.
const RIDE_NAME: &str = "GORBA EPIC 2026";

/// The seed every generator and run test pins.
const SEED: u64 = 42;

/// The fixed gun that the position assertions measure from.
const SECONDS_PER_MINUTE: i64 = 60;

/// The fixed ride seed the store writes.
const RIDE_SEED: u64 = 20_260_920;

// ------------------------------------------------------------- fixtures

/// Return a valid fixture config.
fn fixture_config(
    plate_model: PlateModel,
    min_lap_s: i32,
    deck_count: i32,
) -> Result<RideConfig, String> {
    RideConfig::new(RideConfigParams {
        name: RIDE_NAME.to_owned(),
        venue: "Sea to Sky Gondola".to_owned(),
        lap_km: 8.0,
        organizer: "GORBA".to_owned(),
        scorer: "K. Singh".to_owned(),
        entry_mode: EntryMode::Mixed,
        plate_model,
        min_lap_s,
        deck_count,
        ..RideConfigParams::default()
    })
    .map_err(|error| error.to_string())
}

/// Return an empty `DRAFT` roster over `plate_model`.
fn draft_roster(plate_model: PlateModel) -> Result<Roster, String> {
    Roster::new(EntryMode::Mixed, 4, plate_model, Some(RIDE_SEED))
        .map_err(|error| error.to_string())
}

/// Build a DRAFT engine from a real temporary store over *config* and *roster*.
fn engine_over(
    config: &RideConfig,
    roster: &Roster,
) -> Result<RideEngine<Roster, SystemClock>, String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let mut store =
        Store::open(directory.path().join("rides.db")).map_err(|error| error.to_string())?;
    let ride_id = store
        .create_ride(config, Some(RIDE_SEED))
        .map_err(|error| error.to_string())?;
    store
        .save_roster(ride_id, roster)
        .map_err(|error| error.to_string())?;
    store
        .load_engine(ride_id)
        .map_err(|error| error.to_string())
}

/// Return a generated roster and the engine that holds a copy of it.
fn generated(
    plate_model: PlateModel,
    min_lap_s: i32,
    input: &GeneratorInput,
) -> Result<(Roster, RideEngine<Roster, SystemClock>), String> {
    let mut roster = draft_roster(plate_model)?;
    generate_riders(&mut roster, input)?;
    let config = fixture_config(plate_model, min_lap_s, 8)?;
    let engine = engine_over(&config, &roster)?;
    Ok((roster, engine))
}

/// Return the riders of *roster*, in entry creation order.
fn riders_of(roster: &Roster) -> Vec<&Rider> {
    roster
        .entries()
        .iter()
        .flat_map(|entry| entry.riders.iter())
        .collect()
}

/// Return the team sizes of *roster*.
fn team_sizes(roster: &Roster) -> Vec<usize> {
    roster
        .entries()
        .iter()
        .filter(|entry| entry.kind == EntryType::Team)
        .map(|entry| entry.riders.len())
        .collect()
}

/// One rider's signature: first name, last name, plate and sex.
type RiderSignature = (String, String, Option<String>, Option<String>);

/// One entry's signature: its name and its riders.
type EntrySignature = (String, Vec<RiderSignature>);

/// Return each entry's name and riders, for a compare.
fn signature(roster: &Roster) -> Vec<EntrySignature> {
    roster
        .entries()
        .iter()
        .map(|entry| {
            let riders = entry
                .riders
                .iter()
                .map(|rider| {
                    (
                        rider.first_name.clone(),
                        rider.last_name.clone(),
                        rider.plate.clone(),
                        rider.sex.clone(),
                    )
                })
                .collect();
            (entry.display_name.clone(), riders)
        })
        .collect()
}

/// Return the plates of every crossing, paired with its rider plate and the
/// whole seconds from the gun.
fn race(engine: &RideEngine<Roster, SystemClock>, roster: &Roster) -> Vec<(String, i64)> {
    let plates: HashMap<&str, &str> = roster
        .entries()
        .iter()
        .map(|entry| (entry.key.as_str(), entry.plate.as_str()))
        .collect();
    let gun = engine.actual_start();
    engine
        .crossings()
        .iter()
        .map(|crossing| {
            let plate = plates
                .get(crossing.entry_id.0.as_str())
                .copied()
                .unwrap_or_default()
                .to_owned();
            let seconds = gun.map_or(0, |gun| (crossing.crossed_at - gun).whole_seconds());
            (plate, seconds)
        })
        .collect()
}

/// Return *gun* moved by *minutes*.
fn after_minutes(gun: Timestamp, minutes: i64) -> Result<Timestamp, String> {
    Timestamp::from_unix_timestamp(gun.unix_timestamp() + minutes * SECONDS_PER_MINUTE)
        .map_err(|error| error.to_string())
}

/// Run *settings* over *engine* with a silent progress hook and no cancel.
fn run(
    engine: &mut RideEngine<Roster, SystemClock>,
    roster: &Roster,
    settings: &SimulationSettings,
) -> SimOutcome {
    run_simulation(engine, roster, settings, &mut |_fraction| {}, &|| false)
}

/// Return the default settings for *laps* and *interval*.
fn settings(laps: i64, interval_minutes: i64) -> SimulationSettings {
    SimulationSettings {
        laps,
        interval_minutes,
        short_laps: 0,
        lapped: 0,
        team_stop: 0,
        seed: SEED,
    }
}

// --------------------------------------------------------- resolve_solo

#[rstest]
#[case(79, 40, None)]
#[case(80, 40, Some(0))]
#[case(81, 40, Some(0))]
#[case(159, 40, Some(0))]
#[case(160, 40, Some(0))]
#[case(161, 40, Some(1))]
#[case(175, 40, Some(15))]
#[case(800, 40, Some(640))]
#[case(1, 1, None)]
#[case(2, 1, Some(0))]
#[case(3, 1, Some(0))]
#[case(4, 1, Some(0))]
#[case(5, 1, Some(1))]
fn test_resolve_solo_given_a_rider_count_returns_its_boundary_value(
    #[case] riders: i64,
    #[case] teams: i64,
    #[case] expected: Option<i64>,
) {
    // Arrange
    let min_team_size = 2;
    let max_team_size = DEFAULT_MAX_TEAM_SIZE;

    // Act
    let result = resolve_solo(riders, teams, min_team_size, max_team_size);

    // Assert
    assert_eq!(result, expected);
}

#[rstest]
#[case(8, None)]
#[case(9, Some(0))]
#[case(10, Some(0))]
#[case(14, Some(0))]
#[case(15, Some(0))]
#[case(16, Some(1))]
fn test_resolve_solo_given_explicit_team_bounds_resolves_against_them(
    #[case] riders: i64,
    #[case] expected: Option<i64>,
) {
    // Arrange: three teams of 3..5 riders bound the field at 9..15.
    let teams = 3;

    // Act
    let result = resolve_solo(riders, teams, 3, 5);

    // Assert
    assert_eq!(result, expected);
}

#[rstest]
#[case(80, 40)]
#[case(161, 40)]
#[case(175, 40)]
#[case(500, 100)]
fn test_resolve_solo_given_a_count_returns_an_inside_bounds_solo(
    #[case] riders: i64,
    #[case] teams: i64,
) {
    // Arrange
    let min_team_size = 2;
    let max_team_size = DEFAULT_MAX_TEAM_SIZE;

    // Act
    let solo = resolve_solo(riders, teams, min_team_size, max_team_size);

    // Assert
    let holds = match solo {
        Some(solo) => (min_team_size * teams..=max_team_size * teams).contains(&(riders - solo)),
        None => riders < min_team_size * teams,
    };
    assert!(holds);
}

// ------------------------------------------- default_interval_minutes

#[rstest]
#[case(8.0, 12.0, 45)]
#[case(4.0, 12.0, 25)]
#[case(8.0, 24.0, 25)]
#[case(0.1, 60.0, 5)]
#[case(30.0, 1.0, 240)]
fn test_default_interval_minutes_given_speed_and_lap_returns_the_formula(
    #[case] lap_km: f64,
    #[case] avg_speed_kmh: f64,
    #[case] expected: i64,
) {
    // Arrange
    let minimum = INTERVAL_MIN_MINUTES;
    let maximum = INTERVAL_MAX_MINUTES;

    // Act
    let minutes = default_interval_minutes(lap_km, avg_speed_kmh, minimum, maximum);

    // Assert
    assert_eq!(minutes, expected);
}

#[rstest]
#[case(1, 240, 45)]
#[case(45, 240, 45)]
#[case(46, 240, 46)]
#[case(1, 45, 45)]
#[case(1, 44, 44)]
fn test_default_interval_minutes_given_explicit_bounds_clamps_into_them(
    #[case] minimum: i64,
    #[case] maximum: i64,
    #[case] expected: i64,
) {
    // Arrange
    let lap_km = 8.0;
    let avg_speed_kmh = 12.0;

    // Act
    let minutes = default_interval_minutes(lap_km, avg_speed_kmh, minimum, maximum);

    // Assert
    assert_eq!(minutes, expected);
}

#[rstest]
#[case(0.1, 1000.0)]
#[case(8.0, 12.0)]
#[case(5000.0, 1.0)]
fn test_default_interval_minutes_given_any_ride_stays_inside_the_spin(
    #[case] lap_km: f64,
    #[case] avg_speed_kmh: f64,
) {
    // Arrange
    let minimum = INTERVAL_MIN_MINUTES;
    let maximum = INTERVAL_MAX_MINUTES;

    // Act
    let minutes = default_interval_minutes(lap_km, avg_speed_kmh, minimum, maximum);

    // Assert
    assert!((minimum..=maximum).contains(&minutes));
}

// ---------------------------------------------------- check_message

/// The Check message's opening paragraph, spelled once.
const CHECK_MESSAGE_HEAD: &str = "Team riders = teams × riders per team (2–4 per team). \
     Solo riders = riders − team riders.\n";

/// Return the Check message for *riders* and *teams* under 2..4 teams.
fn check(riders: i64, teams: i64) -> String {
    check_message(riders, teams, 2, DEFAULT_MAX_TEAM_SIZE)
}

#[test]
fn test_check_message_given_full_teams_names_the_remainder_as_solo() {
    // Arrange
    let expected = format!(
        "{CHECK_MESSAGE_HEAD}With 175 riders and 40 teams, team riders must be between 80 and 160; \
         this field fills the teams to 160, so solo riders = 15. Ready to generate."
    );

    // Act
    let message = check(175, 40);

    // Assert
    assert_eq!(message, expected);
}

#[test]
fn test_check_message_given_teams_below_the_ceiling_clears_solo() {
    // Arrange
    let expected = format!(
        "{CHECK_MESSAGE_HEAD}With 100 riders and 40 teams, team riders must be between 80 and 160; \
         this field fills the teams to 100, so solo riders = 0. Ready to generate."
    );

    // Act
    let message = check(100, 40);

    // Assert
    assert_eq!(message, expected);
}

#[test]
fn test_check_message_given_too_few_riders_names_both_corrections() {
    // Arrange
    let expected = format!(
        "{CHECK_MESSAGE_HEAD}With 10 riders and 8 teams, team riders must be between 16 and 32, \
         but the field has only 10 riders.\n\
         Increase Number of riders to at least 16 or reduce Number of teams to 5."
    );

    // Act
    let message = check(10, 8);

    // Assert
    assert_eq!(message, expected);
}

#[test]
fn test_check_message_given_riders_below_one_team_names_the_rider_fix_only() {
    // Arrange
    let expected = format!(
        "{CHECK_MESSAGE_HEAD}With 1 rider and 2 teams, team riders must be between 4 and 8, \
         but the field has only 1 rider.\n\
         Increase Number of riders to at least 4."
    );

    // Act
    let message = check(1, 2);

    // Assert
    assert_eq!(message, expected);
}

#[test]
fn test_check_message_given_one_team_reads_the_team_noun_singular() {
    // Arrange
    let expected = format!(
        "{CHECK_MESSAGE_HEAD}With 2 riders and 1 team, team riders must be between 2 and 4; \
         this field fills the teams to 2, so solo riders = 0. Ready to generate."
    );

    // Act
    let message = check(2, 1);

    // Assert
    assert_eq!(message, expected);
}

#[test]
fn test_check_message_given_custom_team_bounds_names_them() {
    // Arrange
    let expected = "Team riders = teams × riders per team (3–5 per team). \
                    Solo riders = riders − team riders.\n\
                    With 16 riders and 3 teams, team riders must be between 9 and 15; \
                    this field fills the teams to 15, so solo riders = 1. Ready to generate.";

    // Act
    let message = check_message(16, 3, 3, 5);

    // Assert
    assert_eq!(message, expected);
}

// ------------------------------------------------------ generate_teams

#[test]
fn test_generate_teams_given_three_creates_named_team_entries() -> Result<(), String> {
    // Arrange
    let mut roster = draft_roster(PlateModel::RiderPooled)?;

    // Act
    generate_teams(&mut roster, 3)?;

    // Assert
    let names: Vec<&str> = roster
        .entries()
        .iter()
        .map(|entry| entry.display_name.as_str())
        .collect();
    assert_eq!(names, vec!["TEAM-0001", "TEAM-0002", "TEAM-0003"]);
    Ok(())
}

#[test]
fn test_generate_teams_given_zero_creates_no_entries() -> Result<(), String> {
    // Arrange
    let mut roster = draft_roster(PlateModel::RiderPooled)?;

    // Act
    generate_teams(&mut roster, 0)?;

    // Assert
    assert_eq!(roster.entries().len(), 0);
    Ok(())
}

#[test]
fn test_generate_teams_given_one_creates_the_first_named_team() -> Result<(), String> {
    // Arrange
    let mut roster = draft_roster(PlateModel::RiderPooled)?;

    // Act
    generate_teams(&mut roster, 1)?;

    // Assert
    assert_eq!(
        roster
            .entries()
            .first()
            .map(|entry| entry.display_name.as_str()),
        Some("TEAM-0001")
    );
    Ok(())
}

// ----------------------------------------------------- generate_riders

/// Return the standard 10-rider, 2-team, 4-solo input.
fn mixed_input() -> GeneratorInput {
    GeneratorInput {
        total: 10,
        teams: 2,
        solo: 4,
        seed: SEED,
    }
}

#[test]
fn test_generate_riders_given_a_mixed_pooled_ride_splits_solo_and_team_riders() -> Result<(), String>
{
    // Arrange
    let mut roster = draft_roster(PlateModel::RiderPooled)?;

    // Act
    generate_riders(&mut roster, &mixed_input())?;

    // Assert
    let solos = roster
        .entries()
        .iter()
        .filter(|entry| entry.kind == EntryType::Solo)
        .count();
    assert_eq!(solos, 4);
    assert_eq!(team_sizes(&roster), vec![3, 3]);
    assert_eq!(riders_of(&roster).len(), 10);
    Ok(())
}

#[test]
fn test_generate_riders_given_a_mixed_ride_keeps_every_team_between_the_size_bounds()
-> Result<(), String> {
    // Arrange
    let mut roster = draft_roster(PlateModel::RiderPooled)?;

    // Act
    generate_riders(&mut roster, &mixed_input())?;

    // Assert
    let sizes = team_sizes(&roster);
    assert!(sizes.iter().all(|size| (2..=4).contains(size)));
    Ok(())
}

#[test]
fn test_generate_riders_given_a_seed_names_every_rider_once() -> Result<(), String> {
    // Arrange
    let mut roster = draft_roster(PlateModel::RiderPooled)?;
    let expected: Vec<String> = (1..=10)
        .map(|number| format!("FIRSTNAME-{number:04}"))
        .collect();

    // Act
    generate_riders(&mut roster, &mixed_input())?;

    // Assert
    let mut names: Vec<String> = riders_of(&roster)
        .iter()
        .map(|rider| rider.first_name.clone())
        .collect();
    names.sort();
    assert_eq!(names, expected);
    Ok(())
}

#[test]
fn test_generate_riders_given_a_seed_gives_every_rider_a_sex() -> Result<(), String> {
    // Arrange
    let mut roster = draft_roster(PlateModel::RiderPooled)?;

    // Act
    generate_riders(&mut roster, &mixed_input())?;

    // Assert
    assert!(
        riders_of(&roster)
            .iter()
            .all(|rider| matches!(rider.sex.as_deref(), Some("M" | "F")))
    );
    Ok(())
}

#[test]
fn test_generate_riders_given_the_same_seed_generates_identical_rosters() -> Result<(), String> {
    // Arrange
    let mut first = draft_roster(PlateModel::RiderPooled)?;
    let mut second = draft_roster(PlateModel::RiderPooled)?;

    // Act
    generate_riders(&mut first, &mixed_input())?;
    generate_riders(&mut second, &mixed_input())?;

    // Assert
    assert_eq!(signature(&first), signature(&second));
    Ok(())
}

#[test]
fn test_generate_riders_given_solos_first_puts_their_plates_below_the_team_riders()
-> Result<(), String> {
    // Arrange
    let mut roster = draft_roster(PlateModel::RiderPooled)?;

    // Act
    generate_riders(&mut roster, &mixed_input())?;

    // Assert
    let mut solo_plates: Vec<u64> = roster
        .entries()
        .iter()
        .filter(|entry| entry.kind == EntryType::Solo)
        .filter_map(|entry| entry.plate.parse().ok())
        .collect();
    let mut team_plates: Vec<u64> = roster
        .entries()
        .iter()
        .filter(|entry| entry.kind == EntryType::Team)
        .flat_map(|entry| entry.riders.iter())
        .filter_map(|rider| rider.plate.as_deref().and_then(|plate| plate.parse().ok()))
        .collect();
    let solo_top = solo_plates.iter().copied().max().unwrap_or(0);
    let team_floor = team_plates.iter().copied().min().unwrap_or(0);
    solo_plates.sort_unstable();
    team_plates.sort_unstable();
    assert!(solo_top < team_floor);
    Ok(())
}

#[test]
fn test_generate_riders_given_existing_teams_fills_them_without_creating_more() -> Result<(), String>
{
    // Arrange
    let mut roster = draft_roster(PlateModel::RiderPooled)?;
    generate_teams(&mut roster, 2)?;

    // Act
    generate_riders(
        &mut roster,
        &GeneratorInput {
            total: 8,
            teams: 2,
            solo: 2,
            seed: SEED,
        },
    )?;

    // Assert
    assert_eq!(roster.entries().len(), 4);
    let names: Vec<&str> = roster
        .entries()
        .iter()
        .filter(|entry| entry.kind == EntryType::Team)
        .map(|entry| entry.display_name.as_str())
        .collect();
    assert_eq!(names, vec!["TEAM-0001", "TEAM-0002"]);
    Ok(())
}

#[test]
fn test_generate_riders_given_a_relay_ride_leaves_the_team_riders_plateless() -> Result<(), String>
{
    // Arrange
    let mut roster = draft_roster(PlateModel::TeamRelay)?;

    // Act
    generate_riders(
        &mut roster,
        &GeneratorInput {
            total: 6,
            teams: 2,
            solo: 2,
            seed: SEED,
        },
    )?;

    // Assert
    let plated: Vec<Option<String>> = roster
        .entries()
        .iter()
        .filter(|entry| entry.kind == EntryType::Team)
        .flat_map(|entry| entry.riders.iter().map(|rider| rider.plate.clone()))
        .collect();
    assert_eq!(plated, vec![None, None, None, None]);
    Ok(())
}

#[rstest]
#[case(2, 1, 0, &[2])]
#[case(4, 2, 0, &[2, 2])]
#[case(7, 2, 1, &[3, 3])]
#[case(8, 2, 0, &[4, 4])]
#[case(6, 3, 0, &[2, 2, 2])]
#[case(12, 3, 0, &[4, 4, 4])]
fn test_generate_riders_given_a_boundary_accepts_every_team_rider_count(
    #[case] total: i64,
    #[case] teams: i64,
    #[case] solo: i64,
    #[case] per_team: &[usize],
) -> Result<(), String> {
    // Arrange
    let mut roster = draft_roster(PlateModel::RiderPooled)?;

    // Act
    generate_riders(
        &mut roster,
        &GeneratorInput {
            total,
            teams,
            solo,
            seed: SEED,
        },
    )?;

    // Assert
    assert_eq!(team_sizes(&roster), per_team.to_vec());
    assert_eq!(
        riders_of(&roster).len(),
        usize::try_from(total).unwrap_or(0)
    );
    Ok(())
}

#[rstest]
#[case(0, 2, 0, "total must be at least 1")]
#[case(-1, 2, 0, "total must be at least 1")]
#[case(10, 0, 0, "teams must be at least 1")]
#[case(10, -1, 0, "teams must be at least 1")]
#[case(10, 2, -1, "solo must be between 0 and 10")]
#[case(10, 2, 11, "solo must be between 0 and 10")]
#[case(10, 2, 10, "team riders must be between 4 and 8, got 0")]
#[case(10, 2, 7, "team riders must be between 4 and 8, got 3")]
#[case(10, 2, 1, "team riders must be between 4 and 8, got 9")]
fn test_generate_riders_given_a_refused_input_names_the_bound_and_writes_nothing(
    #[case] total: i64,
    #[case] teams: i64,
    #[case] solo: i64,
    #[case] message: &str,
) -> Result<(), String> {
    // Arrange
    let mut roster = draft_roster(PlateModel::RiderPooled)?;

    // Act
    let result = generate_riders(
        &mut roster,
        &GeneratorInput {
            total,
            teams,
            solo,
            seed: SEED,
        },
    );

    // Assert
    assert_eq!(result, Err(message.to_owned()));
    assert_eq!(roster.entries().len(), 0);
    Ok(())
}

// ------------------------------------------------ generate_solo_riders

#[test]
fn test_generate_solo_riders_given_five_creates_five_numbered_solo_entries() -> Result<(), String> {
    // Arrange
    let mut roster = draft_roster(PlateModel::RiderPooled)?;
    let expected: Vec<String> = (1..=5)
        .map(|number| format!("FIRSTNAME-{number:04} LASTNAME-{number:04}"))
        .collect();

    // Act
    generate_solo_riders(&mut roster, 5, SEED)?;

    // Assert
    let names: Vec<&str> = roster
        .entries()
        .iter()
        .map(|entry| entry.display_name.as_str())
        .collect();
    assert_eq!(names, expected);
    Ok(())
}

#[test]
fn test_generate_solo_riders_given_five_plates_every_entry_once() -> Result<(), String> {
    // Arrange
    let mut roster = draft_roster(PlateModel::RiderPooled)?;

    // Act
    generate_solo_riders(&mut roster, 5, SEED)?;

    // Assert
    let plates: Vec<&str> = roster
        .entries()
        .iter()
        .map(|entry| entry.plate.as_str())
        .collect();
    assert!(plates.iter().all(|plate| !plate.is_empty()));
    assert_eq!(
        plates
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        5
    );
    Ok(())
}

#[test]
fn test_generate_solo_riders_given_a_seed_gives_every_rider_a_sex() -> Result<(), String> {
    // Arrange
    let mut roster = draft_roster(PlateModel::RiderPooled)?;

    // Act
    generate_solo_riders(&mut roster, 5, SEED)?;

    // Assert
    assert!(
        riders_of(&roster)
            .iter()
            .all(|rider| matches!(rider.sex.as_deref(), Some("M" | "F")))
    );
    Ok(())
}

#[test]
fn test_generate_solo_riders_given_the_same_seed_generates_identical_rosters() -> Result<(), String>
{
    // Arrange
    let mut first = draft_roster(PlateModel::RiderPooled)?;
    let mut second = draft_roster(PlateModel::RiderPooled)?;

    // Act
    generate_solo_riders(&mut first, 5, SEED)?;
    generate_solo_riders(&mut second, 5, SEED)?;

    // Assert
    assert_eq!(signature(&first), signature(&second));
    Ok(())
}

#[rstest]
#[case(1)]
#[case(2)]
#[case(5)]
fn test_generate_solo_riders_given_a_total_creates_that_many_entries(
    #[case] total: i64,
) -> Result<(), String> {
    // Arrange
    let mut roster = draft_roster(PlateModel::RiderPooled)?;

    // Act
    generate_solo_riders(&mut roster, total, SEED)?;

    // Assert
    assert_eq!(roster.entries().len(), usize::try_from(total).unwrap_or(0));
    Ok(())
}

#[rstest]
#[case(0)]
#[case(-1)]
fn test_generate_solo_riders_given_a_total_below_one_refuses(
    #[case] total: i64,
) -> Result<(), String> {
    // Arrange
    let mut roster = draft_roster(PlateModel::RiderPooled)?;

    // Act
    let result = generate_solo_riders(&mut roster, total, SEED);

    // Assert
    assert_eq!(result, Err("total must be at least 1".to_owned()));
    assert_eq!(roster.entries().len(), 0);
    Ok(())
}

// --------------------------------------------------------------- validate

#[rstest]
#[case(0, 1, Some("laps must be at least 1"))]
#[case(-1, 1, Some("laps must be at least 1"))]
#[case(1, 0, Some("interval must be at least 1 minute"))]
#[case(1, -5, Some("interval must be at least 1 minute"))]
#[case(
    0,
    0,
    Some("laps must be at least 1; interval must be at least 1 minute")
)]
#[case(1, 1, None)]
#[case(2, 1, None)]
#[case(1, 2, None)]
fn test_validate_returns_the_joined_refusal_or_none(
    #[case] laps: i64,
    #[case] interval_minutes: i64,
    #[case] expected: Option<&str>,
) {
    // Arrange
    let expected = expected.map(str::to_owned);

    // Act
    let result = validate(laps, interval_minutes);

    // Assert
    assert_eq!(result, expected);
}

// -------------------------------------------------------- run_simulation

#[test]
fn test_run_simulation_records_one_crossing_per_entry_per_lap() -> Result<(), String> {
    // Arrange
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &mixed_input())?;

    // Act
    let outcome = run(&mut engine, &roster, &settings(2, 1));

    // Assert
    assert_eq!(
        outcome,
        SimOutcome {
            cancelled: false,
            recorded: 12,
            blocked: None,
        }
    );
    assert_eq!(engine.crossings().len(), 12);
    assert!(engine.snapshot().iter().all(|entry| entry.laps == 2));
    Ok(())
}

#[test]
fn test_run_simulation_given_pooled_teams_keeps_team_laps_equal_to_solo_laps() -> Result<(), String>
{
    // Arrange
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &mixed_input())?;

    // Act
    run(&mut engine, &roster, &settings(12, 45));

    // Assert
    let laps: Vec<u32> = engine.snapshot().iter().map(|entry| entry.laps).collect();
    assert_eq!(laps, vec![12, 12, 12, 12, 12, 12]);
    Ok(())
}

#[test]
fn test_run_simulation_given_one_lap_opens_the_first_wave_one_interval_after_the_gun()
-> Result<(), String> {
    // Arrange
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &mixed_input())?;

    // Act
    run(&mut engine, &roster, &settings(1, 45));

    // Assert
    let gun = engine
        .actual_start()
        .ok_or_else(|| "the ride never started".to_owned())?;
    let first = engine
        .crossings()
        .iter()
        .map(|crossing| crossing.crossed_at)
        .min()
        .ok_or_else(|| "no crossing".to_owned())?;
    assert_eq!(first, after_minutes(gun, 45)?);
    Ok(())
}

#[test]
fn test_run_simulation_backdates_the_gun_by_the_whole_race() -> Result<(), String> {
    // Arrange
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &mixed_input())?;
    let before = SystemClock.now();

    // Act
    run(&mut engine, &roster, &settings(12, 45));
    let after = SystemClock.now();

    // Assert: the gun sits the race plus a half-interval back.
    let gun = engine
        .actual_start()
        .ok_or_else(|| "the ride never started".to_owned())?;
    let race = 12 * 45 * SECONDS_PER_MINUTE + 22 * SECONDS_PER_MINUTE + 30;
    let earliest = Timestamp::from_unix_timestamp(before.unix_timestamp() - race - 2)
        .map_err(|error| error.to_string())?;
    let latest = Timestamp::from_unix_timestamp(after.unix_timestamp() - race + 2)
        .map_err(|error| error.to_string())?;
    assert!(gun >= earliest);
    assert!(gun <= latest);
    Ok(())
}

#[test]
fn test_run_simulation_given_three_laps_leaves_no_zero_lap_time() -> Result<(), String> {
    // Arrange
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &mixed_input())?;

    // Act
    run(&mut engine, &roster, &settings(3, 45));

    // Assert: lap 1's leader clears a full interval.
    let quickest = roster
        .entries()
        .iter()
        .flat_map(|entry| engine.lap_times(&entry.id()))
        .fold(f64::MAX, f64::min);
    assert_eq!(quickest, 45.0 * 60.0);
    Ok(())
}

#[test]
fn test_run_simulation_given_two_laps_keeps_the_wave_inside_the_first_half_interval()
-> Result<(), String> {
    // Arrange
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &mixed_input())?;

    // Act
    run(&mut engine, &roster, &settings(2, 45));

    // Assert
    let gun = engine
        .actual_start()
        .ok_or_else(|| "the ride never started".to_owned())?;
    let second = engine
        .crossings()
        .iter()
        .filter(|crossing| crossing.seq == 2)
        .map(|crossing| crossing.crossed_at)
        .collect::<Vec<_>>();
    let base = after_minutes(gun, 90)?;
    assert_eq!(second.iter().min().copied(), Some(base));
    assert_eq!(second.iter().max().copied(), Some(after_minutes(gun, 112)?));
    Ok(())
}

#[test]
fn test_run_simulation_given_three_laps_keeps_the_same_field_order_every_lap() -> Result<(), String>
{
    // Arrange
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &mixed_input())?;

    // Act
    run(&mut engine, &roster, &settings(3, 1));

    // Assert
    let ids: Vec<&str> = engine
        .crossings()
        .iter()
        .map(|crossing| crossing.entry_id.0.as_str())
        .collect();
    let chunks: Vec<&[&str]> = ids.chunks(6).collect();
    let first = chunks.first().copied();
    assert_eq!(chunks.len(), 3);
    assert!(chunks.iter().all(|chunk| Some(*chunk) == first));
    Ok(())
}

#[test]
fn test_run_simulation_appends_start_and_stop_events() -> Result<(), String> {
    // Arrange
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &mixed_input())?;

    // Act
    run(&mut engine, &roster, &settings(1, 45));

    // Assert
    let actions: Vec<rivercrossing_ride::EventAction> =
        engine.events().iter().map(|event| event.action).collect();
    assert_eq!(
        actions.first().copied(),
        Some(rivercrossing_ride::EventAction::Start)
    );
    assert_eq!(
        actions.last().copied(),
        Some(rivercrossing_ride::EventAction::Stop)
    );
    Ok(())
}

#[test]
fn test_run_simulation_given_a_pooled_team_rotates_its_representative_each_lap()
-> Result<(), String> {
    // Arrange
    let input = GeneratorInput {
        total: 2,
        teams: 1,
        solo: 0,
        seed: SEED,
    };
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &input)?;
    let team_riders: Vec<Option<String>> = roster
        .entries()
        .iter()
        .flat_map(|entry| entry.riders.iter().map(|rider| rider.plate.clone()))
        .collect();

    // Act
    run(&mut engine, &roster, &settings(3, 1));

    // Assert
    let plates: Vec<Option<String>> = engine
        .crossings()
        .iter()
        .map(|crossing| crossing.rider_plate.clone())
        .collect();
    assert_eq!(
        plates,
        vec![
            team_riders.first().cloned().flatten(),
            team_riders.get(1).cloned().flatten(),
            team_riders.first().cloned().flatten(),
        ]
    );
    Ok(())
}

#[test]
fn test_run_simulation_leaves_the_ride_running_and_stopped() -> Result<(), String> {
    // Arrange
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &mixed_input())?;

    // Act
    run(&mut engine, &roster, &settings(1, 1));

    // Assert
    assert_eq!(engine.state(), RideStatus::Running);
    assert!(engine.stopped());
    Ok(())
}

#[test]
fn test_run_simulation_given_a_solo_reads_every_lap_as_one_interval() -> Result<(), String> {
    // Arrange
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &mixed_input())?;
    let solo = roster
        .entries()
        .iter()
        .find(|entry| entry.kind == EntryType::Solo)
        .ok_or_else(|| "no solo entry".to_owned())?;

    // Act
    run(&mut engine, &roster, &settings(3, 1));

    // Assert
    assert_eq!(engine.lap_times(&solo.id()), vec![60.0, 60.0, 60.0]);
    Ok(())
}

#[test]
fn test_run_simulation_reports_monotonic_progress_to_one() -> Result<(), String> {
    // Arrange
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &mixed_input())?;
    let mut progress = Vec::new();
    let mut record = |fraction: f32| progress.push(fraction);

    // Act
    let outcome = run_simulation(&mut engine, &roster, &settings(2, 1), &mut record, &|| {
        false
    });

    // Assert
    assert_eq!(progress.len(), 12);
    assert_eq!(progress.last().copied(), Some(1.0));
    assert!(progress.windows(2).all(|pair| pair.first() <= pair.get(1)));
    assert_eq!(outcome.recorded, 12);
    Ok(())
}

#[test]
fn test_run_simulation_given_an_uncancelled_predicate_runs_to_the_end() -> Result<(), String> {
    // Arrange
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &mixed_input())?;

    // Act
    let outcome = run_simulation(
        &mut engine,
        &roster,
        &settings(1, 1),
        &mut |_fraction| {},
        &|| false,
    );

    // Assert
    assert_eq!(outcome.recorded, 6);
    assert!(!outcome.cancelled);
    Ok(())
}

#[test]
fn test_run_simulation_given_a_firing_cancel_stops_after_the_first_crossing() -> Result<(), String>
{
    // Arrange
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &mixed_input())?;

    // Act
    let outcome = run_simulation(
        &mut engine,
        &roster,
        &settings(2, 1),
        &mut |_fraction| {},
        &|| true,
    );

    // Assert
    assert_eq!(
        outcome,
        SimOutcome {
            cancelled: true,
            recorded: 1,
            blocked: None,
        }
    );
    assert_eq!(engine.state(), RideStatus::Running);
    assert!(engine.stopped());
    Ok(())
}

#[test]
fn test_run_simulation_given_an_empty_roster_is_blocked_with_the_engine_reason()
-> Result<(), String> {
    // Arrange
    let roster = draft_roster(PlateModel::RiderPooled)?;
    let mut engine = engine_over(&fixture_config(PlateModel::RiderPooled, 1, 8)?, &roster)?;

    // Act
    let outcome = run(&mut engine, &roster, &settings(1, 1));

    // Assert
    assert_eq!(
        outcome,
        SimOutcome {
            cancelled: false,
            recorded: 0,
            blocked: Some(vec!["roster has no riders".to_owned()]),
        }
    );
    assert_eq!(engine.state(), RideStatus::Draft);
    Ok(())
}

#[test]
fn test_run_simulation_given_a_team_below_the_floor_is_blocked_naming_the_plate()
-> Result<(), String> {
    // Arrange
    let mut roster = draft_roster(PlateModel::RiderPooled)?;
    let team = roster
        .create_empty_team(TeamFields {
            display_name: "TEAM-0001",
            riders: Vec::new(),
            plate: None,
            logo_card: None,
        })
        .map_err(|error| error.to_string())?;
    let rider_plate = roster.next_free_plate();
    roster
        .add_rider_to_team(
            Rider {
                first_name: "A".to_owned(),
                last_name: "B".to_owned(),
                plate: Some(rider_plate),
                sex: Some("M".to_owned()),
                sort_order: 0,
            },
            &team,
        )
        .map_err(|error| error.to_string())?;
    let plate = roster
        .entries()
        .first()
        .map(|entry| entry.plate.clone())
        .ok_or_else(|| "no team".to_owned())?;
    let mut engine = engine_over(&fixture_config(PlateModel::RiderPooled, 1, 8)?, &roster)?;

    // Act
    let outcome = run(&mut engine, &roster, &settings(1, 1));

    // Assert
    assert_eq!(
        outcome.blocked,
        Some(vec![format!(
            "{plate}: team size must be at least 2, got 1"
        )])
    );
    assert_eq!(outcome.recorded, 0);
    Ok(())
}

#[test]
fn test_run_simulation_given_a_finished_engine_is_blocked_with_the_state_refusal()
-> Result<(), String> {
    // Arrange
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &mixed_input())?;
    engine.start(None).map_err(|error| error.to_string())?;
    engine.finish(&[]).map_err(|error| error.to_string())?;

    // Act
    let outcome = run(&mut engine, &roster, &settings(1, 1));

    // Assert
    assert_eq!(
        outcome.blocked,
        Some(vec![
            "illegal ride state: cannot start from finished".to_owned()
        ])
    );
    assert_eq!(outcome.recorded, 0);
    Ok(())
}

#[test]
fn test_run_simulation_given_a_relay_ride_records_the_entry_plate() -> Result<(), String> {
    // Arrange
    let input = GeneratorInput {
        total: 6,
        teams: 2,
        solo: 2,
        seed: SEED,
    };
    let (roster, mut engine) = generated(PlateModel::TeamRelay, 1, &input)?;

    // Act
    let outcome = run(&mut engine, &roster, &settings(1, 1));

    // Assert
    assert_eq!(outcome.recorded, 4);
    let mut crossed: Vec<&str> = engine
        .crossings()
        .iter()
        .map(|crossing| crossing.entry_id.0.as_str())
        .collect();
    let mut entries: Vec<&str> = roster
        .entries()
        .iter()
        .map(|entry| entry.key.as_str())
        .collect();
    crossed.sort_unstable();
    entries.sort_unstable();
    assert_eq!(crossed, entries);
    Ok(())
}

#[test]
fn test_run_simulation_given_a_relay_ride_keeps_team_laps_equal_to_solo_laps() -> Result<(), String>
{
    // Arrange
    let input = GeneratorInput {
        total: 6,
        teams: 2,
        solo: 2,
        seed: SEED,
    };
    let (roster, mut engine) = generated(PlateModel::TeamRelay, 1, &input)?;

    // Act
    run(&mut engine, &roster, &settings(3, 1));

    // Assert
    let laps: Vec<u32> = engine.snapshot().iter().map(|entry| entry.laps).collect();
    assert_eq!(laps, vec![3, 3, 3, 3]);
    Ok(())
}

#[test]
fn test_run_simulation_given_the_same_seed_replays_the_same_race() -> Result<(), String> {
    // Arrange
    let (first_roster, mut first_engine) = generated(PlateModel::RiderPooled, 1, &mixed_input())?;
    let (second_roster, mut second_engine) = generated(PlateModel::RiderPooled, 1, &mixed_input())?;

    // Act
    run(&mut first_engine, &first_roster, &settings(2, 45));
    run(&mut second_engine, &second_roster, &settings(2, 45));

    // Assert
    assert_eq!(
        race(&first_engine, &first_roster),
        race(&second_engine, &second_roster)
    );
    Ok(())
}

#[test]
fn test_run_simulation_given_a_plate_the_engine_does_not_hold_counts_only_accepted()
-> Result<(), String> {
    // Arrange: the engine holds the empty field, while the roster the script
    // reads gains one rider after the engine was built, so that plate is
    // refused and only the accepted crossings count.
    let mut roster = draft_roster(PlateModel::RiderPooled)?;
    generate_riders(
        &mut roster,
        &GeneratorInput {
            total: 2,
            teams: 1,
            solo: 0,
            seed: SEED,
        },
    )?;
    let config = fixture_config(PlateModel::RiderPooled, 1, 8)?;
    let mut engine = engine_over(&config, &roster)?;
    generate_solo_riders(&mut roster, 1, SEED)?;

    // Act
    let outcome = run(&mut engine, &roster, &settings(1, 1));

    // Assert
    assert_eq!(outcome.recorded, 1);
    assert_eq!(engine.crossings().len(), 1);
    Ok(())
}

// ------------------------------------------- the short-lap behaviour

#[rstest]
#[case(0, 1)]
#[case(1, 1)]
#[case(30, 1)]
#[case(31, 1)]
#[case(32, 2)]
#[case(1080, 1050)]
fn test_short_lap_gap_given_a_minimum_returns_the_clamped_margin(
    #[case] min_lap_s: u32,
    #[case] expected: i64,
) {
    // Act
    let gap = short_lap_gap(min_lap_s);

    // Assert
    assert_eq!(gap, expected);
}

#[test]
fn test_run_simulation_given_a_short_lap_rider_flags_and_holds_every_lap() -> Result<(), String> {
    // Arrange
    let mut roster = draft_roster(PlateModel::RiderPooled)?;
    generate_solo_riders(&mut roster, 1, SEED)?;
    let mut engine = engine_over(&fixture_config(PlateModel::RiderPooled, 1_080, 8)?, &roster)?;
    let mut settings = settings(3, 45);
    settings.short_laps = 1;

    // Act
    let outcome = run(&mut engine, &roster, &settings);

    // Assert
    let entry = roster
        .entries()
        .first()
        .ok_or_else(|| "no entry".to_owned())?;
    assert_eq!(
        engine.lap_times(&entry.id()),
        vec![1_050.0, 1_050.0, 1_050.0]
    );
    assert_eq!(engine.held_crossings().len(), 3);
    assert_eq!(outcome.recorded, 3);
    Ok(())
}

#[test]
fn test_run_simulation_given_a_short_lap_rider_and_always_deal_credits_its_cards()
-> Result<(), String> {
    // Arrange
    let mut roster = draft_roster(PlateModel::RiderPooled)?;
    generate_solo_riders(&mut roster, 1, SEED)?;
    let mut config = fixture_config(PlateModel::RiderPooled, 1_080, 8)?;
    config.hold_short_laps = false;
    let mut engine = engine_over(&config, &roster)?;
    let mut settings = settings(3, 45);
    settings.short_laps = 1;

    // Act
    run(&mut engine, &roster, &settings);

    // Assert
    let entry = roster
        .entries()
        .first()
        .ok_or_else(|| "no entry".to_owned())?;
    assert_eq!(engine.held_crossings().len(), 0);
    assert_eq!(engine.credited_cards(&entry.id()).len(), 3);
    Ok(())
}

#[test]
fn test_run_simulation_given_short_lap_riders_shortens_the_orders_leading_entries()
-> Result<(), String> {
    // Arrange: the one-minute minimum equals the interval, so an unshortened
    // lap lands on the floor and is not flagged; only the shortened entries
    // are held.
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 60, &mixed_input())?;
    let mut settings = settings(2, 1);
    settings.short_laps = 2;

    // Act
    run(&mut engine, &roster, &settings);

    // Assert: exactly two entries carry the thirty-second gap, and every other
    // entry keeps the interval's minute.
    let held = engine.held_crossings();
    let flagged: std::collections::HashSet<&str> = held
        .iter()
        .map(|held| held.crossing.entry_id.0.as_str())
        .collect();
    assert_eq!(flagged.len(), 2);
    assert_eq!(held.len(), 4);
    assert!(flagged.iter().all(|key| {
        let entry = EntryId((*key).to_owned());
        engine.lap_times(&entry) == vec![30.0, 30.0]
    }));
    assert!(
        roster
            .entries()
            .iter()
            .filter(|entry| !flagged.contains(entry.key.as_str()))
            .all(|entry| engine.lap_times(&entry.id()) == vec![60.0, 60.0])
    );
    Ok(())
}

#[test]
fn test_run_simulation_given_no_short_lap_riders_flags_nothing() -> Result<(), String> {
    // Arrange
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1_080, &mixed_input())?;

    // Act
    run(&mut engine, &roster, &settings(2, 45));

    // Assert
    assert_eq!(engine.held_crossings().len(), 0);
    Ok(())
}

// ---------------------------------------------- the lapped behaviour

#[test]
fn test_run_simulation_given_a_lapped_rider_finishes_it_one_lap_behind() -> Result<(), String> {
    // Arrange
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &mixed_input())?;
    let mut settings = settings(3, 45);
    settings.lapped = 1;

    // Act
    let outcome = run(&mut engine, &roster, &settings);

    // Assert
    let mut laps: Vec<u32> = engine.snapshot().iter().map(|entry| entry.laps).collect();
    laps.sort_unstable();
    assert_eq!(laps, vec![2, 3, 3, 3, 3, 3]);
    assert_eq!(outcome.recorded, 17);
    Ok(())
}

#[test]
fn test_run_simulation_given_a_lapped_rider_skips_only_the_first_wave() -> Result<(), String> {
    // Arrange
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &mixed_input())?;
    let mut settings = settings(3, 45);
    settings.lapped = 1;

    // Act
    run(&mut engine, &roster, &settings);

    // Assert: the leading position rides at the wave's own opening, so its two
    // crossings sit two and three intervals after the gun.
    let gun = engine
        .actual_start()
        .ok_or_else(|| "the ride never started".to_owned())?;
    let behind = engine
        .snapshot()
        .iter()
        .filter(|entry| entry.laps == 2)
        .map(|entry| entry.entry_id.clone())
        .next()
        .ok_or_else(|| "no lapped entry".to_owned())?;
    let instants: Vec<Timestamp> = engine
        .crossings()
        .iter()
        .filter(|crossing| crossing.entry_id == behind)
        .map(|crossing| crossing.crossed_at)
        .collect();
    assert_eq!(
        instants,
        vec![after_minutes(gun, 90)?, after_minutes(gun, 135)?]
    );
    Ok(())
}

#[test]
fn test_run_simulation_given_no_lapped_riders_records_every_first_wave() -> Result<(), String> {
    // Arrange
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &mixed_input())?;

    // Act
    let outcome = run(&mut engine, &roster, &settings(3, 45));

    // Assert
    assert_eq!(outcome.recorded, 18);
    Ok(())
}

// -------------------------------------------- the team-stop behaviour

#[rstest]
#[case(4, &[0, 1, 2, 3])]
#[case(5, &[0, 1, 2, 3, 1])]
fn test_run_simulation_given_a_stopped_team_rider_swaps_it_out_from_lap_five(
    #[case] laps: i64,
    #[case] expected_offsets: &[usize],
) -> Result<(), String> {
    // Arrange
    let input = GeneratorInput {
        total: 4,
        teams: 1,
        solo: 0,
        seed: SEED,
    };
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &input)?;
    let team = roster
        .entries()
        .first()
        .ok_or_else(|| "no team".to_owned())?;
    let rider_plates: Vec<Option<String>> = team
        .riders
        .iter()
        .map(|rider| rider.plate.clone())
        .collect();
    let expected: Vec<Option<String>> = expected_offsets
        .iter()
        .filter_map(|offset| rider_plates.get(*offset).cloned())
        .collect();
    let mut settings = settings(laps, 1);
    settings.team_stop = 1;

    // Act
    run(&mut engine, &roster, &settings);

    // Assert
    let plates: Vec<Option<String>> = engine
        .crossings()
        .iter()
        .map(|crossing| crossing.rider_plate.clone())
        .collect();
    assert_eq!(plates, expected);
    Ok(())
}

#[test]
fn test_run_simulation_given_a_stopped_team_rider_never_records_it_after_lap_four()
-> Result<(), String> {
    // Arrange
    let input = GeneratorInput {
        total: 4,
        teams: 1,
        solo: 0,
        seed: SEED,
    };
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &input)?;
    let team = roster
        .entries()
        .first()
        .ok_or_else(|| "no team".to_owned())?;
    let stopped = team
        .riders
        .first()
        .and_then(|rider| rider.plate.clone())
        .ok_or_else(|| "no rider plate".to_owned())?;
    let mut settings = settings(8, 1);
    settings.team_stop = 1;

    // Act
    run(&mut engine, &roster, &settings);

    // Assert
    let after_four: Vec<Option<String>> = engine
        .crossings()
        .iter()
        .skip(4)
        .map(|crossing| crossing.rider_plate.clone())
        .collect();
    assert!(!after_four.contains(&Some(stopped)));
    assert_eq!(engine.snapshot().first().map(|entry| entry.laps), Some(8));
    Ok(())
}

#[test]
fn test_run_simulation_given_a_fully_stopped_team_records_nothing_after_lap_four()
-> Result<(), String> {
    // Arrange
    let input = GeneratorInput {
        total: 2,
        teams: 1,
        solo: 0,
        seed: SEED,
    };
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &input)?;
    let mut settings = settings(6, 1);
    settings.team_stop = 2;

    // Act
    let outcome = run(&mut engine, &roster, &settings);

    // Assert
    assert_eq!(outcome.recorded, 4);
    assert_eq!(engine.crossings().len(), 4);
    Ok(())
}

#[test]
fn test_run_simulation_given_a_fully_stopped_team_counts_skipped_positions_as_progress()
-> Result<(), String> {
    // Arrange
    let input = GeneratorInput {
        total: 2,
        teams: 1,
        solo: 0,
        seed: SEED,
    };
    let (roster, mut engine) = generated(PlateModel::RiderPooled, 1, &input)?;
    let mut settings = settings(6, 1);
    settings.team_stop = 2;
    let mut progress = Vec::new();
    let mut record = |fraction: f32| progress.push(fraction);

    // Act
    let outcome = run_simulation(&mut engine, &roster, &settings, &mut record, &|| false);

    // Assert
    assert_eq!(progress.last().copied(), Some(1.0));
    assert_eq!(outcome.recorded, 4);
    Ok(())
}

#[test]
fn test_run_simulation_given_a_relay_ride_keeps_team_stop_a_no_op() -> Result<(), String> {
    // Arrange
    let mut roster = draft_roster(PlateModel::TeamRelay)?;
    roster
        .create_team_entry(TeamFields {
            display_name: "TEAM-0001",
            riders: vec![
                Rider {
                    first_name: "A".to_owned(),
                    sex: Some("M".to_owned()),
                    ..Rider::default()
                },
                Rider {
                    first_name: "B".to_owned(),
                    sex: Some("M".to_owned()),
                    ..Rider::default()
                },
            ],
            plate: Some("11"),
            logo_card: None,
        })
        .map_err(|error| error.to_string())?;
    let mut engine = engine_over(&fixture_config(PlateModel::TeamRelay, 1, 8)?, &roster)?;
    let mut settings = settings(8, 1);
    settings.team_stop = 1;

    // Act
    run(&mut engine, &roster, &settings);

    // Assert
    let key = roster
        .entries()
        .first()
        .map(|entry| entry.key.as_str())
        .unwrap_or_default();
    assert_eq!(engine.crossings().len(), 8);
    assert!(
        engine
            .crossings()
            .iter()
            .all(|crossing| crossing.entry_id.0 == key)
    );
    Ok(())
}

#[test]
fn test_run_simulation_given_every_behaviour_and_one_seed_replays_the_same_race()
-> Result<(), String> {
    // Arrange
    let (first_roster, mut first_engine) =
        generated(PlateModel::RiderPooled, 1_080, &mixed_input())?;
    let (second_roster, mut second_engine) =
        generated(PlateModel::RiderPooled, 1_080, &mixed_input())?;
    let mut settings = settings(8, 45);
    settings.short_laps = 2;
    settings.lapped = 1;
    settings.team_stop = 2;

    // Act
    run(&mut first_engine, &first_roster, &settings);
    run(&mut second_engine, &second_roster, &settings);

    // Assert
    assert_eq!(
        race(&first_engine, &first_roster),
        race(&second_engine, &second_roster)
    );
    Ok(())
}
