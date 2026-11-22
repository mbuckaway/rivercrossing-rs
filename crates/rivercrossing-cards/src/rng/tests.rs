// SPDX-License-Identifier: GPL-3.0-only

use pretty_assertions::assert_eq;
use rstest::rstest;
use serde::Deserialize;

use super::CpRandom;
use crate::card::Card;
use crate::shoe::natural_deck;

const MT_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/mt19937.json"
));
const SHOE_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/shoe-golden.json"
));

#[derive(Deserialize)]
struct MtFixture {
    #[serde(rename = "getrandbits32")]
    getrandbits_32: Vec<BitsCase>,
    randbelow: Vec<BelowCase>,
}

#[derive(Deserialize)]
struct BitsCase {
    seed: u64,
    values: Vec<u32>,
}

#[derive(Deserialize)]
struct BelowCase {
    seed: u64,
    bound: usize,
    values: Vec<usize>,
}

#[derive(Deserialize)]
struct ShoeFixture {
    shoe: Vec<ShoeCase>,
}

#[derive(Deserialize)]
struct ShoeCase {
    seed: u64,
    decks: u8,
    jokers_per_deck: u8,
    mode: String,
    codes: Vec<String>,
}

fn mt_fixture() -> MtFixture {
    serde_json::from_str(MT_FIXTURE).expect("mt19937.json parses")
}

fn expected_bits(seed: u64) -> Vec<u32> {
    mt_fixture()
        .getrandbits_32
        .into_iter()
        .find(|case| case.seed == seed)
        .expect("fixture holds the seed")
        .values
}

fn draw_bits(seed: u64, count: usize) -> Vec<u32> {
    let mut random = CpRandom::new(seed);
    (0..count).map(|_| random.getrandbits(32)).collect()
}

fn golden_deck_codes(seed: u64) -> Vec<String> {
    let fixture: ShoeFixture = serde_json::from_str(SHOE_FIXTURE).expect("shoe-golden.json parses");
    fixture
        .shoe
        .into_iter()
        .find(|case| {
            case.seed == seed
                && case.decks == 1
                && case.jokers_per_deck == 0
                && case.mode == "per_deck"
        })
        .expect("golden shoe case")
        .codes
}

#[test]
fn getrandbits_matches_cpython_for_seed_zero() {
    // Act
    let actual = draw_bits(0, 4);

    // Assert
    assert_eq!(actual, expected_bits(0));
}

#[test]
fn getrandbits_matches_cpython_for_seed_one() {
    // Act
    let actual = draw_bits(1, 4);

    // Assert
    assert_eq!(actual, expected_bits(1));
}

#[test]
fn getrandbits_matches_cpython_for_seed_forty_two() {
    // Act
    let actual = draw_bits(42, 4);

    // Assert
    assert_eq!(actual, expected_bits(42));
}

#[test]
fn getrandbits_matches_cpython_for_seed_above_two_to_the_sixty_two() {
    // Arrange
    let seed = 4_611_686_018_427_387_905;

    // Act
    let actual = draw_bits(seed, 4);

    // Assert
    assert_eq!(actual, expected_bits(seed));
}

#[rstest]
#[case(0)]
#[case(1)]
#[case(42)]
fn randbelow_matches_cpython(#[case] seed: u64) {
    // Arrange
    let case = mt_fixture()
        .randbelow
        .into_iter()
        .find(|case| case.seed == seed)
        .expect("fixture holds the seed");
    let mut random = CpRandom::new(seed);

    // Act
    let actual: Vec<usize> = (0..case.values.len())
        .map(|_| random.randbelow(case.bound))
        .collect();

    // Assert
    assert_eq!(actual, case.values);
}

#[test]
fn randbelow_rejects_values_at_or_above_the_bound() {
    // Arrange
    let mut random = CpRandom::new(0);

    // Act
    let actual: Vec<usize> = (0..6).map(|_| random.randbelow(3)).collect();

    // Assert
    assert_eq!(actual, vec![1, 1, 0, 1, 2, 1]);
}

#[test]
fn randbelow_zero_and_one_return_zero() {
    // Arrange
    let mut random = CpRandom::new(7);

    // Act
    let values = (random.randbelow(0), random.randbelow(1));

    // Assert
    assert_eq!(values, (0, 0));
}

#[test]
fn shuffle_matches_cpython_for_fifty_two_cards() {
    // Arrange
    let mut cards = natural_deck(1);

    // Act
    CpRandom::new(42).shuffle(&mut cards);
    let codes: Vec<String> = cards.iter().map(Card::code).collect();

    // Assert
    assert_eq!(codes, golden_deck_codes(42));
}
