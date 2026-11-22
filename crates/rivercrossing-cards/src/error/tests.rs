// SPDX-License-Identifier: GPL-3.0-only

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::CardsError;

#[rstest]
#[case(CardsError::ShoeEmpty, "shoe is empty")]
#[case(CardsError::ShoeClosed, "shoe is closed")]
#[case(CardsError::JokerHasNoDrawKey, "a joker has no draw key")]
#[case(
    CardsError::NotLastDealt { code: "AS".to_owned() },
    "AS was not the last card dealt from this shoe"
)]
#[case(
    CardsError::InvalidCode { code: "XX".to_owned() },
    "XX is not a valid card code"
)]
#[case(
    CardsError::InvalidConfig { decks: 0, jokers_per_deck: 1 },
    "invalid shoe config: 0 decks, 1 jokers per deck"
)]
fn error_display_matches_the_documented_message(#[case] error: CardsError, #[case] message: &str) {
    // Act
    let display = error.to_string();

    // Assert
    assert_eq!(display, message);
}
