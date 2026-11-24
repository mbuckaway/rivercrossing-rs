// SPDX-License-Identifier: GPL-3.0-only

//! The exporter's domain model: the ride metadata, the publish options, the
//! record rows and the section split that the two `WordPress` fragments share.
//!
//! The types carry no serde derives. The template context is `snake_case` and
//! the `race-data` record is `camelCase` and sparse, so one struct family cannot
//! be both; [`crate::record`] owns the record shape.

use std::collections::{BTreeMap, BTreeSet};

use rivercrossing_cards::{Card, Rank, Suit};
use rivercrossing_standings::Placed;
use time::Date;

/// The page kicker: the samples' own "· poker run" line.
pub const KICKER: &str = "Official results · poker run";

/// The caption the page and the PDF render when the ride was finished over a
/// failed evaluator self-test.
pub const SELF_TEST_NOTE: &str =
    "Self-test unverified — these results were published over a failed evaluator self-test";

/// The 1×1 transparent PNG data URI that stands in for an absent ride logo.
pub const TRANSPARENT_PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

/// The oracle's default closed-loop length in kilometres.
pub(crate) const DEFAULT_LAP_KM: f64 = 8.0;

/// One card as the record's `(rank, suit)` pair.
///
/// The rank is the record token (`"10"`, never `"T"`) and the suit is the
/// lowercase letter (`"c"`, `"d"`, `"h"`, `"s"`); a joker reads `("JK", "j")`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardPair {
    /// The rank token, such as `"10"`, `"Q"` or `"JK"`.
    pub rank: String,
    /// The suit letter, such as `"s"`, or `"j"` for a joker.
    pub suit: String,
}

impl CardPair {
    /// Return the record pair for *card*.
    #[must_use]
    pub fn from_card(card: &Card) -> Self {
        match card {
            Card::Joker => Self {
                rank: "JK".to_owned(),
                suit: "j".to_owned(),
            },
            Card::Natural { rank, suit } => Self {
                rank: rank_letter(*rank).to_owned(),
                suit: suit_letter(*suit).to_owned(),
            },
        }
    }
}

/// Return the record token of *rank*.
fn rank_letter(rank: Rank) -> &'static str {
    match rank {
        Rank::Two => "2",
        Rank::Three => "3",
        Rank::Four => "4",
        Rank::Five => "5",
        Rank::Six => "6",
        Rank::Seven => "7",
        Rank::Eight => "8",
        Rank::Nine => "9",
        Rank::Ten => "10",
        Rank::Jack => "J",
        Rank::Queen => "Q",
        Rank::King => "K",
        Rank::Ace => "A",
    }
}

/// Return the record letter of *suit*.
fn suit_letter(suit: Suit) -> &'static str {
    match suit {
        Suit::Clubs => "c",
        Suit::Diamonds => "d",
        Suit::Hearts => "h",
        Suit::Spades => "s",
    }
}

/// The ride facts that the page header and the meta line read.
#[derive(Debug, Clone)]
pub struct RideInfo {
    /// The ride's name.
    pub name: String,
    /// The ride's event date.
    pub event_date: Date,
    /// The venue.
    pub venue: String,
    /// The closed-loop length in kilometres.
    pub lap_km: f64,
    /// The organizer line.
    pub organizer: String,
    /// The scorer line.
    pub scorer: String,
}

/// The five independent publish settings, plus the render-only loop length.
#[allow(
    clippy::struct_excessive_bools,
    reason = "mirrors the oracle's five independent publish settings; a bitflags type would obscure them"
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExportOptions {
    /// Whether times are published at all.
    pub show_times: bool,
    /// Whether the most-laps board is shown.
    pub laps_board: bool,
    /// Whether the fastest-laps board is shown.
    pub time_board: bool,
    /// Whether the full field is shown, not only the podium.
    pub full_field: bool,
    /// Whether every drawn card is shown.
    pub all_cards: bool,
    /// The closed-loop length that captions the laps board.
    pub lap_km: f64,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            show_times: false,
            laps_board: true,
            time_board: false,
            full_field: true,
            all_cards: true,
            lap_km: DEFAULT_LAP_KM,
        }
    }
}

/// Everything the exporter needs to build one ride's payload.
///
/// The caller composes the ranked rows, the publish settings, the pinned
/// footer stamp and the logo seams; [`crate::payload::build_payload`] maps them
/// to the shared [`RacePayload`].
#[derive(Debug, Clone)]
pub struct ExportInput {
    /// The ride facts that the header and meta line read.
    pub ride: RideInfo,
    /// The ranked rows, DID-NOT-START rows included at the end.
    pub placed: Vec<Placed>,
    /// The publish settings.
    pub options: ExportOptions,
    /// The pinned footer stamp.
    pub generated: String,
    /// The ride logo as a data URI; `None` falls back to the transparent PNG.
    pub logo_src: Option<String>,
    /// The per-entry logos, keyed by the plate spelling the row carries.
    pub team_logos: BTreeMap<String, String>,
    /// The header's unique-rider count.
    pub riders: usize,
    /// Whether the ride was published over a failed evaluator self-test.
    pub self_test_unverified: bool,
    /// The caller-composed residual-tie note.
    pub tie_note: Option<String>,
}

/// The event facts that the record's `event` block carries.
#[derive(Debug, Clone)]
pub struct EventInfo {
    /// The page kicker.
    pub kicker: String,
    /// The event title.
    pub title: String,
    /// The meta line: date, venue and loop length.
    pub meta: String,
    /// The organizer line.
    pub organizer: String,
    /// The scorer line.
    pub scorer: String,
    /// The footer's generated stamp.
    pub generated: String,
    /// The number of entries.
    pub entries: usize,
    /// The number of recorded laps.
    pub laps: u32,
    /// The number of dealt cards.
    pub cards: usize,
    /// The render-only rider counter; the record never carries it.
    pub riders: usize,
}

/// One entry's placed result: one row of the record's `results`.
#[derive(Debug, Clone)]
pub struct ResultRow {
    /// The place; `0` for a did-not-start row, which the page blanks.
    pub place: i64,
    /// The numeric plate.
    pub plate: i64,
    /// The entry name.
    pub entry: String,
    /// The kind spelling, such as `"SOLO"` or `"TEAM ×4"`.
    pub entry_type: String,
    /// The recorded laps.
    pub laps: u32,
    /// The hand name.
    pub hand: String,
    /// The total time reading, when times are published.
    pub total: Option<String>,
    /// The best-lap reading, when times are published.
    pub best_lap: Option<String>,
    /// A solo rider's sex marker.
    pub sex: Option<String>,
    /// Whether a configured order could not separate the row.
    pub tie: bool,
    /// Whether the entry did not finish.
    pub dnf: bool,
    /// Whether the entry did not start.
    pub dns: bool,
    /// The displayed best-five cards.
    pub cards: Vec<CardPair>,
    /// The full draw order.
    pub drawn: Vec<CardPair>,
    /// The entry's drawn tie-break card.
    pub draw: Option<CardPair>,
    /// The entry's logo card as a data URI.
    pub logo: Option<String>,
}

/// One row of the most-laps board.
#[derive(Debug, Clone)]
pub struct LapsBoardRow {
    /// The numeric plate.
    pub plate: i64,
    /// The entry name.
    pub entry: String,
    /// The kind spelling.
    pub entry_type: String,
    /// The recorded laps.
    pub laps: u32,
    /// The total time reading, when times are published.
    pub total: Option<String>,
}

/// One row of the fastest-laps board.
#[derive(Debug, Clone)]
pub struct TimeBoardRow {
    /// The numeric plate.
    pub plate: i64,
    /// The entry name.
    pub entry: String,
    /// The recorded laps.
    pub laps: u32,
    /// The total time reading.
    pub total: String,
    /// The average lap reading.
    pub avg: String,
}

/// The full results record embedded in `<script id="race-data">`.
#[derive(Debug, Clone)]
pub struct RacePayload {
    /// The event block.
    pub event: EventInfo,
    /// The publish settings.
    pub options: ExportOptions,
    /// The residual-tie note; the record always carries the key.
    pub tie_note: Option<String>,
    /// The placed result rows.
    pub results: Vec<ResultRow>,
    /// The most-laps board.
    pub laps_board: Vec<LapsBoardRow>,
    /// The fastest-laps board.
    pub time_board: Vec<TimeBoardRow>,
    /// The self-test caption, when the ride was published over a failed test.
    pub self_test_note: Option<String>,
}

/// The per-kind section split that the two `WordPress` fragments render.
#[derive(Debug, Clone)]
pub struct Sections {
    /// The first three ranked teams.
    pub podium_teams: Vec<ResultRow>,
    /// The first three ranked solo riders.
    pub podium_solo: Vec<ResultRow>,
    /// Up to five ranked teams.
    pub top_teams: Vec<ResultRow>,
    /// Up to five, or ten, ranked solo riders.
    pub top_solo: Vec<ResultRow>,
    /// The team rows of the most-laps board.
    pub laps_teams: Vec<LapsBoardRow>,
    /// The solo rows of the most-laps board.
    pub laps_solo: Vec<LapsBoardRow>,
    /// The fastest-laps board.
    pub time_board: Vec<TimeBoardRow>,
    /// Every team row.
    pub teams: Vec<ResultRow>,
    /// Every solo row.
    pub solo: Vec<ResultRow>,
    /// The plates of every team row.
    pub team_plates: BTreeSet<i64>,
}

#[cfg(test)]
mod tests;
