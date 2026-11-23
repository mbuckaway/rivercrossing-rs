// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the standings error messages.

use pretty_assertions::assert_eq;
use rivercrossing_hands::HandClass;

use super::StandingsError;

#[test]
fn test_standings_error_unknown_spelling_names_it_quoted() {
    let error = StandingsError::UnknownTieBreakSpelling {
        spelling: "fastest".to_owned(),
    };
    assert_eq!(error.to_string(), "unknown tie-break spelling: 'fastest'");
}

#[test]
fn test_standings_error_empty_hand_keeps_the_oracle_sentence() {
    assert_eq!(
        StandingsError::EmptyHand.to_string(),
        "cannot name an empty hand"
    );
}

#[test]
fn test_standings_error_malformed_hand_names_the_class() {
    let error = StandingsError::MalformedHand {
        class: HandClass::FullHouse,
    };
    assert_eq!(error.to_string(), "hand class FullHouse is missing a count");
}

#[test]
fn test_standings_error_joker_draw_key_keeps_the_cards_sentence() {
    assert_eq!(
        StandingsError::JokerDrawKey.to_string(),
        "a joker has no draw key"
    );
}
