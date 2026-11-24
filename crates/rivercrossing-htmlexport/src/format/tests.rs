// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the exporter's pure formatting functions.

#![expect(
    clippy::too_many_arguments,
    reason = "a case table and a fixture name each calendar field separately"
)]

use base64::Engine as _;
use pretty_assertions::assert_eq;
use rstest::rstest;
use time::{Date, Month, OffsetDateTime, Time};

use super::{format_duration, format_event_date, format_generated, format_meta, png_data_uri};
use crate::types::{RideInfo, TRANSPARENT_PNG};

fn date(year: i32, month: Month, day: u8) -> Date {
    Date::from_calendar_date(year, month, day).expect("the fixture date is valid")
}

fn instant(year: i32, month: Month, day: u8, hour: u8, minute: u8) -> OffsetDateTime {
    let time = Time::from_hms(hour, minute, 0).expect("the fixture time is valid");
    date(year, month, day).with_time(time).assume_utc()
}

fn ride(lap_km: f64) -> RideInfo {
    RideInfo {
        name: "GORBA EPIC 2026".to_owned(),
        event_date: date(2026, Month::September, 20),
        venue: "Guelph Lake MTB Trails".to_owned(),
        lap_km,
        organizer: "Organizer: GORBA".to_owned(),
        scorer: "Scorer: D. Whitfield".to_owned(),
    }
}

#[rstest]
#[case(2026, Month::January, 1, 0, 0, "Generated 00:00, Jan 1 2026")]
#[case(2026, Month::September, 20, 16, 7, "Generated 16:07, Sept 20 2026")]
fn test_format_generated_given_a_pinned_instant_reads_the_footer(
    #[case] year: i32,
    #[case] month: Month,
    #[case] day: u8,
    #[case] hour: u8,
    #[case] minute: u8,
    #[case] expected: &str,
) {
    // Arrange
    let at = instant(year, month, day, hour, minute);

    // Act
    let stamp = format_generated(at);

    // Assert
    assert_eq!(stamp, expected);
}

#[rstest]
#[case(Month::January, "Jan")]
#[case(Month::February, "Feb")]
#[case(Month::March, "Mar")]
#[case(Month::April, "Apr")]
#[case(Month::May, "May")]
#[case(Month::June, "Jun")]
#[case(Month::July, "Jul")]
#[case(Month::August, "Aug")]
#[case(Month::September, "Sept")]
#[case(Month::October, "Oct")]
#[case(Month::November, "Nov")]
#[case(Month::December, "Dec")]
fn test_format_generated_given_each_month_reads_its_abbreviation(
    #[case] month: Month,
    #[case] expected: &str,
) {
    // Arrange
    let at = instant(2026, month, 5, 9, 3);

    // Act
    let stamp = format_generated(at);

    // Assert
    assert_eq!(stamp, format!("Generated 09:03, {expected} 5 2026"));
}

#[rstest]
#[case(0.0, "0:00")]
#[case(59.0, "0:59")]
#[case(60.0, "1:00")]
#[case(3599.0, "59:59")]
#[case(3600.0, "1:00:00")]
#[case(3601.0, "1:00:01")]
#[case(1679.0, "27:59")]
#[case(21161.0, "5:52:41")]
fn test_format_duration_given_whole_seconds_reads_the_clock(
    #[case] seconds: f64,
    #[case] expected: &str,
) {
    // Act
    let clock = format_duration(seconds);

    // Assert
    assert_eq!(clock, expected);
}

#[rstest]
#[case(2.5, "0:02")]
#[case(3.5, "0:04")]
fn test_format_duration_given_a_half_second_rounds_to_even(
    #[case] seconds: f64,
    #[case] expected: &str,
) {
    // Act
    let clock = format_duration(seconds);

    // Assert
    assert_eq!(clock, expected);
}

#[test]
fn test_format_duration_given_a_second_that_rounds_across_the_hour_reads_hours() {
    // Act
    let clock = format_duration(3599.6);

    // Assert
    assert_eq!(clock, "1:00:00");
}

#[rstest]
#[case(2026, Month::January, 1, "Thursday January 1, 2026")]
#[case(2026, Month::February, 1, "Sunday February 1, 2026")]
#[case(2026, Month::March, 1, "Sunday March 1, 2026")]
#[case(2026, Month::April, 1, "Wednesday April 1, 2026")]
#[case(2026, Month::May, 1, "Friday May 1, 2026")]
#[case(2026, Month::June, 1, "Monday June 1, 2026")]
#[case(2026, Month::July, 1, "Wednesday July 1, 2026")]
#[case(2026, Month::August, 1, "Saturday August 1, 2026")]
#[case(2026, Month::September, 1, "Tuesday September 1, 2026")]
#[case(2026, Month::October, 1, "Thursday October 1, 2026")]
#[case(2026, Month::November, 1, "Sunday November 1, 2026")]
#[case(2026, Month::December, 1, "Tuesday December 1, 2026")]
fn test_format_event_date_given_each_month_reads_its_english_name(
    #[case] year: i32,
    #[case] month: Month,
    #[case] day: u8,
    #[case] expected: &str,
) {
    // Arrange
    let day = date(year, month, day);

    // Act
    let reading = format_event_date(day);

    // Assert
    assert_eq!(reading, expected);
}

#[rstest]
#[case(20, "Sunday September 20, 2026")]
#[case(21, "Monday September 21, 2026")]
#[case(22, "Tuesday September 22, 2026")]
#[case(23, "Wednesday September 23, 2026")]
#[case(24, "Thursday September 24, 2026")]
#[case(25, "Friday September 25, 2026")]
#[case(26, "Saturday September 26, 2026")]
fn test_format_event_date_given_each_weekday_reads_its_english_name(
    #[case] day: u8,
    #[case] expected: &str,
) {
    // Arrange
    let day = date(2026, Month::September, day);

    // Act
    let reading = format_event_date(day);

    // Assert
    assert_eq!(reading, expected);
}

#[test]
fn test_format_meta_given_an_integral_lap_reads_it_without_a_fraction() {
    // Arrange
    let ride = ride(8.0);

    // Act
    let meta = format_meta(&ride);

    // Assert
    assert_eq!(
        meta,
        "Sunday September 20, 2026 · Guelph Lake MTB Trails · 8 km loop"
    );
}

#[test]
fn test_format_meta_given_a_fractional_lap_keeps_the_fraction() {
    // Arrange
    let ride = ride(6.5);

    // Act
    let meta = format_meta(&ride);

    // Assert
    assert_eq!(
        meta,
        "Sunday September 20, 2026 · Guelph Lake MTB Trails · 6.5 km loop"
    );
}

#[test]
fn test_png_data_uri_given_the_transparent_png_bytes_reads_the_oracle_data_uri() {
    // Arrange
    let encoded = TRANSPARENT_PNG
        .strip_prefix("data:image/png;base64,")
        .expect("the oracle URI carries the PNG prefix");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .expect("the oracle URI decodes");

    // Act
    let uri = png_data_uri(&bytes);

    // Assert
    assert_eq!(uri, TRANSPARENT_PNG);
}
