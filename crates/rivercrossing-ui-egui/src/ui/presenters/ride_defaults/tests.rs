// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the seeded "GORBA Test Ride #N" defaults.
//!
//! The numbering rule, the module's fixed constants and the config the builder
//! returns are all pinned here, and the built config is put through the
//! minimum-setup rule so a test ride is proved to be one a DRAFT ride may
//! actually start.

use pretty_assertions::assert_eq;
use rivercrossing_cards::JokersMode;
use rivercrossing_ride::{EntryMode, PlateModel, RideConfig, TieBreak, setup_minimum_violations};
use rstest::rstest;
use time::{Date, Month, OffsetDateTime, Time};

use super::{
    DEFAULT_MAX_TEAM_SIZE, DEFAULT_MIN_LAP_S, DEFAULT_ORGANIZER, DEFAULT_PLANNED_DURATION_S,
    DEFAULT_SCORER, DEFAULT_VENUE, TEST_RIDE_NAME_PREFIX, build_test_ride_config,
    next_test_ride_name,
};

/// The identity every builder test hands in, fixed so a failure names a drift.
const NAME: &str = "GORBA Test Ride #7";

/// The decoys the numbering rule must read past: only a name carrying the
/// exact prefix with an ASCII decimal tail counts as a test ride.
const UNRELATED_NAME: &str = "GORBA EPIC 2026";
const LOWER_CASE_NAME: &str = "gorba test ride #9";
const INFIX_NAME: &str = "xGORBA Test Ride #9";
const UNPREFIXED_NAME: &str = "GORBA Test Ride 9";
const EMPTY_SUFFIX_NAME: &str = "GORBA Test Ride #";
const WORD_SUFFIX_NAME: &str = "GORBA Test Ride #x";
const MIXED_SUFFIX_NAME: &str = "GORBA Test Ride #9x";
const NEGATIVE_SUFFIX_NAME: &str = "GORBA Test Ride #-1";
const PADDED_SUFFIX_NAME: &str = "GORBA Test Ride # 9";
const TRAILING_SPACE_NAME: &str = "GORBA Test Ride #9 ";

/// Arabic-Indic nine: a decimal digit, but not an ASCII one.
const NON_ASCII_SUFFIX_NAME: &str = "GORBA Test Ride #٩";

/// Return *values* as the owned names the numbering rule reads.
fn names(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

/// Return the fixed event date that every builder test uses.
fn event_date() -> Result<Date, String> {
    Date::from_calendar_date(2026, Month::September, 20).map_err(|error| error.to_string())
}

/// Return the fixed planned gun that every builder test uses.
fn planned_start() -> Result<OffsetDateTime, String> {
    let time = Time::from_hms(10, 0, 0).map_err(|error| error.to_string())?;
    Ok(OffsetDateTime::new_utc(event_date()?, time))
}

/// Build the test ride's config from the fixed identity.
fn test_ride() -> Result<RideConfig, String> {
    Ok(build_test_ride_config(
        NAME,
        event_date()?,
        planned_start()?,
    ))
}

// --------------------------------------------------- the frozen prefix

#[test]
fn test_ride_defaults_prefix_is_the_gorba_test_ride_prefix() {
    // Arrange
    let expected = "GORBA Test Ride #";

    // Act
    let prefix = TEST_RIDE_NAME_PREFIX;

    // Assert
    assert_eq!(prefix, expected);
}

// ------------------------------------------------ next_test_ride_name

#[rstest]
#[case(&[], "GORBA Test Ride #1")]
#[case(&[UNRELATED_NAME], "GORBA Test Ride #1")]
#[case(&[LOWER_CASE_NAME], "GORBA Test Ride #1")]
#[case(&[INFIX_NAME], "GORBA Test Ride #1")]
#[case(&[UNPREFIXED_NAME], "GORBA Test Ride #1")]
#[case(&[EMPTY_SUFFIX_NAME], "GORBA Test Ride #1")]
#[case(&[WORD_SUFFIX_NAME], "GORBA Test Ride #1")]
#[case(&[MIXED_SUFFIX_NAME], "GORBA Test Ride #1")]
#[case(&[NEGATIVE_SUFFIX_NAME], "GORBA Test Ride #1")]
#[case(&[PADDED_SUFFIX_NAME], "GORBA Test Ride #1")]
#[case(&[TRAILING_SPACE_NAME], "GORBA Test Ride #1")]
#[case(&[NON_ASCII_SUFFIX_NAME], "GORBA Test Ride #1")]
#[case(&["GORBA Test Ride #1"], "GORBA Test Ride #2")]
#[case(&["GORBA Test Ride #0"], "GORBA Test Ride #1")]
#[case(&["GORBA Test Ride #99"], "GORBA Test Ride #100")]
#[case(&["GORBA Test Ride #1", "GORBA Test Ride #3"], "GORBA Test Ride #4")]
#[case(&["GORBA Test Ride #3", "GORBA Test Ride #1"], "GORBA Test Ride #4")]
#[case(&["GORBA Test Ride #2", UNRELATED_NAME, "GORBA Test Ride #5"], "GORBA Test Ride #6")]
#[case(&[WORD_SUFFIX_NAME, "GORBA Test Ride #2"], "GORBA Test Ride #3")]
#[case(&["GORBA Test Ride #1", "GORBA Test Ride #1"], "GORBA Test Ride #2")]
fn test_next_test_ride_name_given_names_returns_the_highest_suffix_plus_one(
    #[case] existing: &[&str],
    #[case] expected: &str,
) {
    // Arrange
    let existing = names(existing);

    // Act
    let result = next_test_ride_name(&existing);

    // Assert
    assert_eq!(result, expected);
}

#[test]
fn test_next_test_ride_name_given_a_bare_prefix_matches_nothing() {
    // Arrange
    let existing = names(&[TEST_RIDE_NAME_PREFIX]);

    // Act
    let result = next_test_ride_name(&existing);

    // Assert
    assert_eq!(result, "GORBA Test Ride #1");
}

#[test]
fn test_next_test_ride_name_given_the_largest_suffix_still_returns_a_larger_one() {
    // Arrange
    let existing = names(&["GORBA Test Ride #999999"]);

    // Act
    let result = next_test_ride_name(&existing);

    // Assert
    assert_eq!(result, "GORBA Test Ride #1000000");
}

#[test]
fn test_next_test_ride_name_given_registered_names_never_reuses_a_suffix() {
    // Arrange
    let existing = names(&["GORBA Test Ride #1", "GORBA Test Ride #8"]);

    // Act
    let result = next_test_ride_name(&existing);

    // Assert
    assert_eq!(result, "GORBA Test Ride #9");
}

#[test]
fn test_next_test_ride_name_given_the_returned_name_advances_by_exactly_one() {
    // Arrange
    let mut existing = names(&["GORBA Test Ride #3", "GORBA Test Ride #1"]);
    existing.push(next_test_ride_name(&existing));

    // Act
    let result = next_test_ride_name(&existing);

    // Assert
    assert_eq!(result, "GORBA Test Ride #5");
}

// --------------------------------------------- the default constants

#[test]
fn test_ride_defaults_identity_constants_pin_the_gorba_reference_ride() {
    // Arrange
    let expected = ("Sea to Sky Gondola", "GORBA", "K. Singh");

    // Act
    let constants = (DEFAULT_VENUE, DEFAULT_ORGANIZER, DEFAULT_SCORER);

    // Assert
    assert_eq!(constants, expected);
}

#[test]
fn test_ride_defaults_timing_constants_pin_the_six_hour_six_lap_ride() {
    // Arrange
    let expected = (21_600, 1_080);

    // Act
    let constants = (DEFAULT_PLANNED_DURATION_S, DEFAULT_MIN_LAP_S);

    // Assert
    assert_eq!(constants, expected);
}

#[test]
fn test_ride_defaults_team_size_constant_is_five() {
    // Arrange
    let expected = 5;

    // Act
    let value = DEFAULT_MAX_TEAM_SIZE;

    // Assert
    assert_eq!(value, expected);
}

// ------------------------------------------- build_test_ride_config

#[test]
fn test_build_test_ride_config_given_identity_arguments_returns_them_unchanged()
-> Result<(), String> {
    // Arrange
    let expected = (NAME.to_owned(), event_date()?, planned_start()?);

    // Act
    let config = test_ride()?;

    // Assert
    assert_eq!(
        (config.name, config.event_date, config.planned_start),
        expected
    );
    Ok(())
}

#[test]
fn test_build_test_ride_config_given_identity_arguments_uses_mixed_rider_pooled()
-> Result<(), String> {
    // Arrange
    let expected = (EntryMode::Mixed, PlateModel::RiderPooled, 5);

    // Act
    let config = test_ride()?;

    // Assert
    assert_eq!(
        (config.entry_mode, config.plate_model, config.max_team_size),
        expected
    );
    Ok(())
}

#[test]
fn test_build_test_ride_config_given_identity_arguments_uses_the_gorba_identity()
-> Result<(), String> {
    // Arrange
    let expected = (
        "Sea to Sky Gondola".to_owned(),
        "GORBA".to_owned(),
        "K. Singh".to_owned(),
    );

    // Act
    let config = test_ride()?;

    // Assert
    assert_eq!((config.venue, config.organizer, config.scorer), expected);
    Ok(())
}

#[test]
fn test_build_test_ride_config_given_identity_arguments_uses_the_eight_km_loop()
-> Result<(), String> {
    // Arrange
    let expected = (8.0, 21_600, 1_080);

    // Act
    let config = test_ride()?;

    // Assert
    assert_eq!(
        (config.lap_km, config.planned_duration_s, config.min_lap_s),
        expected
    );
    Ok(())
}

#[test]
fn test_build_test_ride_config_given_identity_arguments_uses_the_default_shoe() -> Result<(), String>
{
    // Arrange
    let expected = (8, 1, JokersMode::Total);

    // Act
    let config = test_ride()?;

    // Assert
    assert_eq!(
        (
            config.deck_count,
            config.jokers_per_deck,
            config.jokers_mode
        ),
        expected
    );
    Ok(())
}

#[test]
fn test_build_test_ride_config_given_identity_arguments_leaves_the_hand_uncapped()
-> Result<(), String> {
    // Arrange
    let expected = None;

    // Act
    let config = test_ride()?;

    // Assert
    assert_eq!(config.max_cards, expected);
    Ok(())
}

#[test]
fn test_build_test_ride_config_given_identity_arguments_keeps_the_default_tiebreak()
-> Result<(), String> {
    // Arrange
    let expected = [TieBreak::HighCard, TieBreak::Laps, TieBreak::TotalTime];

    // Act
    let config = test_ride()?;

    // Assert
    assert_eq!(config.tiebreak_order, expected);
    Ok(())
}

#[test]
fn test_build_test_ride_config_given_identity_arguments_holds_short_laps() -> Result<(), String> {
    // Arrange
    let expected = true;

    // Act
    let config = test_ride()?;

    // Assert
    assert_eq!(config.hold_short_laps, expected);
    Ok(())
}

#[test]
fn test_build_test_ride_config_given_identity_arguments_has_no_logo() -> Result<(), String> {
    // Arrange
    let expected = None;

    // Act
    let config = test_ride()?;

    // Assert
    assert_eq!(config.logo_path, expected);
    Ok(())
}

#[test]
fn test_build_test_ride_config_given_identity_arguments_passes_the_minimum_setup_rule()
-> Result<(), String> {
    // Arrange
    let config = test_ride()?;

    // Act
    let violations = setup_minimum_violations(&config);

    // Assert
    assert_eq!(violations, Vec::<String>::new());
    Ok(())
}
