// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the exporter's domain model and its pinned oracle constants.

#![expect(
    clippy::float_cmp,
    reason = "the oracle's default loop length is exactly eight kilometres"
)]

use pretty_assertions::assert_eq;
use rivercrossing_cards::{Card, Rank, Suit};
use rstest::rstest;

use super::{
    CardPair, ExportInput, ExportOptions, KICKER, RideInfo, SELF_TEST_NOTE, TRANSPARENT_PNG,
};

fn natural(rank: Rank, suit: Suit) -> Card {
    Card::Natural { rank, suit }
}

fn pair(rank: &str, suit: &str) -> CardPair {
    CardPair {
        rank: rank.to_owned(),
        suit: suit.to_owned(),
    }
}

#[test]
fn test_export_options_default_matches_the_oracle_publish_settings() {
    // Act
    let options = ExportOptions::default();

    // Assert
    assert!(!options.show_times);
    assert!(options.laps_board);
    assert!(!options.time_board);
    assert!(options.full_field);
    assert!(options.all_cards);
    assert_eq!(options.lap_km, 8.0);
}

#[test]
fn test_card_pair_from_card_given_a_ten_of_diamonds_reads_ten_and_d() {
    // Act
    let result = CardPair::from_card(&natural(Rank::Ten, Suit::Diamonds));

    // Assert
    assert_eq!(result, pair("10", "d"));
}

#[test]
fn test_card_pair_from_card_given_a_joker_reads_jk_and_j() {
    // Act
    let result = CardPair::from_card(&Card::Joker);

    // Assert
    assert_eq!(result, pair("JK", "j"));
}

#[rstest]
#[case(Rank::Two, "2")]
#[case(Rank::Three, "3")]
#[case(Rank::Four, "4")]
#[case(Rank::Five, "5")]
#[case(Rank::Six, "6")]
#[case(Rank::Seven, "7")]
#[case(Rank::Eight, "8")]
#[case(Rank::Nine, "9")]
#[case(Rank::Ten, "10")]
#[case(Rank::Jack, "J")]
#[case(Rank::Queen, "Q")]
#[case(Rank::King, "K")]
#[case(Rank::Ace, "A")]
fn test_card_pair_from_card_given_each_rank_reads_its_record_token(
    #[case] rank: Rank,
    #[case] expected: &str,
) {
    // Act
    let result = CardPair::from_card(&natural(rank, Suit::Spades));

    // Assert
    assert_eq!(result.rank, expected);
}

#[rstest]
#[case(Suit::Clubs, "c")]
#[case(Suit::Diamonds, "d")]
#[case(Suit::Hearts, "h")]
#[case(Suit::Spades, "s")]
fn test_card_pair_from_card_given_each_suit_reads_its_lowercase_letter(
    #[case] suit: Suit,
    #[case] expected: &str,
) {
    // Act
    let result = CardPair::from_card(&natural(Rank::Two, suit));

    // Assert
    assert_eq!(result.suit, expected);
}

#[test]
fn test_card_pair_clone_equals_the_original() {
    // Arrange
    let original = pair("9", "s");

    // Act
    let copy = original.clone();

    // Assert
    assert_eq!(copy, original);
}

#[test]
fn test_card_pair_given_a_different_suit_is_not_equal() {
    // Arrange
    let spade = pair("9", "s");
    let heart = pair("9", "h");

    // Act
    let equal = spade == heart;

    // Assert
    assert!(!equal);
}

#[test]
fn test_kicker_matches_the_oracle_text() {
    // Act
    let kicker = KICKER;

    // Assert
    assert_eq!(kicker, "Official results · poker run");
}

#[test]
fn test_self_test_note_matches_the_oracle_text() {
    // Act
    let note = SELF_TEST_NOTE;

    // Assert
    assert_eq!(
        note,
        "Self-test unverified — these results were published over a failed evaluator self-test"
    );
}

#[test]
fn test_transparent_png_matches_the_oracle_data_uri() {
    // Act
    let data_uri = TRANSPARENT_PNG;

    // Assert
    assert_eq!(
        data_uri,
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="
    );
}

fn export_input() -> ExportInput {
    let mut team_logos = std::collections::BTreeMap::new();
    team_logos.insert("88".to_owned(), "data:image/png;base64,AA==".to_owned());
    ExportInput {
        ride: RideInfo {
            name: "Test Poker Run 2026".to_owned(),
            event_date: time::Date::from_calendar_date(2026, time::Month::June, 6)
                .expect("the fixture date is valid"),
            venue: "Test Venue".to_owned(),
            lap_km: 8.0,
            organizer: "Test Org".to_owned(),
            scorer: "T. Ester".to_owned(),
        },
        placed: Vec::new(),
        options: ExportOptions::default(),
        generated: "Generated 16:07, Sept 20 2026".to_owned(),
        logo_src: Some("data:image/png;base64,BB==".to_owned()),
        team_logos,
        riders: 12,
        self_test_unverified: true,
        tie_note: Some("draw required".to_owned()),
    }
}

#[test]
fn test_export_input_given_a_fixture_exposes_every_field() {
    // Arrange
    let input = export_input();

    // Act
    let facts = (
        input.ride.name.as_str(),
        input.placed.len(),
        input.options.laps_board,
        input.generated.as_str(),
        input.logo_src.as_deref(),
        input.team_logos.get("88").map(String::as_str),
        input.riders,
        input.self_test_unverified,
        input.tie_note.as_deref(),
    );

    // Assert
    assert_eq!(
        facts,
        (
            "Test Poker Run 2026",
            0,
            true,
            "Generated 16:07, Sept 20 2026",
            Some("data:image/png;base64,BB=="),
            Some("data:image/png;base64,AA=="),
            12,
            true,
            Some("draw required")
        )
    );
}

#[test]
fn test_export_input_debug_names_the_type() {
    // Arrange
    let input = export_input();

    // Act
    let debug = format!("{input:?}");

    // Assert
    assert!(debug.starts_with("ExportInput {"), "debug was {debug}");
}
