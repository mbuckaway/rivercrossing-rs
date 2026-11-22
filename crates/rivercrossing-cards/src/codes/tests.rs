// SPDX-License-Identifier: GPL-3.0-only

use pretty_assertions::assert_eq;
use rstest::rstest;
use serde::Deserialize;

use super::seeded_card_codes;
use crate::card::Card;
use crate::shoe::natural_deck;

const GOLDEN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/shoe-golden.json"
));

#[derive(Deserialize)]
struct GoldenFile {
    seeded_card_codes: Seeded,
}

#[derive(Deserialize)]
struct Seeded {
    seed: u64,
    decks_1: Vec<String>,
    decks_2: Vec<String>,
}

fn golden() -> GoldenFile {
    serde_json::from_str(GOLDEN).expect("shoe-golden.json parses")
}

fn codes_of(cards: &[Card]) -> Vec<String> {
    cards.iter().map(Card::code).collect()
}

#[test]
fn seeded_card_codes_are_fifty_two_unique_naturals() {
    // Act
    let cards = seeded_card_codes(0, 1);
    let mut codes = codes_of(&cards);
    codes.sort();
    codes.dedup();

    // Assert
    assert_eq!(cards.len(), 52);
    assert_eq!(codes.len(), 52);
    assert!(cards.iter().all(|card| !card.is_joker()));
}

#[test]
fn seeded_card_codes_never_contains_a_joker() {
    // Act
    let cards = seeded_card_codes(0, 8);

    // Assert
    assert!(cards.iter().all(|card| !card.is_joker()));
}

#[test]
fn seeded_card_codes_spells_tens() {
    // Act
    let codes = codes_of(&seeded_card_codes(0, 1));

    // Assert
    assert!(codes.iter().any(|code| code == "10C"));
    assert!(codes.iter().all(|code| !code.starts_with('T')));
}

#[test]
fn seeded_card_codes_is_seed_stable() {
    // Act
    let first = codes_of(&seeded_card_codes(0, 1));
    let second = codes_of(&seeded_card_codes(0, 1));

    // Assert
    assert_eq!(first, second);
    assert_eq!(first, golden().seeded_card_codes.decks_1);
}

#[test]
fn seeded_card_codes_differs_in_order_not_set_between_seeds() {
    // Act
    let zero = codes_of(&seeded_card_codes(0, 1));
    let one = codes_of(&seeded_card_codes(1, 1));
    let mut zero_sorted = zero.clone();
    let mut one_sorted = one.clone();
    zero_sorted.sort();
    one_sorted.sort();

    // Assert
    assert_ne!(zero, one);
    assert_eq!(zero_sorted, one_sorted);
}

#[test]
fn seeded_card_codes_with_eight_decks_is_still_fifty_two_unique() {
    // Act
    let cards = seeded_card_codes(0, 8);
    let mut codes = codes_of(&cards);
    codes.sort();
    let mut deck = codes_of(&natural_deck(1));
    deck.sort();

    // Assert
    assert_eq!(cards.len(), 52);
    assert_eq!(codes, deck);
}

#[rstest]
#[case(1)]
#[case(2)]
fn seeded_card_codes_matches_the_oracle(#[case] decks: u8) {
    // Arrange
    let file = golden();
    let expected = match decks {
        1 => &file.seeded_card_codes.decks_1,
        _ => &file.seeded_card_codes.decks_2,
    };

    // Act
    let actual = codes_of(&seeded_card_codes(file.seeded_card_codes.seed, decks));

    // Assert
    assert_eq!(&actual, expected);
}
