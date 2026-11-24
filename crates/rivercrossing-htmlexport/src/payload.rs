// SPDX-License-Identifier: GPL-3.0-only

//! The shared results payload builder.
//!
//! [`build_payload`] maps a caller's [`ExportInput`] to the [`RacePayload`] that
//! both exporters render: one result row per placed entry, the per-kind
//! most-laps board, and the flat fastest-laps board that the publish settings
//! request. The row and board rules are the Python oracle's (`build_payload`,
//! `_result_row_from_placed`, `_boards_from_placed`, `_laps_record_board`).

use rivercrossing_standings::boards::{laps_leaderboard, time_leaderboard};
use rivercrossing_standings::hand_name::hand_name;
use rivercrossing_standings::{DEFAULT_TOP, EntryResult, Placed, SnapshotKind};

use crate::error::ExportError;
use crate::format::{format_duration, format_meta};
use crate::types::{
    CardPair, EventInfo, ExportInput, ExportOptions, KICKER, LapsBoardRow, RacePayload, ResultRow,
    SELF_TEST_NOTE, TimeBoardRow,
};

/// The mixed field's per-kind cap on the most-laps board.
pub(crate) const TOP_TEAM_ROWS: usize = 5;

/// The mixed field's solo cap on the most-laps board.
pub(crate) const TOP_SOLO_ROWS: usize = 5;

/// The solo-only field's cap on the most-laps board.
pub(crate) const TOP_SOLO_ONLY_ROWS: usize = 10;

/// The em dash that a zero-lap entry's average lap reads as.
pub(crate) const NO_AVERAGE: &str = "—";

/// Build the shared results payload from *input*.
///
/// # Errors
///
/// Return [`ExportError::PlateNotNumeric`] when a plate cannot be an integer,
/// and [`ExportError::HandName`] when a non-blank hand cannot be named.
pub fn build_payload(input: &ExportInput) -> Result<RacePayload, ExportError> {
    let results = input
        .placed
        .iter()
        .map(|placed| result_row(placed, input.team_logos.get(&placed.result.plate).cloned()))
        .collect::<Result<Vec<ResultRow>, ExportError>>()?;
    let (laps_board, time_board) = boards_from_placed(&input.placed, &input.options)?;
    Ok(RacePayload {
        event: event_info(input),
        options: input.options,
        tie_note: input.tie_note.clone(),
        results,
        laps_board,
        time_board,
        self_test_note: input
            .self_test_unverified
            .then(|| SELF_TEST_NOTE.to_owned()),
    })
}

/// Compose the record's `event` block from *input*.
fn event_info(input: &ExportInput) -> EventInfo {
    let organizer = &input.ride.organizer;
    let scorer = &input.ride.scorer;
    EventInfo {
        kicker: KICKER.to_owned(),
        title: input.ride.name.clone(),
        meta: format_meta(&input.ride),
        organizer: format!("Organizer: {organizer}"),
        scorer: format!("Scorer: {scorer}"),
        generated: input.generated.clone(),
        entries: input.placed.len(),
        laps: input.placed.iter().map(|placed| placed.result.laps).sum(),
        cards: input
            .placed
            .iter()
            .map(|placed| placed.result.cards.len())
            .sum(),
        riders: input.riders,
    }
}

/// Map one placed entry to the record's results row.
fn result_row(placed: &Placed, logo: Option<String>) -> Result<ResultRow, ExportError> {
    let result = &placed.result;
    let dns = placed.is_dns();
    Ok(ResultRow {
        place: placed.place.map_or(0, i64::from),
        plate: parse_plate(&result.plate)?,
        entry: result.name.clone(),
        entry_type: entry_type(result.kind),
        laps: result.laps,
        hand: hand_label(result)?,
        total: (!dns).then(|| format_duration(result.total_time)),
        best_lap: (!dns).then(|| format_duration(result.best_lap)),
        sex: result.sex.clone(),
        tie: placed.draw_required,
        dnf: result.dnf,
        dns,
        cards: result
            .hand
            .best5()
            .iter()
            .map(CardPair::from_card)
            .collect(),
        drawn: result.cards.iter().map(CardPair::from_card).collect(),
        draw: if dns {
            None
        } else {
            result.tiebreak_card.as_ref().map(CardPair::from_card)
        },
        logo,
    })
}

/// Return *result*'s prose hand name, or the empty string for a no-card hand.
fn hand_label(result: &EntryResult) -> Result<String, ExportError> {
    if result.hand.best5().is_empty() {
        return Ok(String::new());
    }
    Ok(hand_name(&result.hand)?)
}

/// Return the record's uppercase spelling of *kind*.
fn entry_type(kind: SnapshotKind) -> String {
    kind.as_str().to_uppercase()
}

/// Return *plate* as the record's integer, after trimming surrounding space.
///
/// # Errors
///
/// Return [`ExportError::PlateNotNumeric`] when *plate* is not an integer.
pub(crate) fn parse_plate(plate: &str) -> Result<i64, ExportError> {
    plate
        .trim()
        .parse::<i64>()
        .map_err(|_| ExportError::PlateNotNumeric {
            plate: plate.to_owned(),
        })
}

/// Build the boards that *options* request from *placed*.
fn boards_from_placed(
    placed: &[Placed],
    options: &ExportOptions,
) -> Result<(Vec<LapsBoardRow>, Vec<TimeBoardRow>), ExportError> {
    let laps_board = if options.laps_board {
        laps_record_board(placed, options)?
    } else {
        Vec::new()
    };
    let time_board = if options.time_board && options.show_times {
        time_board_rows(placed)?
    } else {
        Vec::new()
    };
    Ok((laps_board, time_board))
}

/// The cap and time setting that one kind's most-laps board is built with.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LapsBoardSpec {
    /// The board's row cap.
    pub top: usize,
    /// Whether each row carries its total time.
    pub show_times: bool,
}

impl LapsBoardSpec {
    /// Read one board's spec from its *top* and the publish *options*.
    pub(crate) const fn new(top: usize, options: &ExportOptions) -> Self {
        Self {
            top,
            show_times: options.show_times,
        }
    }
}

/// Build the record's per-kind most-laps board: teams first, then solo.
fn laps_record_board(
    placed: &[Placed],
    options: &ExportOptions,
) -> Result<Vec<LapsBoardRow>, ExportError> {
    let mixed = placed
        .iter()
        .any(|row| row.result.kind == SnapshotKind::Team);
    let mut rows = Vec::new();
    if mixed {
        rows.extend(laps_rows(
            placed,
            SnapshotKind::Team,
            LapsBoardSpec::new(TOP_TEAM_ROWS, options),
        )?);
    }
    let solo_top = if mixed {
        TOP_SOLO_ROWS
    } else {
        TOP_SOLO_ONLY_ROWS
    };
    rows.extend(laps_rows(
        placed,
        SnapshotKind::Solo,
        LapsBoardSpec::new(solo_top, options),
    )?);
    Ok(rows)
}

/// Build one kind's most-laps rows from the started entries of *placed*.
///
/// The team/solo test is the oracle's exact kind match, never the
/// uppercase-prefix rule that the section split uses.
pub(crate) fn laps_rows(
    placed: &[Placed],
    kind: SnapshotKind,
    spec: LapsBoardSpec,
) -> Result<Vec<LapsBoardRow>, ExportError> {
    let entries: Vec<EntryResult> = placed
        .iter()
        .filter(|row| row.result.kind == kind && !row.is_dns())
        .map(|row| row.result.clone())
        .collect();
    laps_leaderboard(&entries, spec.top)
        .iter()
        .map(|row| laps_board_row(&row.result, spec.show_times))
        .collect()
}

/// Map one ranked entry to a most-laps row.
fn laps_board_row(result: &EntryResult, show_times: bool) -> Result<LapsBoardRow, ExportError> {
    Ok(LapsBoardRow {
        plate: parse_plate(&result.plate)?,
        entry: result.name.clone(),
        entry_type: entry_type(result.kind),
        laps: result.laps,
        total: show_times.then(|| format_duration(result.total_time)),
    })
}

/// Build the flat fastest-laps board from the started entries of *placed*.
fn time_board_rows(placed: &[Placed]) -> Result<Vec<TimeBoardRow>, ExportError> {
    let active = active_results(placed);
    time_leaderboard(&active, DEFAULT_TOP)
        .iter()
        .map(|row| time_board_row(&row.result))
        .collect()
}

/// Map one ranked entry to a fastest-laps row.
fn time_board_row(result: &EntryResult) -> Result<TimeBoardRow, ExportError> {
    Ok(TimeBoardRow {
        plate: parse_plate(&result.plate)?,
        entry: result.name.clone(),
        laps: result.laps,
        total: format_duration(result.total_time),
        avg: average_lap(result),
    })
}

/// Return *result*'s average lap, or the em dash with no lap to average.
fn average_lap(result: &EntryResult) -> String {
    if result.laps == 0 {
        return NO_AVERAGE.to_owned();
    }
    format_duration(result.total_time / f64::from(result.laps))
}

/// Return the readings of the entries that started, in *placed* order.
fn active_results(placed: &[Placed]) -> Vec<EntryResult> {
    placed
        .iter()
        .filter(|row| !row.is_dns())
        .map(|row| row.result.clone())
        .collect()
}

#[cfg(test)]
mod tests;
