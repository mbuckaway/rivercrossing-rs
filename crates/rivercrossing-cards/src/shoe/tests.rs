// SPDX-License-Identifier: GPL-3.0-only

use pretty_assertions::assert_eq;
use rstest::rstest;
use serde::Deserialize;

use super::{JokersMode, Shoe, natural_deck};
use crate::card::Card;
use crate::error::CardsError;

const GOLDEN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/shoe-golden.json"
));

#[derive(Deserialize)]
struct GoldenFile {
    shoe: Vec<GoldenShoe>,
    reshuffle: GoldenReshuffle,
}

#[derive(Deserialize)]
struct GoldenShoe {
    seed: u64,
    decks: u8,
    jokers_per_deck: u8,
    mode: String,
    size: usize,
    codes: Vec<String>,
}

#[derive(Deserialize)]
struct GoldenReshuffle {
    seed: u64,
    decks: u8,
    jokers_per_deck: u8,
    mode: String,
    cycle: u32,
    cycle_seed: u64,
    cycle2_first12: Vec<String>,
}

fn golden() -> GoldenFile {
    serde_json::from_str(GOLDEN).expect("shoe-golden.json parses")
}

fn mode_of(name: &str) -> JokersMode {
    match name {
        "per_deck" => JokersMode::PerDeck,
        "total" => JokersMode::Total,
        other => panic!("unknown jokers mode {other}"),
    }
}

fn drain(shoe: &mut Shoe) -> Vec<Card> {
    let mut cards = Vec::new();
    while let Ok((card, _index)) = shoe.deal() {
        cards.push(card);
    }
    cards
}

fn codes_of(cards: &[Card]) -> Vec<String> {
    cards.iter().map(Card::code).collect()
}

fn count_jokers(cards: &[Card]) -> usize {
    cards.iter().filter(|card| card.is_joker()).count()
}

fn deal_count(shoe: &mut Shoe, count: usize) -> usize {
    let mut jokers = 0;
    for _ in 0..count {
        let Ok((card, _index)) = shoe.deal() else {
            break;
        };
        jokers += usize::from(card.is_joker());
    }
    jokers
}

fn deal_until_joker(shoe: &mut Shoe) -> Option<Card> {
    loop {
        let Ok((card, _index)) = shoe.deal() else {
            return None;
        };
        if card.is_joker() {
            return Some(card);
        }
    }
}

#[test]
fn same_seed_same_sequence() {
    // Arrange
    let mut first = Shoe::new(2, 1, 1234, JokersMode::PerDeck).expect("the config is valid");
    let mut second = Shoe::new(2, 1, 1234, JokersMode::PerDeck).expect("the config is valid");

    // Act
    let left = codes_of(&drain(&mut first));
    let right = codes_of(&drain(&mut second));

    // Assert
    assert_eq!(left, right);
}

#[test]
fn different_seed_differs() {
    // Arrange
    let mut first = Shoe::new(1, 0, 0, JokersMode::PerDeck).expect("the config is valid");
    let mut second = Shoe::new(1, 0, 1, JokersMode::PerDeck).expect("the config is valid");

    // Act
    let left = codes_of(&drain(&mut first));
    let right = codes_of(&drain(&mut second));

    // Assert
    assert_ne!(left, right);
}

#[test]
fn composition_has_every_card_deck_times() {
    // Arrange
    let mut shoe = Shoe::new(2, 1, 9, JokersMode::PerDeck).expect("the config is valid");
    let deck = natural_deck(1);

    // Act
    let cards = drain(&mut shoe);
    let missing: Vec<Card> = deck
        .iter()
        .filter(|card| cards.iter().filter(|other| other == card).count() != 2)
        .cloned()
        .collect();

    // Assert
    assert_eq!(cards.len(), 106);
    assert_eq!(count_jokers(&cards), 2);
    assert_eq!(missing, Vec::new());
}

#[test]
fn deal_returns_zero_based_index() {
    // Arrange
    let mut shoe = Shoe::new(1, 0, 0, JokersMode::PerDeck).expect("the config is valid");

    // Act
    let (_first, first_index) = shoe.deal().expect("the first card");
    let (_second, second_index) = shoe.deal().expect("the second card");

    // Assert
    assert_eq!((first_index, second_index), (0, 1));
}

#[test]
fn deal_past_the_end_is_empty() {
    // Arrange
    let mut shoe = Shoe::replay(1, 0, 0, 52, 1, JokersMode::PerDeck).expect("the replay works");

    // Act
    let result = shoe.deal();

    // Assert
    assert_eq!(shoe.remaining(), 0);
    assert_eq!(result, Err(CardsError::ShoeEmpty));
}

#[test]
fn dealt_and_remaining_track_the_index() {
    // Arrange
    let shoe = Shoe::replay(1, 0, 0, 3, 1, JokersMode::PerDeck).expect("the replay works");

    // Act
    let dealt = shoe.dealt();
    let remaining = shoe.remaining();

    // Assert
    assert_eq!((dealt, remaining), (3, 49));
}

#[test]
fn new_rejects_zero_decks() {
    // Act
    let result = Shoe::new(0, 0, 0, JokersMode::PerDeck).map(|_shoe| ());

    // Assert
    assert_eq!(
        result,
        Err(CardsError::InvalidConfig {
            decks: 0,
            jokers_per_deck: 0
        })
    );
}

#[test]
fn new_rejects_more_than_ten_jokers_per_deck() {
    // Act
    let result = Shoe::new(1, 11, 0, JokersMode::PerDeck).map(|_shoe| ());

    // Assert
    assert_eq!(
        result,
        Err(CardsError::InvalidConfig {
            decks: 1,
            jokers_per_deck: 11
        })
    );
}

#[test]
fn new_accepts_ten_jokers_per_deck() {
    // Act
    let shoe = Shoe::new(1, 10, 0, JokersMode::Total).expect("10 jokers is the limit");

    // Assert
    assert_eq!(shoe.remaining(), 62);
    assert_eq!(shoe.jokers_in_cycle(), 10);
}

#[test]
fn reshuffle_moves_to_cycle_two_with_seed_plus_one() {
    // Arrange
    let reshuffle = golden().reshuffle;
    let mode = mode_of(&reshuffle.mode);
    let mut shoe = Shoe::new(
        reshuffle.decks,
        reshuffle.jokers_per_deck,
        reshuffle.seed,
        mode,
    )
    .expect("the golden config is valid");

    // Act
    shoe.reshuffle().expect("the shoe is open");
    let first12: Vec<String> = (0..12)
        .map(|_| shoe.deal().expect("card").0.code())
        .collect();
    let rest = codes_of(&drain(&mut shoe));
    let mut fresh = Shoe::new(
        reshuffle.decks,
        reshuffle.jokers_per_deck,
        reshuffle.cycle_seed,
        mode,
    )
    .expect("the golden config is valid");
    let fresh_codes = codes_of(&drain(&mut fresh));

    // Assert
    assert_eq!(shoe.cycle(), reshuffle.cycle);
    assert_eq!(shoe.seed(), reshuffle.cycle_seed - 1);
    assert_eq!(first12, reshuffle.cycle2_first12);
    assert_eq!(rest, fresh_codes[12..].to_vec());
}

#[test]
fn reshuffle_keeps_the_stored_seed() {
    // Arrange
    let mut shoe = Shoe::new(1, 0, 12345, JokersMode::PerDeck).expect("the config is valid");

    // Act
    shoe.reshuffle().expect("the shoe is open");

    // Assert
    assert_eq!(shoe.seed(), 12345);
    assert_eq!(shoe.cycle(), 2);
}

#[test]
fn restitute_returns_the_last_card() {
    // Arrange
    let mut shoe = Shoe::new(1, 0, 0, JokersMode::PerDeck).expect("the config is valid");
    let (first, _index) = shoe.deal().expect("the first card");

    // Act
    shoe.restitute(first.clone())
        .expect("the last card returns");
    let (again, index) = shoe.deal().expect("the first card again");

    // Assert
    assert_eq!(shoe.dealt(), 1);
    assert_eq!((again, index), (first, 0));
}

#[test]
fn restitute_rejects_a_different_card() {
    // Arrange
    let mut shoe = Shoe::new(1, 0, 0, JokersMode::PerDeck).expect("the config is valid");
    let (_first, _index) = shoe.deal().expect("the first card");

    // Act
    let result = shoe.restitute(Card::Joker);

    // Assert
    assert_eq!(
        result,
        Err(CardsError::NotLastDealt {
            code: "JK".to_owned()
        })
    );
    assert_eq!(shoe.dealt(), 1);
}

#[test]
fn restitute_before_any_deal_errors() {
    // Arrange
    let mut shoe = Shoe::new(1, 0, 0, JokersMode::PerDeck).expect("the config is valid");

    // Act
    let result = shoe.restitute(Card::Joker);

    // Assert
    assert_eq!(
        result,
        Err(CardsError::NotLastDealt {
            code: "JK".to_owned()
        })
    );
}

#[test]
fn closed_shoe_rejects_deal_reshuffle_and_restitute() {
    // Arrange
    let mut shoe = Shoe::new(1, 0, 0, JokersMode::PerDeck).expect("the config is valid");
    shoe.close();

    // Act
    let dealt = shoe.deal();
    let reshuffled = shoe.reshuffle();
    let restituted = shoe.restitute(Card::Joker);

    // Assert
    assert!(shoe.is_closed());
    assert_eq!(dealt, Err(CardsError::ShoeClosed));
    assert_eq!(reshuffled, Err(CardsError::ShoeClosed));
    assert_eq!(restituted, Err(CardsError::ShoeClosed));
}

#[test]
fn reopen_continues_the_sequence() {
    // Arrange
    let mut shoe = Shoe::new(1, 0, 0, JokersMode::PerDeck).expect("the config is valid");
    let (_first, _index) = shoe.deal().expect("the first card");
    shoe.close();

    // Act
    shoe.reopen();
    let (_second, index) = shoe.deal().expect("the second card");

    // Assert
    assert!(!shoe.is_closed());
    assert_eq!(index, 1);
}

#[test]
fn replay_reproduces_a_live_shoe() {
    // Arrange
    let mut live = Shoe::new(1, 0, 42, JokersMode::PerDeck).expect("the config is valid");
    let live_codes = codes_of(&drain(&mut live));

    // Act
    let mut replayed = Shoe::replay(1, 0, 42, 5, 1, JokersMode::PerDeck).expect("the replay works");
    let dealt = replayed.dealt();
    let remaining = replayed.remaining();
    let replayed_codes = codes_of(&drain(&mut replayed));

    // Assert
    assert_eq!((dealt, remaining), (5, 47));
    assert_eq!(replayed_codes, live_codes[5..].to_vec());
}

#[test]
fn replay_advances_the_cycle_before_dealing() {
    // Arrange
    let mut expected = Shoe::new(1, 0, 7, JokersMode::PerDeck).expect("the config is valid");
    drain(&mut expected);
    expected.reshuffle().expect("the shoe is open");

    // Act
    let mut replayed = Shoe::replay(1, 0, 7, 2, 2, JokersMode::PerDeck).expect("the replay works");
    let replayed_codes = codes_of(&drain(&mut replayed));
    let expected_codes = codes_of(&drain(&mut expected));

    // Assert
    assert_eq!(replayed.cycle(), 2);
    assert_eq!(replayed_codes, expected_codes[2..].to_vec());
}

#[test]
fn replay_rejects_a_bad_config() {
    // Act
    let result = Shoe::replay(0, 0, 0, 0, 1, JokersMode::PerDeck).map(|_shoe| ());

    // Assert
    assert_eq!(
        result,
        Err(CardsError::InvalidConfig {
            decks: 0,
            jokers_per_deck: 0
        })
    );
}

#[test]
fn replay_past_the_end_is_empty() {
    // Act
    let result = Shoe::replay(1, 0, 0, 53, 1, JokersMode::PerDeck).map(|_shoe| ());

    // Assert
    assert_eq!(result, Err(CardsError::ShoeEmpty));
}

#[test]
fn restitute_keeps_the_budget_for_a_natural_in_total_mode() {
    // Arrange
    let mut shoe = Shoe::new(1, 1, 0, JokersMode::Total).expect("the config is valid");
    let (card, _index) = shoe.deal().expect("the first card");

    // Act
    shoe.restitute(card.clone()).expect("the last card returns");
    let budget = shoe.jokers_in_cycle();

    // Assert
    assert!(!card.is_joker());
    assert_eq!(budget, 1);
}

#[test]
fn total_mode_spends_the_budget_once() {
    // Arrange
    let mut shoe = Shoe::new(1, 1, 0, JokersMode::Total).expect("the config is valid");

    // Act
    let first = drain(&mut shoe);
    let spent = shoe.jokers_in_cycle();
    shoe.reshuffle().expect("the shoe is open");
    let second = drain(&mut shoe);

    // Assert
    assert_eq!(count_jokers(&first), 1);
    assert_eq!(spent, 0);
    assert_eq!(shoe.jokers_in_cycle(), 0);
    assert_eq!(count_jokers(&second), 0);
    assert_eq!(second.len(), 52);
}

#[test]
fn total_mode_carries_a_partial_budget_across_cycles() {
    // Arrange
    let mut shoe = Shoe::new(1, 2, 7, JokersMode::Total).expect("the config is valid");

    // Act
    let joker = deal_until_joker(&mut shoe).expect("the cycle holds a joker");
    let spent = shoe.jokers_in_cycle();
    shoe.reshuffle().expect("the shoe is open");
    let carried = shoe.jokers_in_cycle();
    let second = drain(&mut shoe);

    // Assert
    assert_eq!(joker, Card::Joker);
    assert_eq!(spent, 1);
    assert_eq!(carried, 1);
    assert_eq!(count_jokers(&second), 1);
    assert_eq!(second.len(), 53);
}

#[test]
fn per_deck_mode_redeals_jokers_every_cycle() {
    // Arrange
    let mut shoe = Shoe::new(2, 1, 0, JokersMode::PerDeck).expect("the config is valid");

    // Act
    let first = drain(&mut shoe);
    shoe.reshuffle().expect("the shoe is open");
    let per_cycle = shoe.jokers_in_cycle();
    let second = drain(&mut shoe);

    // Assert
    assert_eq!(count_jokers(&first), 2);
    assert_eq!(per_cycle, 2);
    assert_eq!(count_jokers(&second), 2);
}

#[test]
fn naturals_never_touch_the_joker_budget() {
    // Arrange
    let mut shoe = Shoe::new(1, 1, 0, JokersMode::Total).expect("the config is valid");

    // Act
    let jokers = deal_count(&mut shoe, 10);

    // Assert
    assert_eq!(jokers, 0);
    assert_eq!(shoe.jokers_in_cycle(), 1);
}

#[test]
fn restitute_restores_a_spent_joker_budget() {
    // Arrange
    let mut shoe = Shoe::new(1, 1, 0, JokersMode::Total).expect("the config is valid");
    let joker = deal_until_joker(&mut shoe).expect("the cycle holds a joker");
    let spent = shoe.jokers_in_cycle();

    // Act
    shoe.restitute(joker).expect("the last card returns");
    let restored = shoe.jokers_in_cycle();

    // Assert
    assert_eq!(spent, 0);
    assert_eq!(restored, 1);
}

#[test]
fn seed_plus_cycle_wraps_without_panicking() {
    // Arrange
    let mut shoe = Shoe::new(1, 0, u64::MAX, JokersMode::PerDeck).expect("the config is valid");
    let mut wrapped = Shoe::new(1, 0, 0, JokersMode::PerDeck).expect("the config is valid");

    // Act
    shoe.reshuffle().expect("the shoe is open");

    // Assert
    assert_eq!(shoe.seed(), u64::MAX);
    assert_eq!(shoe.cycle(), 2);
    assert_eq!(codes_of(&drain(&mut shoe)), codes_of(&drain(&mut wrapped)));
}

#[rstest]
#[case(0)]
#[case(1)]
#[case(2)]
#[case(3)]
#[case(4)]
#[case(5)]
#[case(6)]
#[case(7)]
#[case(8)]
#[case(9)]
#[case(10)]
#[case(11)]
#[case(12)]
#[case(13)]
#[case(14)]
#[case(15)]
#[case(16)]
#[case(17)]
#[case(18)]
#[case(19)]
#[case(20)]
#[case(21)]
#[case(22)]
#[case(23)]
#[case(24)]
#[case(25)]
#[case(26)]
#[case(27)]
#[case(28)]
#[case(29)]
#[case(30)]
#[case(31)]
#[case(32)]
#[case(33)]
#[case(34)]
#[case(35)]
#[case(36)]
#[case(37)]
#[case(38)]
#[case(39)]
#[case(40)]
#[case(41)]
#[case(42)]
#[case(43)]
#[case(44)]
#[case(45)]
#[case(46)]
#[case(47)]
#[case(48)]
#[case(49)]
#[case(50)]
#[case(51)]
#[case(52)]
#[case(53)]
#[case(54)]
#[case(55)]
#[case(56)]
#[case(57)]
#[case(58)]
#[case(59)]
fn golden_shoe_sequences_match_the_oracle(#[case] index: usize) {
    // Arrange
    let entry = golden()
        .shoe
        .into_iter()
        .nth(index)
        .expect("the fixture holds the entry");
    let mut shoe = Shoe::new(
        entry.decks,
        entry.jokers_per_deck,
        entry.seed,
        mode_of(&entry.mode),
    )
    .expect("the golden config is valid");

    // Act
    let codes = codes_of(&drain(&mut shoe));

    // Assert
    assert_eq!(codes.len(), entry.size);
    assert_eq!(codes, entry.codes);
}

#[test]
fn composition_matches_the_rank_major_order() {
    // Arrange
    let mut shoe = Shoe::new(1, 0, 0, JokersMode::PerDeck).expect("the config is valid");
    let deck = natural_deck(1);

    // Act
    let codes = codes_of(&drain(&mut shoe));
    let deck_codes = codes_of(&deck);
    let mut sorted = codes.clone();
    sorted.sort();
    let mut deck_sorted = deck_codes.clone();
    deck_sorted.sort();

    // Assert
    assert_eq!(sorted, deck_sorted);
    assert_eq!(deck_codes.first(), Some(&"2C".to_owned()));
}
