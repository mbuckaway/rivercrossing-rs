// SPDX-License-Identifier: GPL-3.0-only

use pretty_assertions::assert_eq;

use super::{Card, Rank, Suit};
use crate::error::CardsError;
use crate::shoe::natural_deck;

#[test]
fn code_round_trips_every_card() {
    // Arrange
    let joker = Card::Joker;

    // Act
    let mismatches: Vec<String> = natural_deck(1)
        .iter()
        .chain(std::iter::once(&joker))
        .filter_map(|card| {
            let code = card.code();
            let parsed = Card::parse(&code);
            (parsed.as_ref().ok() != Some(card)).then(|| format!("{code} parsed as {parsed:?}"))
        })
        .collect();

    // Assert
    assert_eq!(mismatches, Vec::<String>::new());
}

#[test]
fn joker_code_is_jk() {
    // Act
    let code = Card::Joker.code();

    // Assert
    assert_eq!(code, "JK");
}

#[test]
fn tens_are_spelled_ten() {
    // Arrange
    let card = Card::Natural {
        rank: Rank::Ten,
        suit: Suit::Clubs,
    };

    // Act
    let code = card.code();

    // Assert
    assert_eq!(code, "10C");
}

#[test]
fn parse_rejects_unknown_rank() {
    // Act
    let result = Card::parse("1C");

    // Assert
    assert_eq!(
        result,
        Err(CardsError::InvalidCode {
            code: "1C".to_owned()
        })
    );
}

#[test]
fn parse_rejects_empty() {
    // Act
    let result = Card::parse("");

    // Assert
    assert_eq!(
        result,
        Err(CardsError::InvalidCode {
            code: String::new()
        })
    );
}

#[test]
fn parse_rejects_unknown_suit() {
    // Act
    let result = Card::parse("2X");

    // Assert
    assert_eq!(
        result,
        Err(CardsError::InvalidCode {
            code: "2X".to_owned()
        })
    );
}

#[test]
fn parse_accepts_joker() {
    // Act
    let result = Card::parse("JK");

    // Assert
    assert_eq!(result, Ok(Card::Joker));
}

#[test]
fn rank_from_u8_maps_every_pip() {
    // Act
    let ranks: Vec<Rank> = (2..=14)
        .filter_map(|value| Rank::from_u8(value).ok())
        .collect();

    // Assert
    assert_eq!(ranks, Rank::ALL.to_vec());
}

#[test]
fn rank_from_u8_rejects_out_of_range() {
    // Act
    let zero = Rank::from_u8(0);
    let one = Rank::from_u8(1);
    let fifteen = Rank::from_u8(15);

    // Assert
    assert_eq!(
        zero,
        Err(CardsError::InvalidCode {
            code: "0".to_owned()
        })
    );
    assert_eq!(
        one,
        Err(CardsError::InvalidCode {
            code: "1".to_owned()
        })
    );
    assert_eq!(
        fifteen,
        Err(CardsError::InvalidCode {
            code: "15".to_owned()
        })
    );
}

#[test]
fn rank_value_matches_the_pip() {
    // Act
    let values = (Rank::Two.value(), Rank::Ten.value(), Rank::Ace.value());

    // Assert
    assert_eq!(values, (2, 10, 14));
}

#[test]
fn card_serialises_as_its_code() {
    // Arrange
    let ace = Card::Natural {
        rank: Rank::Ace,
        suit: Suit::Spades,
    };

    // Act
    let natural = serde_json::to_string(&ace).expect("a natural card serialises");
    let joker = serde_json::to_string(&Card::Joker).expect("a joker serialises");

    // Assert
    assert_eq!(natural, "\"AS\"");
    assert_eq!(joker, "\"JK\"");
}

#[test]
fn card_deserialises_from_its_code() {
    // Act
    let card = serde_json::from_str::<Card>("\"10H\"").expect("10H is a card code");

    // Assert
    assert_eq!(
        card,
        Card::Natural {
            rank: Rank::Ten,
            suit: Suit::Hearts
        }
    );
}

#[test]
fn suit_and_rank_match_the_db_form() {
    // Act
    let suits: Vec<String> = Suit::ALL
        .iter()
        .map(|suit| serde_json::to_string(suit).expect("a suit serialises"))
        .collect();
    let ranks: Vec<String> = Rank::ALL
        .iter()
        .map(|rank| serde_json::to_string(rank).expect("a rank serialises"))
        .collect();

    // Assert
    assert_eq!(suits, vec!["\"c\"", "\"d\"", "\"h\"", "\"s\""]);
    assert_eq!(
        ranks,
        vec![
            "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14"
        ]
    );
    assert_eq!(
        serde_json::from_str::<Suit>("\"s\"").expect("s is a suit letter"),
        Suit::Spades
    );
    assert_eq!(
        serde_json::from_str::<Rank>("10").expect("10 is a pip value"),
        Rank::Ten
    );
}

#[test]
fn invalid_code_deserialisation_is_an_error() {
    // Act
    let result = serde_json::from_str::<Card>("\"XX\"");
    let message = serde_json::from_str::<Card>("\"XX\"")
        .expect_err("XX is not a card code")
        .to_string();

    // Assert
    assert_eq!(
        result.map_err(|error| error.classify()),
        Err(serde_json::error::Category::Data)
    );
    assert!(message.contains("XX"));
}

#[test]
fn invalid_rank_deserialisation_is_an_error() {
    // Act
    let result = serde_json::from_str::<Rank>("15");

    // Assert
    assert_eq!(
        result.map_err(|error| error.classify()),
        Err(serde_json::error::Category::Data)
    );
}

#[test]
fn invalid_suit_deserialisation_is_an_error() {
    // Act
    let result = serde_json::from_str::<Suit>("\"x\"");

    // Assert
    assert_eq!(
        result.map_err(|error| error.classify()),
        Err(serde_json::error::Category::Data)
    );
}
