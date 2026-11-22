// SPDX-License-Identifier: GPL-3.0-only

use pretty_assertions::assert_eq;
use serde::Deserialize;

use super::{draw_key, high_card_draw};
use crate::card::Card;
use crate::error::CardsError;
use crate::shoe::natural_deck;

const GOLDEN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/shoe-golden.json"
));

#[derive(Deserialize)]
struct GoldenFile {
    draw_key: std::collections::BTreeMap<String, u8>,
    high_card_draw: HighCard,
    shoe: Vec<GoldenShoe>,
}

#[derive(Deserialize)]
struct HighCard {
    seed: u64,
    count: usize,
    cards: Vec<HighCardEntry>,
}

#[derive(Deserialize)]
struct HighCardEntry {
    code: String,
    key: u8,
}

#[derive(Deserialize)]
struct GoldenShoe {
    seed: u64,
    decks: u8,
    jokers_per_deck: u8,
    mode: String,
    codes: Vec<String>,
}

fn golden() -> GoldenFile {
    serde_json::from_str(GOLDEN).expect("shoe-golden.json parses")
}

#[test]
fn draw_key_table_is_exact() {
    // Arrange
    let file = golden();

    // Act
    let mismatches: Vec<String> = file
        .draw_key
        .iter()
        .filter_map(|(code, expected)| {
            let card = Card::parse(code).expect("fixture code parses");
            let actual = draw_key(&card).expect("a natural card has a key");
            (actual != *expected).then(|| format!("{code}: expected {expected}, got {actual}"))
        })
        .collect();

    // Assert
    assert_eq!(mismatches, Vec::<String>::new());
    assert_eq!(file.draw_key.len(), 52);
}

#[test]
fn draw_key_is_unique_over_fifty_two() {
    // Act
    let mut keys: Vec<u8> = natural_deck(1)
        .iter()
        .map(|card| draw_key(card).expect("a natural card has a key"))
        .collect();
    keys.sort_unstable();
    keys.dedup();

    // Assert
    assert_eq!(keys, (8..=59).collect::<Vec<u8>>());
}

#[test]
fn joker_has_no_draw_key() {
    // Act
    let result = draw_key(&Card::Joker);

    // Assert
    assert_eq!(result, Err(CardsError::JokerHasNoDrawKey));
}

#[test]
fn high_card_draw_is_seed_stable() {
    // Arrange
    let file = golden();
    let expected: Vec<Card> = file
        .high_card_draw
        .cards
        .iter()
        .map(|entry| Card::parse(&entry.code).expect("fixture code parses"))
        .collect();
    let expected_keys: Vec<u8> = file
        .high_card_draw
        .cards
        .iter()
        .map(|entry| entry.key)
        .collect();

    // Act
    let first = high_card_draw(file.high_card_draw.seed, file.high_card_draw.count);
    let second = high_card_draw(file.high_card_draw.seed, file.high_card_draw.count);
    let keys: Vec<u8> = first
        .iter()
        .map(|card| draw_key(card).expect("a natural card has a key"))
        .collect();

    // Assert
    assert_eq!(first, expected);
    assert_eq!(first, second);
    assert_eq!(keys, expected_keys);
}

#[test]
fn high_card_draw_zero_is_empty() {
    // Act
    let cards = high_card_draw(42, 0);

    // Assert
    assert_eq!(cards, Vec::new());
}

#[test]
fn high_card_draw_fifty_two_is_a_whole_deck() {
    // Act
    let cards = high_card_draw(42, 52);
    let mut codes: Vec<String> = cards.iter().map(Card::code).collect();
    codes.sort();
    codes.dedup();

    // Assert
    assert_eq!(cards.len(), 52);
    assert_eq!(codes.len(), 52);
    assert!(cards.iter().all(|card| !card.is_joker()));
}

#[test]
fn high_card_draw_matches_the_fresh_deck_prefix() {
    // Arrange
    let file = golden();
    let entry = file
        .shoe
        .into_iter()
        .find(|case| {
            case.seed == 42
                && case.decks == 1
                && case.jokers_per_deck == 0
                && case.mode == "per_deck"
        })
        .expect("golden deck case");
    let prefix: Vec<String> = entry.codes.into_iter().take(3).collect();

    // Act
    let actual: Vec<String> = high_card_draw(42, 3).iter().map(Card::code).collect();

    // Assert
    assert_eq!(actual, prefix);
}

#[test]
fn high_card_draw_beyond_the_deck_is_capped() {
    // Act
    let cards = high_card_draw(42, 100);

    // Assert
    assert_eq!(cards.len(), 52);
}
