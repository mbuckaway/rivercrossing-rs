// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the ride-setup parsing and validation.

use pretty_assertions::assert_eq;
use rivercrossing_ride::{EntryMode, PlateModel, RideConfigParams};
use rstest::rstest;

use super::{
    SetupDefaults, defaults, format_duration, format_min_lap, params_from_fields, parse_duration,
    parse_min_lap, submit_errors, validated_config,
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

// -------------------------------------------------------- form fields

/// Return a form's fields with every value the ride-setup dialog seeds.
fn complete_fields() -> std::collections::BTreeMap<String, String> {
    let mut fields = std::collections::BTreeMap::new();
    for (id, value) in [
        ("name_input", "GORBA EPIC 2026"),
        ("date_picker", "2026-11-25"),
        ("start_time_picker", "08:00"),
        ("venue_input", "Sea to Sky Gondola"),
        ("lap_km_spin", "8.0"),
        ("organizer_input", "GORBA"),
        ("scorer_input", "K. Singh"),
        ("duration_input", "6:00"),
        ("min_lap_input", "20:00"),
        ("team_size_spin", "4"),
        ("decks_spin", "8"),
        ("jokers_spin", "1"),
        ("cap_choice", "Disabled"),
    ] {
        fields.insert(id.to_owned(), value.to_owned());
    }
    fields
}

#[test]
fn test_params_from_fields_reads_every_text_value() -> Result<(), String> {
    // Arrange
    let fields = complete_fields();

    // Act
    let params = params_from_fields(&fields).map_err(|errors| errors.join("; "))?;

    // Assert
    assert_eq!(params.name, "GORBA EPIC 2026");
    assert_eq!(params.venue, "Sea to Sky Gondola");
    assert_eq!(params.organizer, "GORBA");
    assert_eq!(params.scorer, "K. Singh");
    assert_eq!(params.jokers_mode, "total");
    assert_eq!(
        params.event_date,
        time::Date::from_calendar_date(2026, time::Month::November, 25)
            .map_err(|error| error.to_string())?
    );
    assert_eq!(params.planned_start.hour(), 8);
    assert_eq!(params.planned_start.minute(), 0);
    Ok(())
}

#[test]
fn test_params_from_fields_reads_every_numeric_value() -> Result<(), String> {
    // Arrange
    let fields = complete_fields();

    // Act
    let params = params_from_fields(&fields).map_err(|errors| errors.join("; "))?;

    // Assert
    assert!((params.lap_km - 8.0).abs() < f64::EPSILON);
    assert_eq!(params.planned_duration_s, 6 * 3600);
    assert_eq!(params.min_lap_s, 20 * 60);
    assert_eq!(params.max_team_size, 4);
    assert_eq!(params.deck_count, 8);
    assert_eq!(params.jokers_per_deck, 1);
    assert_eq!(params.max_cards, None);
    assert_eq!(params.entry_mode, EntryMode::Solo);
    assert_eq!(params.plate_model, PlateModel::RiderPooled);
    assert!(params.hold_short_laps);
    Ok(())
}

#[test]
fn test_params_from_fields_defaults_the_numbers_when_absent() -> Result<(), String> {
    // Arrange
    let mut fields = complete_fields();
    for id in [
        "lap_km_spin",
        "team_size_spin",
        "decks_spin",
        "jokers_spin",
        "cap_choice",
    ] {
        fields.remove(id);
    }

    // Act
    let params = params_from_fields(&fields).map_err(|errors| errors.join("; "))?;

    // Assert
    assert!((params.lap_km - 8.0).abs() < f64::EPSILON);
    assert_eq!(params.max_team_size, 4);
    assert_eq!(params.deck_count, 8);
    assert_eq!(params.jokers_per_deck, 1);
    assert_eq!(params.max_cards, None);
    Ok(())
}

#[test]
fn test_params_from_fields_reads_the_radio_modes() -> Result<(), String> {
    // Arrange
    let mut fields = complete_fields();
    for (id, value) in [
        ("mixed_radio", "true"),
        ("relay_radio", "true"),
        ("always_deal_radio", "true"),
        ("jokers_per_deck_radio", "true"),
        ("cap_choice", "7"),
    ] {
        fields.insert(id.to_owned(), value.to_owned());
    }

    // Act
    let params = params_from_fields(&fields).map_err(|errors| errors.join("; "))?;

    // Assert
    assert_eq!(params.entry_mode, EntryMode::Mixed);
    assert_eq!(params.plate_model, PlateModel::TeamRelay);
    assert!(!params.hold_short_laps);
    assert_eq!(params.jokers_mode, "per_deck");
    assert_eq!(params.max_cards, Some(7));
    Ok(())
}

#[test]
fn test_params_from_fields_given_a_bad_date_reports_it() {
    // Arrange
    let mut fields = complete_fields();
    fields.insert("date_picker".to_owned(), "25/11/2026".to_owned());

    // Act
    let errors = params_from_fields(&fields);

    // Assert
    assert_eq!(
        errors,
        Err(vec!["Date must be YYYY-MM-DD, got '25/11/2026'".to_owned()])
    );
}

#[test]
fn test_params_from_fields_given_a_blank_date_reports_it() {
    // Arrange
    let mut fields = complete_fields();
    fields.insert("date_picker".to_owned(), " ".to_owned());

    // Act
    let errors = params_from_fields(&fields);

    // Assert
    assert_eq!(
        errors,
        Err(vec!["Date is empty and must be completed".to_owned()])
    );
}

#[test]
fn test_params_from_fields_given_a_bad_time_reports_it() {
    // Arrange
    let mut fields = complete_fields();
    fields.insert("start_time_picker".to_owned(), "8h".to_owned());

    // Act
    let errors = params_from_fields(&fields);

    // Assert
    assert_eq!(
        errors,
        Err(vec!["Start time must be HH:MM, got '8h'".to_owned()])
    );
}

#[test]
fn test_params_from_fields_given_a_duration_that_does_not_parse_reports_it() {
    // Arrange
    let mut fields = complete_fields();
    fields.insert("duration_input".to_owned(), "six".to_owned());

    // Act
    let errors = params_from_fields(&fields);

    // Assert
    assert_eq!(
        errors,
        Err(vec!["Duration must be H:MM, got 'six'".to_owned()])
    );
}

#[test]
fn test_params_from_fields_collects_every_message() {
    // Arrange
    let mut fields = complete_fields();
    fields.insert("date_picker".to_owned(), "x".to_owned());
    fields.insert("lap_km_spin".to_owned(), "eight".to_owned());
    fields.insert("cap_choice".to_owned(), "3".to_owned());

    // Act
    let errors = params_from_fields(&fields);

    // Assert
    assert_eq!(
        errors,
        Err(vec![
            "Date must be YYYY-MM-DD, got 'x'".to_owned(),
            "Lap length must be a number, got 'eight'".to_owned(),
            "Card cap must be 'Disabled' or a number 5..20, got '3'".to_owned(),
        ])
    );
}

#[test]
fn test_validated_config_accepts_a_complete_form() -> Result<(), String> {
    // Arrange
    let params = params_from_fields(&complete_fields()).map_err(|errors| errors.join("; "))?;

    // Act
    let config = validated_config(&params);

    // Assert
    assert_eq!(
        config.map(|config| config.name),
        Ok("GORBA EPIC 2026".to_owned())
    );
    Ok(())
}

#[test]
fn test_validated_config_reports_the_minimum_violations() {
    // Arrange
    let mut fields = complete_fields();
    fields.insert("venue_input".to_owned(), String::new());

    // Act
    let errors = params_from_fields(&fields)
        .map_err(|errors| errors.join("; "))
        .and_then(|params| validated_config(&params).map_err(|errors| errors.join("; ")));

    // Assert
    assert_eq!(errors, Err("venue is required".to_owned()));
}

#[test]
fn test_validated_config_reports_a_bound_violation() {
    // Arrange
    let mut fields = complete_fields();
    fields.insert("decks_spin".to_owned(), "0".to_owned());

    // Act
    let errors = params_from_fields(&fields)
        .map_err(|errors| errors.join("; "))
        .and_then(|params| validated_config(&params).map_err(|errors| errors.join("; ")));

    // Assert
    assert_eq!(errors, Err("deck_count must be >= 1, got 0".to_owned()));
}

#[test]
fn test_params_from_fields_given_a_blank_time_reports_it() {
    // Arrange
    let mut fields = complete_fields();
    fields.insert("start_time_picker".to_owned(), String::new());

    // Act
    let errors = params_from_fields(&fields);

    // Assert
    assert_eq!(
        errors,
        Err(vec!["Start time is empty and must be completed".to_owned()])
    );
}

#[test]
fn test_params_from_fields_given_a_date_with_four_parts_reports_it() {
    // Arrange
    let mut fields = complete_fields();
    fields.insert("date_picker".to_owned(), "2026-11-25-1".to_owned());

    // Act
    let errors = params_from_fields(&fields);

    // Assert
    assert_eq!(
        errors,
        Err(vec![
            "Date must be YYYY-MM-DD, got '2026-11-25-1'".to_owned()
        ])
    );
}

#[test]
fn test_params_from_fields_given_an_impossible_date_reports_it() {
    // Arrange
    let mut fields = complete_fields();
    fields.insert("date_picker".to_owned(), "2026-02-31".to_owned());

    // Act
    let errors = params_from_fields(&fields);

    // Assert
    assert_eq!(
        errors,
        Err(vec!["Date must be YYYY-MM-DD, got '2026-02-31'".to_owned()])
    );
}

#[test]
fn test_params_from_fields_given_an_impossible_time_reports_it() {
    // Arrange
    let mut fields = complete_fields();
    fields.insert("start_time_picker".to_owned(), "25:00".to_owned());

    // Act
    let errors = params_from_fields(&fields);

    // Assert
    assert_eq!(
        errors,
        Err(vec!["Start time must be HH:MM, got '25:00'".to_owned()])
    );
}

#[test]
fn test_params_from_fields_given_a_duration_past_the_bound_reports_it() {
    // Arrange
    let mut fields = complete_fields();
    fields.insert("duration_input".to_owned(), "600000:00".to_owned());

    // Act
    let errors = params_from_fields(&fields);

    // Assert
    assert_eq!(errors, Err(vec!["Duration is too long".to_owned()]));
}

#[test]
fn test_params_from_fields_given_a_bad_team_size_reports_it() {
    // Arrange
    let mut fields = complete_fields();
    fields.insert("team_size_spin".to_owned(), "four".to_owned());
    fields.insert("decks_spin".to_owned(), "eight".to_owned());
    fields.insert("jokers_spin".to_owned(), "one".to_owned());

    // Act
    let errors = params_from_fields(&fields);

    // Assert
    assert_eq!(
        errors,
        Err(vec![
            "Team size must be a number, got 'four'".to_owned(),
            "Decks must be a number, got 'eight'".to_owned(),
            "Jokers must be a number, got 'one'".to_owned(),
        ])
    );
}
