// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the roster error type's messages and its ride-crate mapping.

use pretty_assertions::assert_eq;
use rivercrossing_ride::RideError;
use rstest::rstest;

use super::RosterError;

#[rstest]
#[case(
    RosterError::TeamSize {
        message: "max_team_size must be between 2 and 10, got 1".to_owned(),
    },
    "max_team_size"
)]
#[case(
    RosterError::TeamSize {
        message: "team size must be between 2 and 4, got 5".to_owned(),
    },
    "team size"
)]
#[case(RosterError::SoloOnlyRide, "solo-only")]
#[case(
    RosterError::Locked {
        message: "rider moves are locked for a team_relay ride once running".to_owned(),
    },
    "team_relay"
)]
#[case(
    RosterError::Locked {
        message: "rider moves are locked for a rider_pooled ride once running".to_owned(),
    },
    "rider_pooled"
)]
#[case(
    RosterError::EntryNotFound {
        key: "a1b2".to_owned(),
    },
    "not a member"
)]
#[case(
    RosterError::RiderNotFound {
        description: "rider is not on any entry in this roster".to_owned(),
    },
    "not on any entry"
)]
#[case(
    RosterError::InvalidMove {
        message: "move would exceed the destination team's max size".to_owned(),
    },
    "max size"
)]
#[case(
    RosterError::InvalidMove {
        message: "move_rider's destination must be a team entry".to_owned(),
    },
    "destination must be a team entry"
)]
#[case(
    RosterError::Locked {
        message: "rider moves are locked for a rider_pooled ride once running".to_owned(),
    },
    "locked"
)]
#[case(
    RosterError::Locked {
        message: "entries can no longer be deleted once the ride is running".to_owned(),
    },
    "no longer be deleted"
)]
#[case(
    RosterError::Locked {
        message: "entry has recorded data; DNF or void it instead of deleting".to_owned(),
    },
    "recorded data"
)]
#[case(
    RosterError::PlateShape {
        plate: "77A".to_owned(),
        reason: "must be a whole number".to_owned(),
    },
    "plate '77A' must be a whole number"
)]
#[case(
    RosterError::PlateShape {
        plate: String::new(),
        reason: "must not be empty".to_owned(),
    },
    "plate '' must not be empty"
)]
#[case(
    RosterError::RiderNotFound {
        description: "a rider_pooled team plate change requires the member rider".to_owned(),
    },
    "a rider_pooled team plate change requires the member rider"
)]
#[case(
    RosterError::TeamSize {
        message: "team size must be at most 4, got 5".to_owned(),
    },
    "team size must be at most"
)]
#[case(
    RosterError::DuplicatePlate {
        plate: "77A".to_owned(),
    },
    "plate '77A' is already in use"
)]
fn test_roster_error_display_contains_the_oracle_substring(
    #[case] error: RosterError,
    #[case] expected: &str,
) {
    assert!(
        error.to_string().contains(expected),
        "{error} must contain {expected}"
    );
}

#[test]
fn test_roster_error_display_given_a_whole_number_plate_matches_the_oracle_text() {
    let error = RosterError::PlateShape {
        plate: "77A".to_owned(),
        reason: "must be a whole number".to_owned(),
    };

    assert_eq!(error.to_string(), "plate '77A' must be a whole number");
}

#[test]
fn test_roster_error_display_given_an_empty_plate_matches_the_oracle_text() {
    let error = RosterError::PlateShape {
        plate: String::new(),
        reason: "must not be empty".to_owned(),
    };

    assert_eq!(error.to_string(), "plate '' must not be empty");
}

#[test]
fn test_roster_error_from_given_a_solo_only_ride_wraps_the_display_text() {
    let error = RideError::from(RosterError::SoloOnlyRide);

    assert_eq!(
        error,
        RideError::Roster {
            message: "this ride is solo-only; team entries are not allowed".to_owned(),
        }
    );
}

#[test]
fn test_roster_error_from_given_a_team_size_refusal_wraps_the_display_text() {
    let error = RideError::from(RosterError::TeamSize {
        message: "team size must be at most 4, got 5".to_owned(),
    });

    assert_eq!(
        error,
        RideError::Roster {
            message: "team size must be at most 4, got 5".to_owned(),
        }
    );
}
