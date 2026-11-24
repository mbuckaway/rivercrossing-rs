// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the CSV import and export error messages.

use std::io::{Error, ErrorKind};

use pretty_assertions::assert_eq;
use rivercrossing_roster::error::RosterError;
use rivercrossing_standings::StandingsError;

use super::CsvIoError;

#[test]
fn test_error_io_display_keeps_the_cause_text() {
    let error = CsvIoError::Io(Error::other("disk on fire"));

    assert_eq!(error.to_string(), "io: disk on fire");
}

#[test]
fn test_error_from_io_keeps_the_error_kind() {
    let error = CsvIoError::from(Error::new(ErrorKind::NotFound, "gone"));

    assert!(matches!(
        error,
        CsvIoError::Io(cause) if cause.kind() == ErrorKind::NotFound
    ));
}

#[test]
fn test_error_not_utf8_display_names_the_encoding_failure() {
    assert_eq!(
        CsvIoError::NotUtf8.to_string(),
        "csv data is not valid utf-8"
    );
}

#[test]
fn test_error_malformed_display_passes_the_reader_message() {
    let error = CsvIoError::Malformed {
        message: "malformed CSV data: field too large".to_owned(),
    };

    assert_eq!(error.to_string(), "malformed CSV data: field too large");
}

#[test]
fn test_error_header_display_passes_the_resolver_message() {
    let error = CsvIoError::Header {
        message: "no name column in the header row".to_owned(),
    };

    assert_eq!(error.to_string(), "no name column in the header row");
}

#[test]
fn test_error_conflicts_present_display_counts_them() {
    let error = CsvIoError::ConflictsPresent { count: 3 };

    assert_eq!(error.to_string(), "import has 3 blocking conflicts");
}

#[test]
fn test_error_blocked_display_passes_the_reason() {
    let error = CsvIoError::Blocked {
        message: "the ride has started".to_owned(),
    };

    assert_eq!(error.to_string(), "the ride has started");
}

#[test]
fn test_error_stale_preview_display_asks_for_a_fresh_preview() {
    assert_eq!(
        CsvIoError::StalePreview.to_string(),
        "the preview is stale; preview the file again"
    );
}

#[test]
fn test_error_not_finished_display_names_the_status() {
    let error = CsvIoError::NotFinished {
        status: "running".to_owned(),
    };

    assert_eq!(
        error.to_string(),
        "the ride is not finished (status: running)"
    );
}

#[test]
fn test_error_missing_standings_display_names_the_plate() {
    let error = CsvIoError::MissingStandings {
        plate: "42".to_owned(),
    };

    assert_eq!(error.to_string(), "no standings row for plate '42'");
}

#[test]
fn test_error_roster_display_forwards_the_roster_sentence() {
    let error = CsvIoError::from(RosterError::DuplicatePlate {
        plate: "7".to_owned(),
    });

    assert_eq!(error.to_string(), "plate '7' is already in use");
}

#[test]
fn test_error_standings_display_forwards_the_standings_sentence() {
    let error = CsvIoError::from(StandingsError::EmptyHand);

    assert_eq!(error.to_string(), "cannot name an empty hand");
}
