// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

use pretty_assertions::assert_eq;
use rivercrossing_cards::JokersMode;
use rstest::rstest;
use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time};

use super::{
    EntryMode, PlateModel, RideConfig, RideConfigParams, TieBreak, setup_minimum_violations,
};
use crate::error::RideError;

fn fixture_date() -> Date {
    Date::from_calendar_date(2026, Month::September, 20).expect("the fixture date is valid")
}

fn fixture_start() -> OffsetDateTime {
    let time = Time::from_hms(10, 0, 0).expect("the fixture time is valid");
    PrimitiveDateTime::new(fixture_date(), time).assume_utc()
}

fn valid_params() -> RideConfigParams {
    RideConfigParams {
        name: "GORBA EPIC 2026".to_owned(),
        event_date: fixture_date(),
        venue: "Sea to Sky Gondola".to_owned(),
        lap_km: 8.0,
        organizer: "GORBA".to_owned(),
        scorer: "K. Singh".to_owned(),
        planned_start: fixture_start(),
        planned_duration_s: 21_600,
        min_lap_s: 1_080,
        entry_mode: EntryMode::Mixed,
        plate_model: PlateModel::RiderPooled,
        ..RideConfigParams::default()
    }
}

fn valid_config() -> RideConfig {
    RideConfig::new(valid_params()).expect("the fixture params are valid")
}

// --------------------------------------------------------------- defaults

#[test]
fn test_ride_config_defaults_max_team_size_to_four() {
    assert_eq!(valid_config().max_team_size, 4);
}

#[test]
fn test_ride_config_defaults_deck_count_to_eight() {
    assert_eq!(valid_config().deck_count, 8);
}

#[test]
fn test_ride_config_defaults_jokers_per_deck_to_one() {
    assert_eq!(valid_config().jokers_per_deck, 1);
}

#[test]
fn test_ride_config_defaults_jokers_mode_to_total() {
    assert_eq!(valid_config().jokers_mode, JokersMode::Total);
}

#[test]
fn test_ride_config_defaults_max_cards_to_uncapped() {
    assert_eq!(valid_config().max_cards, None);
}

#[test]
fn test_ride_config_defaults_tiebreak_order_to_high_card_first() {
    let expected = [TieBreak::HighCard, TieBreak::Laps, TieBreak::TotalTime];
    assert_eq!(valid_config().tiebreak_order, expected);
}

#[test]
fn test_ride_config_defaults_logo_path_to_none() {
    assert_eq!(valid_config().logo_path, None);
}

#[test]
fn test_ride_config_defaults_hold_short_laps_to_true() {
    assert_eq!(valid_config().hold_short_laps, true);
}

// ------------------------------------------------------- stored verbatim

#[test]
fn test_ride_config_given_a_logo_path_stores_it_verbatim() {
    let path = PathBuf::from("/tmp/gorba-logo.png");
    let params = RideConfigParams {
        logo_path: Some(path.clone()),
        ..valid_params()
    };

    let config = RideConfig::new(params).expect("the fixture params are valid");

    assert_eq!(config.logo_path, Some(path));
}

#[test]
fn test_ride_config_given_a_custom_tiebreak_order_stores_it_verbatim() {
    let order = [TieBreak::Laps, TieBreak::TotalTime, TieBreak::HighCard];
    let params = RideConfigParams {
        tiebreak_order: order,
        ..valid_params()
    };

    let config = RideConfig::new(params).expect("the fixture params are valid");

    assert_eq!(config.tiebreak_order, order);
}

#[test]
fn test_ride_config_given_every_required_field_stores_each_verbatim() {
    let config = valid_config();

    assert_eq!(
        (
            config.name.as_str(),
            config.venue.as_str(),
            config.organizer.as_str(),
            config.scorer.as_str(),
            config.event_date,
            config.planned_start,
            config.planned_duration_s,
            config.min_lap_s,
            config.entry_mode,
            config.plate_model,
            config.lap_km,
        ),
        (
            "GORBA EPIC 2026",
            "Sea to Sky Gondola",
            "GORBA",
            "K. Singh",
            fixture_date(),
            fixture_start(),
            21_600,
            1_080,
            EntryMode::Mixed,
            PlateModel::RiderPooled,
            8.0,
        )
    );
}

// ------------------------------------------------------ max_team_size bound

#[rstest]
#[case(1)]
#[case(11)]
fn test_ride_config_max_team_size_out_of_range_is_refused(#[case] value: i32) {
    let params = RideConfigParams {
        max_team_size: value,
        ..valid_params()
    };

    let result = RideConfig::new(params);

    assert_eq!(
        result,
        Err(RideError::Config {
            message: format!("max_team_size must be 2..10, got {value}"),
        })
    );
}

#[rstest]
#[case(2)]
#[case(3)]
#[case(9)]
#[case(10)]
fn test_ride_config_max_team_size_in_range_is_accepted(#[case] value: i32) {
    let params = RideConfigParams {
        max_team_size: value,
        ..valid_params()
    };

    let config = RideConfig::new(params).expect("the value is in range");

    assert_eq!(
        config.max_team_size,
        u32::try_from(value).expect("the fixture is non-negative")
    );
}

// --------------------------------------------------------- deck_count bound

#[rstest]
#[case(0)]
#[case(-1)]
fn test_ride_config_deck_count_below_one_is_refused(#[case] value: i32) {
    let params = RideConfigParams {
        deck_count: value,
        ..valid_params()
    };

    let result = RideConfig::new(params);

    assert_eq!(
        result,
        Err(RideError::Config {
            message: format!("deck_count must be >= 1, got {value}"),
        })
    );
}

#[rstest]
#[case(1)]
#[case(2)]
#[case(8)]
fn test_ride_config_deck_count_at_or_above_one_is_accepted(#[case] value: i32) {
    let params = RideConfigParams {
        deck_count: value,
        ..valid_params()
    };

    let config = RideConfig::new(params).expect("the value is in range");

    assert_eq!(
        config.deck_count,
        u32::try_from(value).expect("the fixture is non-negative")
    );
}

// ---------------------------------------------------- jokers_per_deck bound

#[rstest]
#[case(-1)]
#[case(11)]
fn test_ride_config_jokers_per_deck_out_of_range_is_refused(#[case] value: i32) {
    let params = RideConfigParams {
        jokers_per_deck: value,
        ..valid_params()
    };

    let result = RideConfig::new(params);

    assert_eq!(
        result,
        Err(RideError::Config {
            message: format!("jokers_per_deck must be 0..10, got {value}"),
        })
    );
}

#[rstest]
#[case(0)]
#[case(1)]
#[case(2)]
#[case(9)]
#[case(10)]
fn test_ride_config_jokers_per_deck_in_range_is_accepted(#[case] value: i32) {
    let params = RideConfigParams {
        jokers_per_deck: value,
        ..valid_params()
    };

    let config = RideConfig::new(params).expect("the value is in range");

    assert_eq!(
        config.jokers_per_deck,
        u32::try_from(value).expect("the fixture is non-negative")
    );
}

// ----------------------------------------------------------- jokers_mode

#[rstest]
#[case("per_deck", JokersMode::PerDeck)]
#[case("total", JokersMode::Total)]
fn test_ride_config_jokers_mode_given_a_known_spelling_is_accepted(
    #[case] text: &str,
    #[case] expected: JokersMode,
) {
    let params = RideConfigParams {
        jokers_mode: text.to_owned(),
        ..valid_params()
    };

    let config = RideConfig::new(params).expect("the mode is known");

    assert_eq!(config.jokers_mode, expected);
}

#[rstest]
#[case("perdeck")]
#[case("Per_Deck")]
#[case("per-deck")]
#[case("both")]
#[case("")]
fn test_ride_config_jokers_mode_given_an_unknown_spelling_is_refused(#[case] text: &str) {
    let params = RideConfigParams {
        jokers_mode: text.to_owned(),
        ..valid_params()
    };

    let result = RideConfig::new(params);

    assert_eq!(
        result,
        Err(RideError::Config {
            message: format!("jokers_mode must be one of ('per_deck', 'total'), got '{text}'"),
        })
    );
}

// ------------------------------------------------------ planned_duration_s

#[rstest]
#[case(0)]
#[case(-1)]
fn test_ride_config_planned_duration_not_positive_is_refused(#[case] value: i32) {
    let params = RideConfigParams {
        planned_duration_s: value,
        ..valid_params()
    };

    let result = RideConfig::new(params);

    assert_eq!(
        result,
        Err(RideError::Config {
            message: format!("planned_duration_s must be positive, got {value}"),
        })
    );
}

#[rstest]
#[case(1)]
#[case(21_600)]
fn test_ride_config_planned_duration_positive_is_accepted(#[case] value: i32) {
    let params = RideConfigParams {
        planned_duration_s: value,
        ..valid_params()
    };

    let config = RideConfig::new(params).expect("the value is positive");

    assert_eq!(
        config.planned_duration_s,
        u32::try_from(value).expect("the fixture is positive")
    );
}

// --------------------------------------------------------------- min_lap_s

#[rstest]
#[case(0)]
#[case(-1)]
fn test_ride_config_min_lap_not_positive_is_refused(#[case] value: i32) {
    let params = RideConfigParams {
        min_lap_s: value,
        ..valid_params()
    };

    let result = RideConfig::new(params);

    assert_eq!(
        result,
        Err(RideError::Config {
            message: format!("min_lap_s must be positive, got {value}"),
        })
    );
}

#[rstest]
#[case(1)]
#[case(1_080)]
fn test_ride_config_min_lap_positive_is_accepted(#[case] value: i32) {
    let params = RideConfigParams {
        min_lap_s: value,
        ..valid_params()
    };

    let config = RideConfig::new(params).expect("the value is positive");

    assert_eq!(
        config.min_lap_s,
        u32::try_from(value).expect("the fixture is positive")
    );
}

// ------------------------------------------------------------ TieBreak text

#[rstest]
#[case(TieBreak::HighCard, "high_card")]
#[case(TieBreak::Laps, "laps")]
#[case(TieBreak::TotalTime, "total_time")]
fn test_tiebreak_as_str_returns_the_oracle_spelling(
    #[case] value: TieBreak,
    #[case] expected: &str,
) {
    assert_eq!(value.as_str(), expected);
}

#[rstest]
#[case("high_card", TieBreak::HighCard)]
#[case("laps", TieBreak::Laps)]
#[case("total_time", TieBreak::TotalTime)]
fn test_tiebreak_from_str_reads_the_oracle_spelling(
    #[case] text: &str,
    #[case] expected: TieBreak,
) {
    assert_eq!(text.parse::<TieBreak>(), Ok(expected));
}

#[test]
fn test_tiebreak_from_str_given_an_unknown_spelling_returns_a_config_error() {
    assert_eq!(
        "highcard".parse::<TieBreak>(),
        Err(RideError::Config {
            message: "unknown tiebreak: highcard".to_owned(),
        })
    );
}

// ------------------------------------------------------------ EntryMode text

#[rstest]
#[case(EntryMode::Solo, "solo")]
#[case(EntryMode::Mixed, "mixed")]
fn test_entry_mode_as_str_returns_the_oracle_spelling(
    #[case] value: EntryMode,
    #[case] expected: &str,
) {
    assert_eq!(value.as_str(), expected);
}

#[rstest]
#[case("solo", EntryMode::Solo)]
#[case("mixed", EntryMode::Mixed)]
fn test_entry_mode_from_str_reads_the_oracle_spelling(
    #[case] text: &str,
    #[case] expected: EntryMode,
) {
    assert_eq!(text.parse::<EntryMode>(), Ok(expected));
}

#[test]
fn test_entry_mode_from_str_given_an_unknown_spelling_returns_a_config_error() {
    assert_eq!(
        "teams".parse::<EntryMode>(),
        Err(RideError::Config {
            message: "unknown entry mode: teams".to_owned(),
        })
    );
}

// ------------------------------------------------------------ PlateModel text

#[rstest]
#[case(PlateModel::RiderPooled, "rider_pooled")]
#[case(PlateModel::TeamRelay, "team_relay")]
fn test_plate_model_as_str_returns_the_oracle_spelling(
    #[case] value: PlateModel,
    #[case] expected: &str,
) {
    assert_eq!(value.as_str(), expected);
}

#[rstest]
#[case("rider_pooled", PlateModel::RiderPooled)]
#[case("team_relay", PlateModel::TeamRelay)]
fn test_plate_model_from_str_reads_the_oracle_spelling(
    #[case] text: &str,
    #[case] expected: PlateModel,
) {
    assert_eq!(text.parse::<PlateModel>(), Ok(expected));
}

#[test]
fn test_plate_model_from_str_given_an_unknown_spelling_returns_a_config_error() {
    assert_eq!(
        "pooled".parse::<PlateModel>(),
        Err(RideError::Config {
            message: "unknown plate model: pooled".to_owned(),
        })
    );
}

// ------------------------------------------------------------- serde values

#[rstest]
#[case(TieBreak::HighCard, "\"high_card\"")]
#[case(TieBreak::Laps, "\"laps\"")]
#[case(TieBreak::TotalTime, "\"total_time\"")]
fn test_tiebreak_serde_round_trips_the_oracle_spelling(
    #[case] value: TieBreak,
    #[case] json: &str,
) {
    let encoded = serde_json::to_string(&value).expect("the value serializes");

    assert_eq!(encoded, json);
    assert_eq!(
        serde_json::from_str::<TieBreak>(json).expect("the json parses"),
        value
    );
}

#[rstest]
#[case(EntryMode::Solo, "\"solo\"")]
#[case(EntryMode::Mixed, "\"mixed\"")]
fn test_entry_mode_serde_round_trips_the_oracle_spelling(
    #[case] value: EntryMode,
    #[case] json: &str,
) {
    let encoded = serde_json::to_string(&value).expect("the value serializes");

    assert_eq!(encoded, json);
    assert_eq!(
        serde_json::from_str::<EntryMode>(json).expect("the json parses"),
        value
    );
}

#[rstest]
#[case(PlateModel::RiderPooled, "\"rider_pooled\"")]
#[case(PlateModel::TeamRelay, "\"team_relay\"")]
fn test_plate_model_serde_round_trips_the_oracle_spelling(
    #[case] value: PlateModel,
    #[case] json: &str,
) {
    let encoded = serde_json::to_string(&value).expect("the value serializes");

    assert_eq!(encoded, json);
    assert_eq!(
        serde_json::from_str::<PlateModel>(json).expect("the json parses"),
        value
    );
}

// ------------------------------------------------- minimum-setup rule

#[test]
fn test_setup_minimum_violations_complete_config_returns_no_violation() {
    assert_eq!(
        setup_minimum_violations(&valid_config()),
        Vec::<String>::new()
    );
}

#[rstest]
#[case("", "name is required")]
#[case("   ", "name is required")]
fn test_setup_minimum_violations_blank_name_reports_its_reason(
    #[case] value: &str,
    #[case] reason: &str,
) {
    let config = RideConfig {
        name: value.to_owned(),
        ..valid_config()
    };

    assert_eq!(setup_minimum_violations(&config), vec![reason.to_owned()]);
}

#[rstest]
#[case("", "venue is required")]
#[case("\t", "venue is required")]
fn test_setup_minimum_violations_blank_venue_reports_its_reason(
    #[case] value: &str,
    #[case] reason: &str,
) {
    let config = RideConfig {
        venue: value.to_owned(),
        ..valid_config()
    };

    assert_eq!(setup_minimum_violations(&config), vec![reason.to_owned()]);
}

#[rstest]
#[case("", "organizer is required")]
#[case(" \t ", "organizer is required")]
fn test_setup_minimum_violations_blank_organizer_reports_its_reason(
    #[case] value: &str,
    #[case] reason: &str,
) {
    let config = RideConfig {
        organizer: value.to_owned(),
        ..valid_config()
    };

    assert_eq!(setup_minimum_violations(&config), vec![reason.to_owned()]);
}

#[rstest]
#[case("", "scorer is required")]
#[case("  ", "scorer is required")]
fn test_setup_minimum_violations_blank_scorer_reports_its_reason(
    #[case] value: &str,
    #[case] reason: &str,
) {
    let config = RideConfig {
        scorer: value.to_owned(),
        ..valid_config()
    };

    assert_eq!(setup_minimum_violations(&config), vec![reason.to_owned()]);
}

#[rstest]
#[case(0.0)]
#[case(-0.5)]
fn test_setup_minimum_violations_nonpositive_lap_reports_its_reason(#[case] value: f64) {
    let config = RideConfig {
        lap_km: value,
        ..valid_config()
    };

    assert_eq!(
        setup_minimum_violations(&config),
        vec!["lap length must be positive".to_owned()]
    );
}

#[test]
fn test_setup_minimum_violations_fully_blank_config_reports_every_reason_in_order() {
    let config = RideConfig {
        name: String::new(),
        venue: " ".to_owned(),
        organizer: String::new(),
        scorer: "  ".to_owned(),
        lap_km: 0.0,
        ..valid_config()
    };

    assert_eq!(
        setup_minimum_violations(&config),
        vec![
            "name is required".to_owned(),
            "venue is required".to_owned(),
            "organizer is required".to_owned(),
            "scorer is required".to_owned(),
            "lap length must be positive".to_owned(),
        ]
    );
}
