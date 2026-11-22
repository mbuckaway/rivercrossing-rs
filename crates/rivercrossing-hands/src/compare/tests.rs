// SPDX-License-Identifier: GPL-3.0-only

use std::cmp::Ordering;

use pretty_assertions::assert_eq;
use rivercrossing_cards::Card;

use super::compare;
use crate::class::HandClass;
use crate::eval::{EvaluatedHand, eval5};
use crate::self_test::seeded_compare_sample;

fn cards(codes: &[&str]) -> Vec<Card> {
    codes
        .iter()
        .map(|code| Card::parse(code).expect("the test card code parses"))
        .collect()
}

fn evaluated(codes: &[&str]) -> EvaluatedHand {
    eval5(&cards(codes)).expect("the test hand holds five cards")
}

fn wild_straight_flush(joker_count: usize) -> EvaluatedHand {
    EvaluatedHand::new(
        HandClass::StraightFlush,
        vec![5, 13],
        Vec::new(),
        vec![Card::Joker; joker_count],
    )
}

#[test]
fn test_compare_orders_by_class_first() {
    // Arrange
    let high_card = evaluated(&["AS", "KD", "QH", "JC", "9S"]);
    let pair = evaluated(&["2S", "2D", "3H", "4C", "5S"]);

    // Act
    let ordering = compare(&pair, &high_card);

    // Assert
    assert_eq!(ordering, Ordering::Greater);
}

#[test]
fn test_compare_orders_by_kickers_within_a_class() {
    // Arrange
    let ace_king = evaluated(&["AS", "AD", "KH", "QC", "2S"]);
    let ace_queen = evaluated(&["AH", "AC", "QH", "JC", "3S"]);

    // Act
    let ordering = compare(&ace_king, &ace_queen);

    // Assert
    assert_eq!(ordering, Ordering::Greater);
}

#[test]
fn test_compare_prefers_fewer_jokers() {
    // Arrange
    let one_joker = wild_straight_flush(1);
    let two_jokers = wild_straight_flush(2);

    // Act
    let ordering = compare(&one_joker, &two_jokers);

    // Assert
    assert_eq!(ordering, Ordering::Greater);
}

#[test]
fn test_compare_natural_beats_an_equal_wild_hand() {
    // Arrange
    let natural = evaluated(&["9S", "10S", "JS", "QS", "KS"]);
    let wild = evaluated(&["9S", "10S", "JS", "QS", "JK"]);

    // Act
    let ordering = compare(&natural, &wild);

    // Assert
    assert_eq!(ordering, Ordering::Greater);
}

#[test]
fn test_compare_five_of_a_kind_beats_a_royal_flush() {
    // Arrange
    let five_of_a_kind = evaluated(&["AS", "AD", "AH", "AC", "JK"]);
    let royal_flush = evaluated(&["AS", "KS", "QS", "JS", "10S"]);

    // Act
    let ordering = compare(&five_of_a_kind, &royal_flush);

    // Assert
    assert_eq!(ordering, Ordering::Greater);
}

#[test]
fn test_compare_is_antisymmetric() {
    // Arrange
    let sample = seeded_compare_sample();

    // Act
    let antisymmetric = sample.iter().all(|left| {
        sample
            .iter()
            .all(|right| compare(left, right) == compare(right, left).reverse())
    });

    // Assert
    assert_eq!(antisymmetric, true);
}

#[test]
fn test_compare_is_reflexive() {
    // Arrange
    let sample = seeded_compare_sample();

    // Act
    let reflexive = sample
        .iter()
        .all(|hand| compare(hand, hand) == Ordering::Equal);

    // Assert
    assert_eq!(reflexive, true);
}

#[test]
fn test_compare_is_transitive_over_a_seeded_sample() {
    // Arrange
    let sample = seeded_compare_sample();
    let mut ordered: Vec<&EvaluatedHand> = sample.iter().collect();
    ordered.sort_by(|left, right| compare(left, right));

    // Act
    let transitive = ordered.iter().enumerate().all(|(index, lower)| {
        ordered
            .iter()
            .skip(index + 1)
            .all(|higher| compare(lower, higher) != Ordering::Greater)
    });

    // Assert
    assert_eq!(transitive, true);
}
