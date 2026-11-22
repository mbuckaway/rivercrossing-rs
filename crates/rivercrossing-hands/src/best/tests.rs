// SPDX-License-Identifier: GPL-3.0-only

use pretty_assertions::assert_eq;
use rivercrossing_cards::{Card, high_card_draw};
use rstest::rstest;

use super::{best_hand, grouped_candidate};
use crate::class::HandClass;
use crate::eval::{EvaluatedHand, eval5};
use crate::vectors::joker_vector_rows;

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

fn key(hand: &EvaluatedHand) -> (HandClass, Vec<u8>) {
    (hand.cls(), hand.tiebreak().to_vec())
}

fn assert_pool_never_weakens(pool: &[Card]) {
    for size in 1..pool.len() {
        let smaller_window = pool.get(..size);
        let larger_window = pool.get(..size.saturating_add(1));
        if let (Some(smaller_cards), Some(larger_cards)) = (smaller_window, larger_window) {
            let smaller = best_hand(smaller_cards);
            let larger = best_hand(larger_cards);
            assert!(
                key(&smaller) <= key(&larger),
                "the pool of {size} cards weakened when card {size} arrived"
            );
        }
    }
}

#[test]
fn test_best_hand_empty_pool_is_a_high_card_with_a_zero_count_tiebreak() {
    // Act
    let hand = best_hand(&cards(&[]));

    // Assert
    assert_eq!(hand.cls(), HandClass::HighCard);
    assert_eq!(hand.tiebreak().to_vec(), vec![0]);
    assert_eq!(hand.best5().to_vec(), Vec::<Card>::new());
}

#[test]
fn test_best_hand_one_card_partial_is_high_card_with_its_rank() {
    // Act
    let hand = best_hand(&cards(&["AS"]));

    // Assert
    assert_eq!(hand.cls(), HandClass::HighCard);
    assert_eq!(hand.tiebreak().to_vec(), vec![1, 14]);
    assert_eq!(hand.best5().to_vec(), cards(&["AS"]));
}

#[test]
fn test_best_hand_a_lone_joker_is_an_ace_high_high_card() {
    // Act
    let hand = best_hand(&cards(&["JK"]));

    // Assert
    assert_eq!(hand.cls(), HandClass::HighCard);
    assert_eq!(hand.tiebreak().to_vec(), vec![1, 14]);
    assert_eq!(codes_of(hand.jokers_played_as()), vec!["AC".to_owned()]);
}

#[rstest]
#[case::distinct(&["AS", "KS", "QH", "JD"], HandClass::HighCard, &[4, 14, 13, 12, 11])]
#[case::pair(&["AS", "AD", "QH", "JD"], HandClass::Pair, &[4, 14, 12, 11])]
#[case::two_pair(&["AS", "AD", "QH", "QD"], HandClass::TwoPair, &[4, 14, 12])]
#[case::trips(&["AS", "AD", "AH", "JD"], HandClass::Trips, &[4, 14, 11])]
#[case::quads(&["AS", "AD", "AH", "AC"], HandClass::Quads, &[4, 14])]
fn test_best_hand_four_card_partial_orders_by_class_then_kickers(
    #[case] codes: &[&str],
    #[case] class: HandClass,
    #[case] tiebreak: &[u8],
) {
    // Act
    let hand = best_hand(&cards(codes));

    // Assert
    assert_eq!(hand.cls(), class);
    assert_eq!(hand.tiebreak().to_vec(), tiebreak.to_vec());
    assert_eq!(hand.best5().to_vec(), cards(codes));
}

#[test]
fn test_best_hand_four_naturals_and_a_joker_makes_five() {
    // Act
    let hand = best_hand(&cards(&["AS", "KS", "QS", "JS", "JK"]));

    // Assert
    assert_eq!(hand.cls(), HandClass::RoyalFlush);
    assert_eq!(hand.tiebreak().to_vec(), vec![5]);
    assert_eq!(hand.best5().len(), 5);
    assert_eq!(codes_of(hand.jokers_played_as()), vec!["10S".to_owned()]);
}

#[test]
fn test_best_hand_surplus_jokers_stay_unused() {
    // Act
    let hand = best_hand(&cards(&["10S", "JS", "QS", "KS", "AS", "JK", "JK", "JK"]));

    // Assert
    assert_eq!(hand.cls(), HandClass::RoyalFlush);
    assert_eq!(hand.tiebreak().to_vec(), vec![5]);
    assert_eq!(hand.jokers_played_as().to_vec(), Vec::<Card>::new());
}

#[test]
fn test_best_hand_pool_of_ten_with_three_jokers_matches_the_best_five() {
    // Arrange
    let pool = cards(&["KC", "QC", "2D", "3H", "4S", "5D", "6H", "JK", "JK", "JK"]);
    let best_five = evaluated(&["KC", "QC", "AC", "JC", "10C"]);

    // Act
    let hand = best_hand(&pool);

    // Assert
    assert_eq!(key(&hand), key(&best_five));
    assert_eq!(
        codes_of(hand.jokers_played_as()),
        vec!["AC".to_owned(), "JC".to_owned(), "10C".to_owned()]
    );
}

#[test]
fn test_best_hand_uncapped_pool_is_linear_not_exponential() {
    // Arrange
    let pool = high_card_draw(7, 30);

    // Act
    let hand = best_hand(&pool);
    let replay = eval5(hand.best5()).expect("the best five holds five cards");

    // Assert
    assert_eq!(hand.best5().len(), 5);
    assert!(hand.best5().iter().all(|card| pool.contains(card)));
    assert_eq!(hand.jokers_played_as().to_vec(), Vec::<Card>::new());
    assert_eq!(key(&replay), key(&hand));
}

#[test]
fn test_best_hand_add_a_card_never_weakens_the_hand() {
    // Arrange
    let pool = high_card_draw(20_260_807, 20);

    // Act
    assert_pool_never_weakens(&pool);

    // Assert
    assert_eq!(pool.len(), 20);
}

#[rstest]
#[case::six_jokers(Vec::new(), 6)]
#[case::twelve_jokers(cards(&["AS"]), 12)]
fn test_best_hand_never_plays_more_than_five_jokers(
    #[case] naturals: Vec<Card>,
    #[case] joker_count: usize,
) {
    // Arrange
    let mut pool = naturals;
    pool.extend(std::iter::repeat_n(Card::Joker, joker_count));

    // Act
    let hand = best_hand(&pool);

    // Assert
    assert_eq!(hand.cls(), HandClass::FiveOfAKind);
    assert_eq!(hand.tiebreak().to_vec(), vec![5, 14]);
    assert_eq!(hand.best5().len(), 5);
    assert!(hand.jokers_played_as().len() <= 5);
}

#[test]
fn test_best_hand_partial_hand_missing_kicker_ranks_below_a_present_one() {
    // Arrange
    let four_cards = best_hand(&cards(&["AS", "KD", "QH", "JC"]));
    let five_cards = best_hand(&cards(&["AS", "KD", "QH", "JC", "9S"]));

    // Act
    let four_key = key(&four_cards);
    let five_key = key(&five_cards);

    // Assert
    assert_eq!(four_key, (HandClass::HighCard, vec![4, 14, 13, 12, 11]));
    assert_eq!(five_key, (HandClass::HighCard, vec![5, 14, 13, 12, 11, 9]));
    assert!(four_key < five_key);
}

#[test]
fn test_best_hand_card_cap_is_the_callers_slice() {
    // Arrange
    let pool = cards(&["2C", "3D", "5H", "7S", "9C", "10S", "JS", "QS", "KS", "AS"]);
    let cap = pool.get(..5).expect("the pool holds five cards");

    // Act
    let capped = best_hand(cap);
    let whole = best_hand(&pool);

    // Assert
    assert_eq!(
        key(&capped),
        key(&evaluated(&["2C", "3D", "5H", "7S", "9C"]))
    );
    assert_eq!(key(&whole), (HandClass::RoyalFlush, vec![5]));
    assert!(key(&capped) < key(&whole));
}

#[test]
fn test_best_hand_leftover_jokers_fill_the_chosen_shape() {
    // Arrange
    let naturals = cards(&["AS", "AD", "AH", "AC", "KS"]);
    let jokers = cards(&["JK", "JK"]);

    // Act
    let hand = grouped_candidate(&naturals, &jokers, &[4]).expect("four of a kind is reachable");

    // Assert
    assert_eq!(hand.cls(), HandClass::Quads);
    assert_eq!(hand.tiebreak().to_vec(), vec![5, 14, 13]);
    assert_eq!(
        codes_of(hand.jokers_played_as()),
        vec!["AC".to_owned(), "AD".to_owned()]
    );
    assert_eq!(hand.best5().len(), 5);
}

#[test]
fn test_best_hand_a_straight_completing_rank_already_held_is_not_counted_twice() {
    // Act
    let hand = best_hand(&cards(&["6S", "5S", "4S", "3C", "2S", "JK"]));

    // Assert
    assert_eq!(hand.cls(), HandClass::StraightFlush);
    assert_eq!(hand.tiebreak().to_vec(), vec![5, 6]);
    assert_eq!(codes_of(hand.jokers_played_as()), vec!["3S".to_owned()]);
}

#[test]
fn test_best_hand_five_naturals_of_one_rank_stay_quads() {
    // Act
    let hand = best_hand(&cards(&["9S", "9S", "9S", "9S", "9S"]));

    // Assert
    assert_eq!(hand.cls(), HandClass::Quads);
    assert_eq!(hand.tiebreak().to_vec(), vec![5, 9]);
    assert_eq!(hand.jokers_played_as().to_vec(), Vec::<Card>::new());
}

#[test]
fn test_best_hand_best_five_round_trips_through_serde_json() {
    // Arrange
    let hand = best_hand(&cards(&["10S", "JS", "QS", "KS", "AS"]));
    let text = serde_json::to_string(hand.best5()).expect("the cards serialise");

    // Act
    let round_trip: Vec<Card> = serde_json::from_str(&text).expect("the cards parse");

    // Assert
    assert_eq!(round_trip, hand.best5().to_vec());
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
fn test_best_hand_on_five_cards_equals_eval5(#[case] index: usize) {
    // Arrange
    let rows = joker_vector_rows().expect("the committed joker table parses");
    let row = rows.get(index).expect("the table holds the row");

    // Act
    let best = best_hand(row.cards());
    let five = eval5(row.cards()).expect("the row holds five cards");

    // Assert
    assert_eq!(key(&best), key(&five));
}
