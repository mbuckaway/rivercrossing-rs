// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the exporter's error type.

use pretty_assertions::assert_eq;
use rivercrossing_standings::StandingsError;

use super::ExportError;

#[test]
fn test_export_error_given_a_non_numeric_plate_names_the_plate() {
    // Arrange
    let error = ExportError::PlateNotNumeric {
        plate: "ABC".to_owned(),
    };

    // Act
    let message = error.to_string();

    // Assert
    assert_eq!(
        message,
        "plate 'ABC' is not numeric; htmlexport.render() emits plates as integers"
    );
}

#[test]
fn test_export_error_given_a_standings_refusal_converts_and_keeps_its_message() {
    // Arrange
    let standings = StandingsError::EmptyHand;

    // Act
    let error = ExportError::from(standings);

    // Assert
    assert!(matches!(
        error,
        ExportError::HandName(StandingsError::EmptyHand)
    ));
    assert_eq!(error.to_string(), "cannot name an empty hand");
}

#[test]
fn test_export_error_given_a_template_message_displays_it() {
    // Arrange
    let error = ExportError::Template("base.html.j2 is missing".to_owned());

    // Act
    let message = error.to_string();

    // Assert
    assert_eq!(message, "template: base.html.j2 is missing");
}

#[test]
fn test_export_error_given_a_record_message_displays_it() {
    // Arrange
    let error = ExportError::Json("the record is missing 'results'".to_owned());

    // Act
    let message = error.to_string();

    // Assert
    assert_eq!(message, "record: the record is missing 'results'");
}
