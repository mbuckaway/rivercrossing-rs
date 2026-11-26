// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the ride library's rows and its delete and duplicate copy.

use pretty_assertions::assert_eq;
use rivercrossing_ride::RideStatus;
use rivercrossing_store::RideRow;
use rstest::rstest;
use time::{Date, Month};

use super::{delete_matches, delete_requires, duplicate_confirm, rows};

// ------------------------------------------------------------- fixtures

/// Return the fixture library row.
fn ride_row(name: &str) -> Result<RideRow, String> {
    Ok(RideRow {
        id: 7,
        name: name.to_owned(),
        event_date: Date::from_calendar_date(2026, Month::September, 20)
            .map_err(|error| error.to_string())?,
        status: RideStatus::Running,
        entries: 12,
    })
}

// ------------------------------------------------------------------ rows

#[test]
fn test_library_rows_given_store_rows_maps_the_four_columns() -> Result<(), String> {
    // Arrange
    let store_rows = vec![ride_row("GORBA EPIC 2026")?];

    // Act
    let models = rows(&store_rows);

    // Assert
    assert_eq!(
        models.first().map(|row| (
            row.name.as_str(),
            row.date.as_str(),
            row.status.as_str(),
            row.entries
        )),
        Some(("GORBA EPIC 2026", "2026-09-20", "RUNNING", 12))
    );
    Ok(())
}

#[test]
fn test_library_rows_keeps_the_store_order() -> Result<(), String> {
    // Arrange
    let store_rows = vec![ride_row("Zulu")?, ride_row("Alpha")?];

    // Act
    let models = rows(&store_rows);

    // Assert
    assert_eq!(
        models
            .iter()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Zulu", "Alpha"]
    );
    Ok(())
}

#[test]
fn test_library_rows_given_a_one_digit_day_zero_pads_the_date() -> Result<(), String> {
    // Arrange
    let store_row = RideRow {
        event_date: Date::from_calendar_date(2026, Month::September, 5)
            .map_err(|error| error.to_string())?,
        ..ride_row("GORBA EPIC 2026")?
    };

    // Act
    let models = rows(&[store_row]);

    // Assert
    assert_eq!(
        models.first().map(|row| row.date.as_str()),
        Some("2026-09-05")
    );
    Ok(())
}

#[rstest]
#[case(RideStatus::Draft, "DRAFT")]
#[case(RideStatus::Running, "RUNNING")]
#[case(RideStatus::Finished, "FINISHED")]
#[case(RideStatus::Reopened, "REOPENED")]
fn test_library_rows_given_each_status_renders_it_upper_case(
    #[case] status: RideStatus,
    #[case] expected: &str,
) -> Result<(), String> {
    // Arrange
    let store_row = RideRow {
        status,
        ..ride_row("GORBA EPIC 2026")?
    };

    // Act
    let models = rows(&[store_row]);

    // Assert
    assert_eq!(
        models.first().map(|row| row.status.as_str()),
        Some(expected)
    );
    Ok(())
}

#[test]
fn test_library_rows_given_no_rides_returns_an_empty_list() {
    // Arrange
    let store_rows: Vec<RideRow> = Vec::new();

    // Act
    let models = rows(&store_rows);

    // Assert
    assert_eq!(models, Vec::new());
}

// --------------------------------------------------------------- delete

#[test]
fn test_library_delete_requires_names_the_ride_and_the_instruction() -> Result<(), String> {
    // Arrange
    let row = ride_row("GORBA EPIC 2026")?;

    // Act
    let text = delete_requires(&row);

    // Assert
    assert_eq!(
        text,
        "Deletes \"GORBA EPIC 2026\" and all its data. Type the ride name to confirm."
    );
    Ok(())
}

#[test]
fn test_library_delete_requires_given_two_rides_differs_per_ride() -> Result<(), String> {
    // Arrange
    let first = ride_row("GORBA EPIC 2026")?;
    let second = ride_row("Club poker night")?;

    // Act
    let first_text = delete_requires(&first);
    let second_text = delete_requires(&second);

    // Assert
    assert_ne!(first_text, second_text);
    assert_eq!(second_text.contains("Club poker night"), true);
    assert_eq!(first_text.contains("Club poker night"), false);
    Ok(())
}

#[test]
fn test_library_delete_matches_given_the_exact_name_matches() -> Result<(), String> {
    // Arrange
    let row = ride_row("GORBA EPIC 2026")?;

    // Act
    let matches = delete_matches(&row, "GORBA EPIC 2026");

    // Assert
    assert_eq!(matches, true);
    Ok(())
}

#[test]
fn test_library_delete_matches_given_surrounding_whitespace_still_matches() -> Result<(), String> {
    // Arrange
    let row = ride_row("GORBA EPIC 2026")?;

    // Act
    let matches = delete_matches(&row, "  GORBA EPIC 2026\t");

    // Assert
    assert_eq!(matches, true);
    Ok(())
}

#[test]
fn test_library_delete_matches_given_a_case_difference_refuses() -> Result<(), String> {
    // Arrange
    let row = ride_row("GORBA EPIC 2026")?;

    // Act
    let matches = delete_matches(&row, "gorba epic 2026");

    // Assert
    assert_eq!(matches, false);
    Ok(())
}

#[test]
fn test_library_delete_matches_given_a_partial_name_refuses() -> Result<(), String> {
    // Arrange
    let row = ride_row("GORBA EPIC 2026")?;

    // Act
    let partial = delete_matches(&row, "GORBA");
    let blank = delete_matches(&row, "   ");

    // Assert
    assert_eq!(partial, false);
    assert_eq!(blank, false);
    Ok(())
}

// ------------------------------------------------------------ duplicate

#[test]
fn test_library_duplicate_confirm_given_a_ride_names_it_and_the_copy_scope() {
    // Arrange, Act
    let text = duplicate_confirm("GORBA EPIC 2026");

    // Assert
    assert_eq!(
        text,
        "Duplicate \"GORBA EPIC 2026\" as a new DRAFT ride?\n\n\
         Copies the ride's setup and full rider list — no timing data."
    );
}

#[test]
fn test_library_duplicate_confirm_given_two_rides_differs_per_ride() {
    // Arrange, Act
    let first = duplicate_confirm("GORBA EPIC 2026");
    let second = duplicate_confirm("Club poker night");

    // Assert
    assert_ne!(first, second);
    assert_eq!(second.contains("Club poker night"), true);
}
