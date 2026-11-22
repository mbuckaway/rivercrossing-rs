// SPDX-License-Identifier: GPL-3.0-only

//! The golden ranking vectors that pin the evaluator's behaviour.
//!
//! The two CSV tables ship inside the crate, so a bundled application runs the
//! same vectors that the unit tests and the self-test read. A hand-rolled splitter
//! keeps the dependency list small.

use rivercrossing_cards::{Card, Rank};

use crate::class::HandClass;

/// The committed rank-sweep table, one row per distinct natural rank.
const RANK_SWEEP_CSV: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/fixtures/rank_sweep.csv"
));

/// The committed joker table, one row per wild hand under test.
const JOKER_VECTORS_CSV: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/fixtures/joker_vectors.csv"
));

/// The number of cards that a vector row spells out.
const VECTOR_CARD_COUNT: usize = 5;

/// Rank names as the joker table spells them.
const RANK_NAMES: [(&str, Rank); 13] = [
    ("TWO", Rank::Two),
    ("THREE", Rank::Three),
    ("FOUR", Rank::Four),
    ("FIVE", Rank::Five),
    ("SIX", Rank::Six),
    ("SEVEN", Rank::Seven),
    ("EIGHT", Rank::Eight),
    ("NINE", Rank::Nine),
    ("TEN", Rank::Ten),
    ("JACK", Rank::Jack),
    ("QUEEN", Rank::Queen),
    ("KING", Rank::King),
    ("ACE", Rank::Ace),
];

/// A vector table could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VectorParseError {
    /// A row holds fewer fields than its column needs.
    #[error("line {line} has {count} fields")]
    FieldCount {
        /// The one-based line number of the row.
        line: usize,
        /// The number of fields that the row holds.
        count: usize,
    },
    /// A row does not spell out five cards.
    #[error("line {line} holds {count} cards, expected 5")]
    CardCount {
        /// The one-based line number of the row.
        line: usize,
        /// The number of cards that the row holds.
        count: usize,
    },
    /// A text is not a card code.
    #[error("line {line}: {code} is not a card code")]
    CardCode {
        /// The one-based line number of the row.
        line: usize,
        /// The text that failed to parse.
        code: String,
    },
    /// A text is not a hand-class name.
    #[error("line {line}: {name} is not a hand class")]
    HandClass {
        /// The one-based line number of the row.
        line: usize,
        /// The text that failed to parse.
        name: String,
    },
    /// A text is not a rank name.
    #[error("line {line}: {name} is not a rank name")]
    RankName {
        /// The one-based line number of the row.
        line: usize,
        /// The text that failed to parse.
        name: String,
    },
    /// A text is not a number.
    #[error("line {line}: {text} is not a number")]
    Number {
        /// The one-based line number of the row.
        line: usize,
        /// The text that failed to parse.
        text: String,
    },
}

/// One row of the rank-sweep table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankSweepRow {
    cards: Vec<Card>,
    rank: usize,
    class: HandClass,
}

impl RankSweepRow {
    /// Return the five cards that the row spells out.
    #[must_use]
    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    /// Return the recorded natural rank, one for the best and 7462 for the worst.
    #[must_use]
    pub fn rank(&self) -> usize {
        self.rank
    }

    /// Return the recorded hand class.
    #[must_use]
    pub fn class(&self) -> HandClass {
        self.class
    }
}

/// One row of the joker table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JokerVectorRow {
    cards: Vec<Card>,
    expected_class: HandClass,
    expected_tiebreak: Vec<u8>,
    expected_joker_ranks: Vec<Rank>,
    expected_joker_codes: Vec<Card>,
    description: String,
}

impl JokerVectorRow {
    /// Return the five cards that the row spells out.
    #[must_use]
    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    /// Return the hand class that the row expects.
    #[must_use]
    pub fn expected_class(&self) -> HandClass {
        self.expected_class
    }

    /// Return the tiebreak that the row expects.
    #[must_use]
    pub fn expected_tiebreak(&self) -> &[u8] {
        &self.expected_tiebreak
    }

    /// Return the rank of each joker's resolution that the row expects.
    #[must_use]
    pub fn expected_joker_ranks(&self) -> &[Rank] {
        &self.expected_joker_ranks
    }

    /// Return the card of each joker's resolution that the row expects, or nothing.
    #[must_use]
    pub fn expected_joker_codes(&self) -> &[Card] {
        &self.expected_joker_codes
    }

    /// Return the text that describes the row.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }
}

/// Build a rank-sweep row from its parts, so a unit test can corrupt the table.
#[cfg(test)]
impl RankSweepRow {
    pub(crate) fn from_parts(cards: Vec<Card>, rank: usize, class: HandClass) -> Self {
        Self { cards, rank, class }
    }
}

/// Build a joker row from its parts, so a unit test can corrupt the table.
#[cfg(test)]
impl JokerVectorRow {
    pub(crate) fn from_parts(
        cards: Vec<Card>,
        expected_class: HandClass,
        expected_tiebreak: Vec<u8>,
    ) -> Self {
        Self {
            cards,
            expected_class,
            expected_tiebreak,
            expected_joker_ranks: Vec::new(),
            expected_joker_codes: Vec::new(),
            description: "test row".to_owned(),
        }
    }
}

/// Return every row of the committed rank-sweep table.
///
/// # Errors
///
/// Return [`VectorParseError`] when a committed row does not parse.
pub fn rank_sweep_rows() -> Result<Vec<RankSweepRow>, VectorParseError> {
    data_rows(RANK_SWEEP_CSV)
        .map(|(line, text)| parse_rank_sweep_row(line, text))
        .collect()
}

/// Return every row of the committed joker table.
///
/// # Errors
///
/// Return [`VectorParseError`] when a committed row does not parse.
pub fn joker_vector_rows() -> Result<Vec<JokerVectorRow>, VectorParseError> {
    data_rows(JOKER_VECTORS_CSV)
        .map(|(line, text)| parse_joker_vector_row(line, text))
        .collect()
}

/// Return each data row of a CSV with its one-based line number.
fn data_rows(csv: &str) -> impl Iterator<Item = (usize, &str)> {
    csv.lines()
        .enumerate()
        .skip(1)
        .filter(|(_, line)| !line.is_empty())
        .map(|(index, line)| (index + 1, line))
}

/// The comma-separated fields of one row.
struct Fields<'a> {
    line: usize,
    values: Vec<&'a str>,
}

impl<'a> Fields<'a> {
    /// Split one row into its fields.
    fn new(line: usize, text: &'a str) -> Self {
        Self {
            line,
            values: text.split(',').collect(),
        }
    }

    /// Return the first field, or the field-count error for an empty row.
    fn first(&self) -> Result<&'a str, VectorParseError> {
        self.get(0)
    }

    /// Return the field at `index`, or a field-count error when it is absent.
    fn get(&self, index: usize) -> Result<&'a str, VectorParseError> {
        self.values
            .get(index)
            .copied()
            .ok_or(VectorParseError::FieldCount {
                line: self.line,
                count: self.values.len(),
            })
    }
}

/// Parse one rank-sweep row.
fn parse_rank_sweep_row(line: usize, text: &str) -> Result<RankSweepRow, VectorParseError> {
    let fields = Fields::new(line, text);
    let cards = parse_cards(&fields)?;
    let rank = parse_number(fields.get(1)?, line)?;
    let class = parse_class(fields.get(2)?, line)?;
    Ok(RankSweepRow { cards, rank, class })
}

/// Parse one joker row.
fn parse_joker_vector_row(line: usize, text: &str) -> Result<JokerVectorRow, VectorParseError> {
    let fields = Fields::new(line, text);
    let cards = parse_cards(&fields)?;
    let expected_class = parse_class(fields.get(1)?, line)?;
    let expected_tiebreak = parse_tiebreak(&fields)?;
    let expected_joker_ranks = parse_ranks(fields.get(3)?, line)?;
    let expected_joker_codes = parse_codes(fields.get(4)?, line)?;
    let description = fields.get(5)?.to_owned();
    Ok(JokerVectorRow {
        cards,
        expected_class,
        expected_tiebreak,
        expected_joker_ranks,
        expected_joker_codes,
        description,
    })
}

/// Parse the space-separated card codes of the first field.
fn parse_cards(fields: &Fields<'_>) -> Result<Vec<Card>, VectorParseError> {
    let text = fields.first()?;
    let mut cards = Vec::with_capacity(VECTOR_CARD_COUNT);
    for code in text.split_whitespace() {
        cards.push(parse_card(fields.line, code)?);
    }
    if cards.len() != VECTOR_CARD_COUNT {
        return Err(VectorParseError::CardCount {
            line: fields.line,
            count: cards.len(),
        });
    }
    Ok(cards)
}

/// Parse one card code.
fn parse_card(line: usize, code: &str) -> Result<Card, VectorParseError> {
    Card::parse(code).map_err(|_| VectorParseError::CardCode {
        line,
        code: code.to_owned(),
    })
}

/// Parse one hand-class name.
fn parse_class(text: &str, line: usize) -> Result<HandClass, VectorParseError> {
    HandClass::from_name(text).ok_or_else(|| VectorParseError::HandClass {
        line,
        name: text.to_owned(),
    })
}

/// Parse the semicolon-separated tiebreak of a joker row.
fn parse_tiebreak(fields: &Fields<'_>) -> Result<Vec<u8>, VectorParseError> {
    fields
        .get(2)?
        .split(';')
        .map(|part| parse_byte(fields.line, part))
        .collect()
}

/// Parse the semicolon-separated joker rank names of a joker row.
fn parse_ranks(text: &str, line: usize) -> Result<Vec<Rank>, VectorParseError> {
    if text.is_empty() {
        return Ok(Vec::new());
    }
    text.split(';')
        .map(|name| parse_rank_name(name, line))
        .collect()
}

/// Parse the semicolon-separated joker card codes of a joker row.
fn parse_codes(text: &str, line: usize) -> Result<Vec<Card>, VectorParseError> {
    if text.is_empty() {
        return Ok(Vec::new());
    }
    text.split(';').map(|code| parse_card(line, code)).collect()
}

/// Parse one rank name.
fn parse_rank_name(name: &str, line: usize) -> Result<Rank, VectorParseError> {
    RANK_NAMES
        .iter()
        .find(|(text, _)| *text == name)
        .map(|(_, rank)| *rank)
        .ok_or_else(|| VectorParseError::RankName {
            line,
            name: name.to_owned(),
        })
}

/// Parse one one-byte number.
fn parse_byte(line: usize, text: &str) -> Result<u8, VectorParseError> {
    text.parse::<u8>().map_err(|_| VectorParseError::Number {
        line,
        text: text.to_owned(),
    })
}

/// Parse one number.
fn parse_number(text: &str, line: usize) -> Result<usize, VectorParseError> {
    text.parse::<usize>().map_err(|_| VectorParseError::Number {
        line,
        text: text.to_owned(),
    })
}

#[cfg(test)]
mod tests;
