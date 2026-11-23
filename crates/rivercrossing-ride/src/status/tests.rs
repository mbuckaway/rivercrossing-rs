// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the ride lifecycle state's stored spellings.

use std::str::FromStr;

use rstest::rstest;

use super::RideStatus;
use crate::error::RideError;

#[rstest]
#[case(RideStatus::Draft, "draft")]
#[case(RideStatus::Running, "running")]
#[case(RideStatus::Finished, "finished")]
#[case(RideStatus::Reopened, "reopened")]
fn as_str_returns_the_stored_spelling(#[case] status: RideStatus, #[case] spelling: &str) {
    assert_eq!(status.as_str(), spelling);
}

#[rstest]
#[case("draft", RideStatus::Draft)]
#[case("running", RideStatus::Running)]
#[case("finished", RideStatus::Finished)]
#[case("reopened", RideStatus::Reopened)]
fn from_str_reads_the_stored_spelling(#[case] spelling: &str, #[case] status: RideStatus) {
    assert_eq!(RideStatus::from_str(spelling), Ok(status));
}

#[test]
fn from_str_given_an_unknown_spelling_is_refused() {
    let error = RideStatus::from_str("paused").expect_err("paused is not a ride status");

    assert_eq!(
        error,
        RideError::IllegalState {
            message: "unknown ride status: paused".to_owned(),
        }
    );
}

#[test]
fn from_str_given_the_uppercase_spelling_is_refused() {
    assert!(
        RideStatus::from_str("Draft").is_err(),
        "the stored spelling is lowercase only"
    );
}
