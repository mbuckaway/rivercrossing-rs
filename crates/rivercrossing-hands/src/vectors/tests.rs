// SPDX-License-Identifier: GPL-3.0-only

use pretty_assertions::assert_eq;
use rivercrossing_cards::{Card, Rank};

use super::{
    JokerVectorRow, RankSweepRow, VectorParseError, data_rows, joker_vector_rows,
    parse_joker_vector_row, parse_rank_sweep_row, rank_sweep_rows,
};
use crate::class::HandClass;

fn card(code: &str) -> Card {
    Card::parse(code).expect("the test card code parses")
}

#[test]
fn test_vectors_rank_sweep_reads_every_row() {
    // Act
    let rows = rank_sweep_rows();

    // Assert
    assert_eq!(rows.as_ref().map(Vec::len), Ok(7462));
}

#[test]
fn test_vectors_rank_sweep_first_row_holds_the_royal_flush() {
    // Arrange
    let rows = rank_sweep_rows().expect("the committed sweep parses");
    let row: &RankSweepRow = rows.first().expect("the sweep holds a first row");

    // Act
    let cards: Vec<Card> = row.cards().to_vec();

    // Assert
    assert_eq!(cards, ["10C", "JC", "QC", "KC", "AC"].map(card));
    assert_eq!(row.rank(), 1);
    assert_eq!(row.class(), HandClass::RoyalFlush);
}

#[test]
fn test_vectors_rank_sweep_last_row_holds_the_worst_high_card() {
    // Arrange
    let rows = rank_sweep_rows().expect("the committed sweep parses");
    let row: &RankSweepRow = rows.last().expect("the sweep holds a last row");

    // Act
    let cards: Vec<Card> = row.cards().to_vec();

    // Assert
    assert_eq!(cards, ["2C", "3C", "4C", "5C", "7D"].map(card));
    assert_eq!(row.rank(), 7462);
    assert_eq!(row.class(), HandClass::HighCard);
}

#[test]
fn test_vectors_joker_vectors_read_every_row() {
    // Act
    let rows = joker_vector_rows();

    // Assert
    assert_eq!(rows.as_ref().map(Vec::len), Ok(28));
}

#[test]
fn test_vectors_joker_first_row_holds_the_five_aces() {
    // Arrange
    let rows = joker_vector_rows().expect("the committed joker table parses");
    let row: &JokerVectorRow = rows.first().expect("the table holds a first row");

    // Act
    let cards: Vec<Card> = row.cards().to_vec();

    // Assert
    assert_eq!(cards, ["AS", "AD", "AH", "AC", "JK"].map(card));
    assert_eq!(row.expected_class(), HandClass::FiveOfAKind);
    assert_eq!(row.expected_tiebreak(), [5, 14]);
    assert_eq!(row.expected_joker_ranks(), [Rank::Ace]);
    assert_eq!(row.expected_joker_codes(), Vec::<Card>::new());
}

#[test]
fn test_vectors_joker_last_row_holds_five_jokers() {
    // Arrange
    let rows = joker_vector_rows().expect("the committed joker table parses");
    let row: &JokerVectorRow = rows.last().expect("the table holds a last row");

    // Act
    let cards: Vec<Card> = row.cards().to_vec();

    // Assert
    assert_eq!(cards, ["JK", "JK", "JK", "JK", "JK"].map(card));
    assert_eq!(row.expected_class(), HandClass::FiveOfAKind);
    assert_eq!(row.expected_tiebreak(), [5, 14]);
    assert_eq!(row.expected_joker_ranks().len(), 5);
    assert_eq!(row.expected_joker_codes(), Vec::<Card>::new());
}

#[test]
fn test_vectors_data_rows_skips_the_header_and_blank_lines() {
    // Act
    let rows: Vec<(usize, &str)> = data_rows("header\n\n10C JC QC KC AC,1,ROYAL_FLUSH\n").collect();

    // Assert
    assert_eq!(rows, [(3, "10C JC QC KC AC,1,ROYAL_FLUSH")]);
}

#[test]
fn test_vectors_rank_sweep_row_rejects_a_missing_field() {
    // Act
    let result = parse_rank_sweep_row(2, "10C JC QC KC AC,1");

    // Assert
    assert_eq!(
        result,
        Err(VectorParseError::FieldCount { line: 2, count: 2 })
    );
}

#[test]
fn test_vectors_rank_sweep_row_rejects_a_short_card_list() {
    // Act
    let result = parse_rank_sweep_row(2, "10C JC QC KC,1,ROYAL_FLUSH");

    // Assert
    assert_eq!(
        result,
        Err(VectorParseError::CardCount { line: 2, count: 4 })
    );
}

#[test]
fn test_vectors_rank_sweep_row_rejects_a_bad_card_code() {
    // Act
    let result = parse_rank_sweep_row(2, "10C JC QC KC XX,1,ROYAL_FLUSH");

    // Assert
    assert_eq!(
        result,
        Err(VectorParseError::CardCode {
            line: 2,
            code: "XX".to_owned()
        })
    );
}

#[test]
fn test_vectors_rank_sweep_row_rejects_an_unknown_class() {
    // Act
    let result = parse_rank_sweep_row(2, "10C JC QC KC AC,1,NOT_A_CLASS");

    // Assert
    assert_eq!(
        result,
        Err(VectorParseError::HandClass {
            line: 2,
            name: "NOT_A_CLASS".to_owned()
        })
    );
}

#[test]
fn test_vectors_rank_sweep_row_rejects_a_bad_rank_number() {
    // Act
    let result = parse_rank_sweep_row(2, "10C JC QC KC AC,first,ROYAL_FLUSH");

    // Assert
    assert_eq!(
        result,
        Err(VectorParseError::Number {
            line: 2,
            text: "first".to_owned()
        })
    );
}

#[test]
fn test_vectors_joker_vector_row_accepts_empty_optional_fields() {
    // Act
    let result = parse_joker_vector_row(2, "AS AD AH AC JK,FIVE_OF_A_KIND,5;14,ACE,,five aces");

    // Assert
    assert_eq!(
        result.map(|row| (
            row.expected_joker_ranks().to_vec(),
            row.expected_joker_codes().to_vec(),
            row.description().to_owned()
        )),
        Ok((vec![Rank::Ace], Vec::new(), "five aces".to_owned()))
    );
}

#[test]
fn test_vectors_joker_vector_row_rejects_a_missing_description() {
    // Act
    let result = parse_joker_vector_row(2, "AS AD AH AC JK,FIVE_OF_A_KIND,5;14,ACE,");

    // Assert
    assert_eq!(
        result,
        Err(VectorParseError::FieldCount { line: 2, count: 5 })
    );
}

#[test]
fn test_vectors_joker_vector_row_rejects_a_bad_tiebreak_number() {
    // Act
    let result = parse_joker_vector_row(2, "AS AD AH AC JK,FIVE_OF_A_KIND,5;x,ACE,,d");

    // Assert
    assert_eq!(
        result,
        Err(VectorParseError::Number {
            line: 2,
            text: "x".to_owned()
        })
    );
}

#[test]
fn test_vectors_joker_vector_row_rejects_a_bad_joker_rank() {
    // Act
    let result = parse_joker_vector_row(2, "AS AD AH AC JK,FIVE_OF_A_KIND,5;14,ACEK,,d");

    // Assert
    assert_eq!(
        result,
        Err(VectorParseError::RankName {
            line: 2,
            name: "ACEK".to_owned()
        })
    );
}

#[test]
fn test_vectors_joker_vector_row_rejects_a_bad_joker_code() {
    // Act
    let result = parse_joker_vector_row(2, "AS AD AH AC JK,FIVE_OF_A_KIND,5;14,ACE,XX,d");

    // Assert
    assert_eq!(
        result,
        Err(VectorParseError::CardCode {
            line: 2,
            code: "XX".to_owned()
        })
    );
}

#[test]
fn test_vectors_joker_vector_row_rejects_an_unknown_class() {
    // Act
    let result = parse_joker_vector_row(2, "AS AD AH AC JK,NOPE,5;14,ACE,,d");

    // Assert
    assert_eq!(
        result,
        Err(VectorParseError::HandClass {
            line: 2,
            name: "NOPE".to_owned()
        })
    );
}

#[test]
fn test_vectors_joker_vector_row_rejects_a_short_card_list() {
    // Act
    let result = parse_joker_vector_row(2, "AS AD AH AC,FIVE_OF_A_KIND,5;14,ACE,,d");

    // Assert
    assert_eq!(
        result,
        Err(VectorParseError::CardCount { line: 2, count: 4 })
    );
}
