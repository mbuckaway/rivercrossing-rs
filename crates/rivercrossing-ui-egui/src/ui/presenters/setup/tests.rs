// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the ride-setup parsing and validation.

use pretty_assertions::assert_eq;
use rivercrossing_ride::{EntryMode, PlateModel, RideConfigParams};
use rstest::rstest;

use super::{
    SetupDefaults, defaults, format_duration, format_min_lap, parse_duration, parse_min_lap,
    submit_errors,
};

/// Return a setup form whose only faults are the blank text fields.
fn blank_form() -> RideConfigParams {
    RideConfigParams::default()
}

/// Return a complete setup form that clears every bound and the minimum rule.
fn complete_form() -> RideConfigParams {
    RideConfigParams {
        name: "GORBA EPIC 2026".to_owned(),
        venue: "Sea to Sky Gondola".to_owned(),
        lap_km: 8.0,
        organizer: "GORBA".to_owned(),
        scorer: "K. Singh".to_owned(),
        entry_mode: EntryMode::Mixed,
        plate_model: PlateModel::RiderPooled,
        ..RideConfigParams::default()
    }
}

// ------------------------------------------------------------ duration

#[rstest]
#[case("0:00", 0)]
#[case("6:00", 21600)]
#[case("12:30", 45000)]
fn test_duration_parsing_round_trips(#[case] text: &str, #[case] expected: i64) {
    // Arrange
    // (the case supplies the field text and its whole seconds)

    // Act
    let seconds = parse_duration(text);

    // Assert
    assert_eq!(seconds, Ok(expected));
}

#[rstest]
#[case("", Err("Duration is empty and must be completed".to_owned()))]
#[case("   ", Err("Duration is empty and must be completed".to_owned()))]
#[case("6", Err("Duration must be H:MM, got '6'".to_owned()))]
#[case("6:00:00", Err("Duration must be H:MM, got '6:00:00'".to_owned()))]
#[case("a:bb", Err("Duration must be H:MM, got 'a:bb'".to_owned()))]
fn test_duration_parsing_given_blank_and_malformed(
    #[case] text: &str,
    #[case] expected: Result<i64, String>,
) {
    // Arrange
    // (the case supplies the refused field text)

    // Act
    let seconds = parse_duration(text);

    // Assert
    assert_eq!(seconds, expected);
}

#[rstest]
#[case(0, "0:00")]
#[case(59, "0:00")]
#[case(60, "0:01")]
#[case(21600, "6:00")]
#[case(45000, "12:30")]
fn test_format_duration_given_seconds_returns_h_mm(#[case] seconds: i64, #[case] expected: &str) {
    // Arrange
    // (the case supplies the stored duration)

    // Act
    let text = format_duration(seconds);

    // Assert
    assert_eq!(text, expected);
}

#[test]
fn test_duration_text_round_trips() {
    // Arrange
    let text = "6:00";

    // Act
    let rendered = parse_duration(text).map(format_duration);

    // Assert
    assert_eq!(rendered, Ok(text.to_owned()));
}

// ------------------------------------------------------------- min lap

#[rstest]
#[case("0:00", 0)]
#[case("18:00", 1080)]
#[case("2:05", 125)]
fn test_min_lap_parsing_round_trips(#[case] text: &str, #[case] expected: i64) {
    // Arrange
    // (the case supplies the field text and its whole seconds)

    // Act
    let seconds = parse_min_lap(text);

    // Assert
    assert_eq!(seconds, Ok(expected));
}

#[rstest]
#[case("", Err("Min lap is blank and must be completed".to_owned()))]
#[case("\t", Err("Min lap is blank and must be completed".to_owned()))]
#[case("18", Err("Min lap must be M:SS, got '18'".to_owned()))]
#[case("18:00:00", Err("Min lap must be M:SS, got '18:00:00'".to_owned()))]
#[case("a:bb", Err("Min lap must be M:SS, got 'a:bb'".to_owned()))]
fn test_min_lap_parsing_given_blank_and_malformed(
    #[case] text: &str,
    #[case] expected: Result<i64, String>,
) {
    // Arrange
    // (the case supplies the refused field text)

    // Act
    let seconds = parse_min_lap(text);

    // Assert
    assert_eq!(seconds, expected);
}

#[rstest]
#[case(0, "0:00")]
#[case(59, "0:59")]
#[case(60, "1:00")]
#[case(125, "2:05")]
#[case(1080, "18:00")]
fn test_format_min_lap_given_seconds_returns_m_ss(#[case] seconds: i64, #[case] expected: &str) {
    // Arrange
    // (the case supplies the stored minimum lap)

    // Act
    let text = format_min_lap(seconds);

    // Assert
    assert_eq!(text, expected);
}

// ------------------------------------------------------------ defaults

#[test]
fn test_setup_defaults() {
    // Arrange
    // (the defaults are the function's whole output)

    // Act
    let settings = defaults();

    // Assert
    assert_eq!(
        settings,
        SetupDefaults {
            decks: 8,
            lap_km: 8.0,
            jokers_per_deck: 1,
            team_size: 4,
        }
    );
}

// ---------------------------------------------------------- submit

#[test]
fn test_submit_errors_names_every_missing_minimum() {
    // Arrange
    let params = blank_form();

    // Act
    let errors = submit_errors(&params);

    // Assert
    assert_eq!(
        errors,
        vec![
            "name is required; venue is required; organizer is required; scorer is required; \
             lap length must be positive"
                .to_owned()
        ]
    );
}

#[test]
fn test_submit_errors_reports_a_bound_violation() {
    // Arrange
    let params = RideConfigParams {
        max_team_size: 1,
        ..complete_form()
    };

    // Act
    let errors = submit_errors(&params);

    // Assert
    assert_eq!(
        errors,
        vec!["max_team_size must be 2..10, got 1".to_owned()]
    );
}

#[test]
fn test_submit_errors_accepts_a_complete_setup() {
    // Arrange
    let params = complete_form();

    // Act
    let errors = submit_errors(&params);

    // Assert
    assert_eq!(errors, Vec::<String>::new());
}
