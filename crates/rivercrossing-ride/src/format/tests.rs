// SPDX-License-Identifier: GPL-3.0-only

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::format_elapsed;

#[rstest]
#[case(-3661.0, "-1:01:01")]
#[case(-3601.0, "-1:00:01")]
#[case(-3600.0, "-1:00:00")]
#[case(-59.0, "-0:00:59")]
#[case(-1.0, "-0:00:01")]
#[case(0.0, "0:00:00")]
#[case(1.0, "0:00:01")]
#[case(59.0, "0:00:59")]
#[case(60.0, "0:01:00")]
#[case(3599.0, "0:59:59")]
#[case(3600.0, "1:00:00")]
#[case(3601.0, "1:00:01")]
#[case(3725.0, "1:02:05")]
#[case(86399.0, "23:59:59")]
#[case(86400.0, "24:00:00")]
#[case(90061.0, "25:01:01")]
fn test_format_elapsed_given_boundary_seconds_reads_h_mm_ss(
    #[case] seconds: f64,
    #[case] expected: &str,
) {
    assert_eq!(format_elapsed(seconds), expected);
}

#[rstest]
#[case(-300.5, "-0:05:00")]
#[case(300.9, "0:05:00")]
fn test_format_elapsed_given_a_fractional_second_truncates_toward_zero(
    #[case] seconds: f64,
    #[case] expected: &str,
) {
    assert_eq!(format_elapsed(seconds), expected);
}

fn independent_reading(seconds: i32) -> String {
    let total = i64::from(seconds);
    let sign = if total < 0 { "-" } else { "" };
    let magnitude = total.unsigned_abs();
    format!(
        "{sign}{}:{:02}:{:02}",
        magnitude / 3600,
        magnitude % 3600 / 60,
        magnitude % 60
    )
}

fn assert_reading_round_trips_over_one_million_seconds() {
    for seconds in -1_000_000_i32..=1_000_000 {
        let actual = format_elapsed(f64::from(seconds));
        assert_eq!(actual, independent_reading(seconds), "seconds={seconds}");
    }
}

#[test]
fn test_format_elapsed_given_any_whole_second_round_trips_its_reading() {
    assert_reading_round_trips_over_one_million_seconds();
}
