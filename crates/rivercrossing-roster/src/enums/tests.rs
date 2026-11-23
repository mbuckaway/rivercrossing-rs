// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the roster entry enums' stored spellings.

use std::str::FromStr;

use rstest::rstest;

use super::{EntryStatus, EntryType};
use crate::error::RosterError;

#[rstest]
#[case(EntryType::Solo, "solo")]
#[case(EntryType::Team, "team")]
fn test_entry_type_as_str_returns_the_stored_spelling(
    #[case] kind: EntryType,
    #[case] spelling: &str,
) {
    assert_eq!(kind.as_str(), spelling);
}

#[rstest]
#[case(EntryStatus::Active, "active")]
#[case(EntryStatus::Dnf, "dnf")]
fn test_entry_status_as_str_returns_the_stored_spelling(
    #[case] status: EntryStatus,
    #[case] spelling: &str,
) {
    assert_eq!(status.as_str(), spelling);
}

#[rstest]
#[case("solo", EntryType::Solo)]
#[case("team", EntryType::Team)]
fn test_entry_type_from_str_reads_the_stored_spelling(
    #[case] spelling: &str,
    #[case] kind: EntryType,
) {
    assert_eq!(EntryType::from_str(spelling), Ok(kind));
}

#[rstest]
#[case("active", EntryStatus::Active)]
#[case("dnf", EntryStatus::Dnf)]
fn test_entry_status_from_str_reads_the_stored_spelling(
    #[case] spelling: &str,
    #[case] status: EntryStatus,
) {
    assert_eq!(EntryStatus::from_str(spelling), Ok(status));
}

#[test]
fn test_entry_type_from_str_given_an_unknown_spelling_is_refused() {
    assert_eq!(
        EntryType::from_str("spectator"),
        Err(RosterError::InvalidMove {
            message: "unknown entry type: spectator".to_owned(),
        })
    );
}

#[test]
fn test_entry_status_from_str_given_an_unknown_spelling_is_refused() {
    assert_eq!(
        EntryStatus::from_str("withdrawn"),
        Err(RosterError::InvalidMove {
            message: "unknown entry status: withdrawn".to_owned(),
        })
    );
}

#[test]
fn test_entry_type_from_str_given_the_uppercase_spelling_is_refused() {
    assert_eq!(
        EntryType::from_str("Solo"),
        Err(RosterError::InvalidMove {
            message: "unknown entry type: Solo".to_owned(),
        })
    );
}

#[test]
fn test_entry_status_from_str_given_the_uppercase_spelling_is_refused() {
    assert_eq!(
        EntryStatus::from_str("DNF"),
        Err(RosterError::InvalidMove {
            message: "unknown entry status: DNF".to_owned(),
        })
    );
}
