// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the hand-name renderer.
//!
//! The twelve golden strings come from the Python oracle's own golden cases,
//! em dash included; the extra cases pin the joker resolution, the partial
//! hands and the empty-hand refusal.

use pretty_assertions::assert_eq;
use rivercrossing_cards::Card;
use rivercrossing_hands::{EvaluatedHand, HandClass, best_hand};

use super::{RANK_WORD, full_house_label, hand_name, highest_count_rank, word};
use crate::error::StandingsError;

/// Evaluate the best hand of the space-separated card *codes*.
fn hand(codes: &str) -> EvaluatedHand {
    let cards: Vec<Card> = codes
        .split_whitespace()
        .map(|code| Card::parse(code).expect("the test card code parses"))
        .collect();
    best_hand(&cards)
}

/// Return the name of the best hand of the space-separated card *codes*.
fn named(codes: &str) -> Result<String, StandingsError> {
    hand_name(&hand(codes))
}

// ------------------------------------------------------- golden hand names

#[test]
fn test_hand_name_high_card_names_ace() -> Result<(), StandingsError> {
    // Act
    let name = named("AS QD 9H 5C 3S")?;

    // Assert
    assert_eq!(name, "High Card — Ace");
    Ok(())
}

#[test]
fn test_hand_name_pair_names_aces() -> Result<(), StandingsError> {
    // Act
    let name = named("AH AC KS JD 8H")?;

    // Assert
    assert_eq!(name, "Pair — Aces");
    Ok(())
}

#[test]
fn test_hand_name_two_pair_names_kings_and_fives() -> Result<(), StandingsError> {
    // Act
    let name = named("KH KC 5H 5D AS")?;

    // Assert
    assert_eq!(name, "Two Pair — Kings & Fives");
    Ok(())
}

#[test]
fn test_hand_name_trips_names_sevens() -> Result<(), StandingsError> {
    // Act
    let name = named("7H 7C 7D KS JH")?;

    // Assert
    assert_eq!(name, "Three of a Kind — Sevens");
    Ok(())
}

#[test]
fn test_hand_name_straight_names_nine_high() -> Result<(), StandingsError> {
    // Act
    let name = named("9H 8C 7D 6S 5H")?;

    // Assert
    assert_eq!(name, "Straight — Nine high");
    Ok(())
}

#[test]
fn test_hand_name_wheel_straight_names_five_high() -> Result<(), StandingsError> {
    // Act
    let name = named("AS 2H 3D 4C 5S")?;

    // Assert
    assert_eq!(name, "Straight — Five high");
    Ok(())
}

#[test]
fn test_hand_name_flush_names_king_high() -> Result<(), StandingsError> {
    // Act
    let name = named("KS JS 8S 6S 2S")?;

    // Assert
    assert_eq!(name, "Flush — King high");
    Ok(())
}

#[test]
fn test_hand_name_full_house_names_jacks_over_fours() -> Result<(), StandingsError> {
    // Act
    let name = named("JH JC JD 4H 4C")?;

    // Assert
    assert_eq!(name, "Full House — Jacks over Fours");
    Ok(())
}

#[test]
fn test_hand_name_quads_names_nines() -> Result<(), StandingsError> {
    // Act
    let name = named("9H 9C 9D 9S KS")?;

    // Assert
    assert_eq!(name, "Four of a Kind — Nines");
    Ok(())
}

#[test]
fn test_hand_name_straight_flush_names_queen_high() -> Result<(), StandingsError> {
    // Act
    let name = named("QS JS 10S 9S 8S")?;

    // Assert
    assert_eq!(name, "Straight Flush — Queen high");
    Ok(())
}

#[test]
fn test_hand_name_royal_flush_names_without_a_kicker() -> Result<(), StandingsError> {
    // Act
    let name = named("AS KS QS JS 10S")?;

    // Assert
    assert_eq!(name, "Royal Flush");
    Ok(())
}

#[test]
fn test_hand_name_five_of_a_kind_names_aces() -> Result<(), StandingsError> {
    // Act
    let name = named("AS AD AH AC JK")?;

    // Assert
    assert_eq!(name, "Five of a Kind — Aces");
    Ok(())
}

// --------------------------------------------------------- the extra cases

#[test]
fn test_hand_name_five_identical_naturals_name_their_quads_rank() -> Result<(), StandingsError> {
    // Act
    let name = named("9H 9H 9H 9H 9H")?;

    // Assert
    assert_eq!(name, "Four of a Kind — Nines");
    Ok(())
}

#[test]
fn test_hand_name_joker_full_house_names_the_jokers_resolution() -> Result<(), StandingsError> {
    // Act
    let name = named("AS AD JK 4C 4H")?;

    // Assert
    assert_eq!(name, "Full House — Aces over Fours");
    Ok(())
}

#[test]
fn test_hand_name_partial_four_card_hand_names_its_high_card() -> Result<(), StandingsError> {
    // Act
    let name = named("AS KD QH JC")?;

    // Assert
    assert_eq!(name, "High Card — Ace");
    Ok(())
}

#[test]
fn test_hand_name_partial_pair_names_aces() -> Result<(), StandingsError> {
    // Act
    let name = named("AH AC")?;

    // Assert
    assert_eq!(name, "Pair — Aces");
    Ok(())
}

#[test]
fn test_hand_name_partial_hand_with_a_joker_names_the_completed_pair() -> Result<(), StandingsError>
{
    // Act
    let name = named("AS KD QH JK")?;

    // Assert
    assert_eq!(name, "Pair — Aces");
    Ok(())
}

// ---------------------------------------------------------- refused hand

#[test]
fn test_hand_name_empty_hand_returns_empty_hand() {
    // Act
    let outcome = named("");

    // Assert
    assert_eq!(outcome.err(), Some(StandingsError::EmptyHand));
}

#[test]
fn test_hand_name_empty_hand_message_is_the_oracle_sentence() {
    // Act
    let message = named("").map_err(|error| error.to_string());

    // Assert
    assert_eq!(message, Err("cannot name an empty hand".to_owned()));
}

// --------------------------------------------------- the naming helpers

#[test]
fn test_hand_name_full_house_label_without_a_trips_count_returns_malformed_hand() {
    // Act
    let outcome = full_house_label(&[7, 7, 7, 7, 7]);

    // Assert
    assert_eq!(
        outcome.err(),
        Some(StandingsError::MalformedHand {
            class: HandClass::FullHouse,
        })
    );
}

#[test]
fn test_hand_name_highest_count_rank_breaks_a_count_tie_to_the_higher_rank() {
    // Act
    let rank = highest_count_rank(&[10, 10, 3, 3, 2]);

    // Assert
    assert_eq!(rank, 10);
}

#[test]
fn test_hand_name_rank_word_lookup_has_no_word_past_ace() {
    // Act
    let looked_up = word(&RANK_WORD, 15);

    // Assert
    assert_eq!(looked_up, "");
}
