// SPDX-License-Identifier: GPL-3.0-only

use std::collections::{BTreeMap, BTreeSet};

use pretty_assertions::assert_eq;
use rivercrossing_cards::{Card, Suit};
use rstest::rstest;

use super::{EvaluatedHand, cycle_suits, eval5};
use crate::class::HandClass;
use crate::error::HandsError;
use crate::vectors::{JokerVectorRow, joker_vector_rows, rank_sweep_rows};

fn cards(codes: &[&str]) -> Vec<Card> {
    codes
        .iter()
        .map(|code| Card::parse(code).expect("the test card code parses"))
        .collect()
}

fn evaluated(codes: &[&str]) -> EvaluatedHand {
    eval5(&cards(codes)).expect("the test hand holds five cards")
}

fn codes_of(cards: &[Card]) -> Vec<String> {
    cards.iter().map(Card::code).collect()
}

fn assert_joker_codes(hand: &EvaluatedHand, row: &JokerVectorRow) {
    if row.expected_joker_codes().is_empty() {
        return;
    }
    assert_eq!(
        codes_of(hand.jokers_played_as()),
        codes_of(row.expected_joker_codes())
    );
}

fn sweep_evaluations() -> Vec<(usize, HandClass, Vec<u8>)> {
    let rows = rank_sweep_rows().expect("the committed sweep parses");
    let mut evaluations = Vec::with_capacity(rows.len());
    for row in rows {
        let hand = eval5(row.cards()).expect("the sweep holds five cards");
        assert_eq!(
            hand.cls(),
            row.class(),
            "the sweep row {} class",
            row.rank()
        );
        evaluations.push((row.rank(), hand.cls(), hand.tiebreak().to_vec()));
    }
    evaluations
}

fn ordered_sweep_ranks() -> Vec<usize> {
    let mut evaluations = sweep_evaluations();
    evaluations.sort_by(|left, right| (right.1, &right.2).cmp(&(left.1, &left.2)));
    evaluations.into_iter().map(|(rank, _, _)| rank).collect()
}

fn distinct_sweep_keys() -> usize {
    let keys: BTreeSet<(HandClass, Vec<u8>)> = sweep_evaluations()
        .into_iter()
        .map(|(_, class, tiebreak)| (class, tiebreak))
        .collect();
    keys.len()
}

fn sweep_class_counts() -> BTreeMap<HandClass, usize> {
    let mut counts = BTreeMap::new();
    for (_, class, _) in sweep_evaluations() {
        *counts.entry(class).or_insert(0) += 1;
    }
    counts
}

fn committed_class_counts() -> BTreeMap<HandClass, usize> {
    BTreeMap::from([
        (HandClass::HighCard, 1277),
        (HandClass::Pair, 2860),
        (HandClass::TwoPair, 858),
        (HandClass::Trips, 858),
        (HandClass::Straight, 10),
        (HandClass::Flush, 1277),
        (HandClass::FullHouse, 156),
        (HandClass::Quads, 156),
        (HandClass::StraightFlush, 9),
        (HandClass::RoyalFlush, 1),
    ])
}

#[test]
fn test_eval5_royal_flush_has_a_bare_five_tiebreak() {
    // Act
    let hand = evaluated(&["10C", "JC", "QC", "KC", "AC"]);

    // Assert
    assert_eq!(hand.cls(), HandClass::RoyalFlush);
    assert_eq!(hand.tiebreak().to_vec(), vec![5]);
    assert_eq!(hand.jokers_played_as().to_vec(), Vec::<Card>::new());
}

#[test]
fn test_eval5_straight_flush_orders_by_high_card() {
    // Arrange
    let ten_high = evaluated(&["6D", "7D", "8D", "9D", "10D"]);
    let nine_high = evaluated(&["5D", "6D", "7D", "8D", "9D"]);

    // Act
    let ten_key = (ten_high.cls(), ten_high.tiebreak().to_vec());
    let nine_key = (nine_high.cls(), nine_high.tiebreak().to_vec());

    // Assert
    assert_eq!(ten_key, (HandClass::StraightFlush, vec![5, 10]));
    assert_eq!(nine_key, (HandClass::StraightFlush, vec![5, 9]));
    assert!(ten_key > nine_key);
}

#[test]
fn test_eval5_wheel_is_five_high_and_below_six_high() {
    // Arrange
    let wheel = evaluated(&["2C", "3C", "4C", "5C", "AC"]);
    let six_high = evaluated(&["2D", "3D", "4D", "5D", "6D"]);

    // Act
    let wheel_key = (wheel.cls(), wheel.tiebreak().to_vec());
    let six_key = (six_high.cls(), six_high.tiebreak().to_vec());

    // Assert
    assert_eq!(wheel_key, (HandClass::StraightFlush, vec![5, 5]));
    assert_eq!(six_key, (HandClass::StraightFlush, vec![5, 6]));
    assert!(wheel_key < six_key);
}

#[test]
fn test_eval5_flush_beats_straight() {
    // Arrange
    let flush = evaluated(&["2C", "4C", "6C", "8C", "10C"]);
    let straight = evaluated(&["9D", "10H", "JC", "QS", "KD"]);

    // Act
    let flush_class = flush.cls();
    let straight_class = straight.cls();

    // Assert
    assert_eq!(flush_class, HandClass::Flush);
    assert_eq!(straight_class, HandClass::Straight);
    assert!(flush_class > straight_class);
}

#[test]
fn test_eval5_quads_beats_full_house() {
    // Arrange
    let quads = evaluated(&["9C", "9D", "9H", "9S", "2C"]);
    let full_house = evaluated(&["KC", "KD", "KH", "2C", "2D"]);

    // Act
    let quads_class = quads.cls();
    let full_house_class = full_house.cls();

    // Assert
    assert_eq!(quads_class, HandClass::Quads);
    assert_eq!(full_house_class, HandClass::FullHouse);
    assert!(quads_class > full_house_class);
}

#[test]
fn test_eval5_quads_orders_by_rank_then_kicker() {
    // Arrange
    let ace_king = evaluated(&["AC", "AD", "AH", "AS", "KC"]);
    let ace_queen = evaluated(&["AC", "AD", "AH", "AS", "QC"]);
    let king_ace = evaluated(&["KC", "KD", "KH", "KS", "AC"]);

    // Act
    let ace_king_key = (ace_king.cls(), ace_king.tiebreak().to_vec());
    let ace_queen_key = (ace_queen.cls(), ace_queen.tiebreak().to_vec());
    let king_ace_key = (king_ace.cls(), king_ace.tiebreak().to_vec());

    // Assert
    assert_eq!(ace_king_key, (HandClass::Quads, vec![5, 14, 13]));
    assert_eq!(ace_queen_key, (HandClass::Quads, vec![5, 14, 12]));
    assert_eq!(king_ace_key, (HandClass::Quads, vec![5, 13, 14]));
    assert!(ace_king_key > ace_queen_key);
    assert!(ace_queen_key > king_ace_key);
}

#[test]
fn test_eval5_full_house_orders_by_trips_then_pair() {
    // Arrange
    let kings_over_aces = evaluated(&["KC", "KD", "KH", "AC", "AD"]);
    let kings_over_twos = evaluated(&["KC", "KD", "KH", "2C", "2D"]);
    let queens_over_aces = evaluated(&["QC", "QD", "QH", "AC", "AD"]);

    // Act
    let kings_over_aces_key = (kings_over_aces.cls(), kings_over_aces.tiebreak().to_vec());
    let kings_over_twos_key = (kings_over_twos.cls(), kings_over_twos.tiebreak().to_vec());
    let queens_over_aces_key = (queens_over_aces.cls(), queens_over_aces.tiebreak().to_vec());

    // Assert
    assert_eq!(kings_over_aces_key, (HandClass::FullHouse, vec![5, 13, 14]));
    assert_eq!(kings_over_twos_key, (HandClass::FullHouse, vec![5, 13, 2]));
    assert_eq!(
        queens_over_aces_key,
        (HandClass::FullHouse, vec![5, 12, 14])
    );
    assert!(kings_over_aces_key > kings_over_twos_key);
    assert!(kings_over_twos_key > queens_over_aces_key);
}

#[test]
fn test_eval5_two_pair_kicker_breaks_the_tie() {
    // Arrange
    let queen_kicker = evaluated(&["AC", "AD", "KC", "KD", "QC"]);
    let jack_kicker = evaluated(&["AC", "AD", "KC", "KD", "JC"]);

    // Act
    let queen_key = (queen_kicker.cls(), queen_kicker.tiebreak().to_vec());
    let jack_key = (jack_kicker.cls(), jack_kicker.tiebreak().to_vec());

    // Assert
    assert_eq!(queen_key, (HandClass::TwoPair, vec![5, 14, 13, 12]));
    assert_eq!(jack_key, (HandClass::TwoPair, vec![5, 14, 13, 11]));
    assert!(queen_key > jack_key);
}

#[test]
fn test_eval5_kickers_order_pairs_and_trips() {
    // Arrange
    let pair_with_jack = evaluated(&["AC", "AD", "KC", "QC", "JC"]);
    let pair_with_ten = evaluated(&["AC", "AD", "KC", "QC", "10C"]);
    let trips_with_two = evaluated(&["KC", "KD", "KH", "AC", "2C"]);
    let trips_with_three = evaluated(&["KC", "KD", "KH", "AC", "3C"]);

    // Act
    let pair_with_jack_key = (pair_with_jack.cls(), pair_with_jack.tiebreak().to_vec());
    let pair_with_ten_key = (pair_with_ten.cls(), pair_with_ten.tiebreak().to_vec());
    let trips_with_two_key = (trips_with_two.cls(), trips_with_two.tiebreak().to_vec());
    let trips_with_three_key = (trips_with_three.cls(), trips_with_three.tiebreak().to_vec());

    // Assert
    assert_eq!(
        pair_with_jack_key,
        (HandClass::Pair, vec![5, 14, 13, 12, 11])
    );
    assert_eq!(
        pair_with_ten_key,
        (HandClass::Pair, vec![5, 14, 13, 12, 10])
    );
    assert_eq!(trips_with_two_key, (HandClass::Trips, vec![5, 13, 14, 2]));
    assert_eq!(trips_with_three_key, (HandClass::Trips, vec![5, 13, 14, 3]));
    assert!(pair_with_jack_key > pair_with_ten_key);
    assert!(trips_with_three_key > trips_with_two_key);
}

#[test]
fn test_eval5_flush_keeps_repeated_ranks_in_its_kickers() {
    // Act
    let hand = evaluated(&["9H", "9H", "KH", "QH", "2H"]);

    // Assert
    assert_eq!(hand.cls(), HandClass::Flush);
    assert_eq!(hand.tiebreak().to_vec(), vec![5, 13, 12, 9, 9, 2]);
}

#[rstest]
#[case::four(&["2C", "3C", "4C", "5C"])]
#[case::six(&["2C", "3C", "4C", "5C", "6C", "7C"])]
fn test_eval5_rejects_four_and_six_cards(#[case] codes: &[&str]) {
    // Act
    let result = eval5(&cards(codes));

    // Assert
    assert_eq!(
        result,
        Err(HandsError::InvalidCardCount { count: codes.len() })
    );
}

#[test]
fn test_eval5_rejects_an_empty_slice() {
    // Act
    let result = eval5(&cards(&[]));

    // Assert
    assert_eq!(result, Err(HandsError::InvalidCardCount { count: 0 }));
}

#[test]
fn test_eval5_identical_cards_are_a_pair() {
    // Act
    let hand = evaluated(&["9H", "9H", "KS", "QD", "2C"]);

    // Assert
    assert_eq!(hand.cls(), HandClass::Pair);
    assert_eq!(hand.tiebreak().to_vec(), vec![5, 9, 13, 12, 2]);
}

#[test]
fn test_eval5_three_identical_cards_are_trips() {
    // Act
    let hand = evaluated(&["9H", "9H", "9H", "KS", "2C"]);

    // Assert
    assert_eq!(hand.cls(), HandClass::Trips);
    assert_eq!(hand.tiebreak().to_vec(), vec![5, 9, 13, 2]);
}

#[test]
fn test_eval5_four_identical_cards_are_quads() {
    // Act
    let hand = evaluated(&["9H", "9H", "9H", "9H", "KS"]);

    // Assert
    assert_eq!(hand.cls(), HandClass::Quads);
    assert_eq!(hand.tiebreak().to_vec(), vec![5, 9, 13]);
}

#[test]
fn test_eval5_five_naturals_of_one_rank_stay_quads() {
    // Act
    let hand = evaluated(&["9H", "9H", "9H", "9H", "9H"]);

    // Assert
    assert_eq!(hand.cls(), HandClass::Quads);
    assert_eq!(hand.tiebreak().to_vec(), vec![5, 9]);
}

#[test]
fn test_eval5_sweep_classes_match_the_committed_counts() {
    // Act
    let counts = sweep_class_counts();

    // Assert
    assert_eq!(counts, committed_class_counts());
}

#[test]
fn test_eval5_sweep_ordering_is_one_to_seven_thousand_four_hundred_sixty_two() {
    // Arrange
    let expected: Vec<usize> = (1..=7462).collect();

    // Act
    let ordered = ordered_sweep_ranks();
    let distinct = distinct_sweep_keys();

    // Assert
    assert_eq!(ordered, expected);
    assert_eq!(distinct, 7462);
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
fn test_eval5_joker_vector_matches_the_committed_row(#[case] index: usize) {
    // Arrange
    let rows = joker_vector_rows().expect("the committed joker table parses");
    let row = rows.get(index).expect("the table holds the row");

    // Act
    let hand = eval5(row.cards()).expect("the row holds five cards");

    // Assert
    assert_eq!(hand.cls(), row.expected_class());
    assert_eq!(hand.tiebreak().to_vec(), row.expected_tiebreak().to_vec());
    assert_joker_codes(&hand, row);
}

#[test]
fn test_eval5_a_joker_completes_the_highest_kicker() {
    // Act
    let hand = evaluated(&["KS", "JS", "8S", "6S", "JK"]);

    // Assert
    assert_eq!(hand.cls(), HandClass::Flush);
    assert_eq!(hand.tiebreak().to_vec(), vec![5, 14, 13, 11, 8, 6]);
    assert_eq!(codes_of(hand.jokers_played_as()), vec!["AS".to_owned()]);
}

#[test]
fn test_eval5_a_joker_prefers_a_straight_over_a_flush() {
    // Act
    let hand = evaluated(&["8H", "7H", "6H", "5H", "JK"]);

    // Assert
    assert_eq!(hand.cls(), HandClass::StraightFlush);
    assert_eq!(hand.tiebreak().to_vec(), vec![5, 9]);
    assert_eq!(codes_of(hand.jokers_played_as()), vec!["9H".to_owned()]);
}

#[test]
fn test_eval5_a_joker_takes_the_wheel_over_an_ace_high_flush() {
    // Act
    let hand = evaluated(&["AS", "2S", "3S", "4S", "JK"]);

    // Assert
    assert_eq!(hand.cls(), HandClass::StraightFlush);
    assert_eq!(hand.tiebreak().to_vec(), vec![5, 5]);
    assert_eq!(codes_of(hand.jokers_played_as()), vec!["5S".to_owned()]);
}

#[test]
fn test_eval5_two_jokers_build_the_top_straight_flush_window() {
    // Act
    let hand = evaluated(&["QS", "JS", "9S", "JK", "JK"]);

    // Assert
    assert_eq!(hand.cls(), HandClass::StraightFlush);
    assert_eq!(hand.tiebreak().to_vec(), vec![5, 13]);
    assert_eq!(
        codes_of(hand.jokers_played_as()),
        vec!["KS".to_owned(), "10S".to_owned()]
    );
}

#[test]
fn test_eval5_jokers_maximize_a_five_of_a_kind() {
    // Act
    let hand = evaluated(&["AS", "AD", "AH", "AC", "JK"]);

    // Assert
    assert_eq!(hand.cls(), HandClass::FiveOfAKind);
    assert_eq!(hand.tiebreak().to_vec(), vec![5, 14]);
    assert_eq!(codes_of(hand.jokers_played_as()), vec!["AC".to_owned()]);
}

#[test]
fn test_eval5_a_joker_may_duplicate_a_held_card() {
    // Act
    let hand = evaluated(&["9C", "9C", "9C", "9C", "JK"]);

    // Assert
    assert_eq!(hand.cls(), HandClass::FiveOfAKind);
    assert_eq!(hand.tiebreak().to_vec(), vec![5, 9]);
    assert_eq!(codes_of(hand.jokers_played_as()), vec!["9C".to_owned()]);
}

#[test]
fn test_eval5_uses_every_joker() {
    // Act
    let hand = evaluated(&["AS", "KS", "QS", "JK", "JK"]);

    // Assert
    assert_eq!(hand.cls(), HandClass::RoyalFlush);
    assert_eq!(hand.tiebreak().to_vec(), vec![5]);
    assert_eq!(hand.jokers_played_as().len(), 2);
    assert_eq!(
        codes_of(hand.jokers_played_as()),
        vec!["JS".to_owned(), "10S".to_owned()]
    );
}

#[test]
fn test_eval5_joker_suits_cycle_in_declaration_order() {
    // Act
    let suits = cycle_suits(5);

    // Assert
    assert_eq!(
        suits,
        vec![
            Suit::Clubs,
            Suit::Diamonds,
            Suit::Hearts,
            Suit::Spades,
            Suit::Clubs
        ]
    );
}

#[test]
fn test_eval5_no_straight_window_leaves_the_flush_candidate_to_win() {
    // Act
    let hand = evaluated(&["KC", "10C", "7C", "4C", "JK"]);

    // Assert
    assert_eq!(hand.cls(), HandClass::Flush);
    assert_eq!(hand.tiebreak().to_vec(), vec![5, 14, 13, 10, 7, 4]);
    assert_eq!(codes_of(hand.jokers_played_as()), vec!["AC".to_owned()]);
}
