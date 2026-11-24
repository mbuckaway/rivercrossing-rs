// SPDX-License-Identifier: GPL-3.0-only

//! The two CSV exporters: the unified roster CSV and the §15 standings CSV.
//!
//! Both writers build the same Python `csv` `excel` dialect the import reads —
//! the canonical header, one row per rider, the formula guard on every cell and
//! CRLF terminators — and land the bytes through the atomic swap, so a failed
//! export never truncates a previous file (R-21, R-24, R-65).

use std::collections::BTreeMap;
use std::path::Path;

use rivercrossing_cards::Card;
use rivercrossing_roster::enums::{EntryType, PlateModel, RideStatus};
use rivercrossing_roster::roster::Roster;
use rivercrossing_roster::types::{Entry, Rider};
use rivercrossing_standings::hand_name::hand_name;
use rivercrossing_standings::types::{EntryResult, Placed};

use crate::error::CsvIoError;
use crate::format::python_float_repr;
use crate::wire::write_records;
use crate::writer::write_bytes_atomic;

/// The canonical roster columns, in export order (R-24).
const ROSTER_COLUMNS: [&str; 7] = [
    "FIRSTNAME",
    "LASTNAME",
    "TYPE",
    "TEAMNAME",
    "PLATE",
    "NOTES",
    "SEX",
];

/// The four finished-ride columns that a `placed` export appends (§7).
const STATS_COLUMNS: [&str; 4] = ["laps", "cards", "best_hand", "total_time"];

/// The §15 standings columns, in export order (R-65).
const STANDINGS_COLUMNS: [&str; 8] = [
    "place", "plate", "entry", "type", "sex", "laps", "hand", "draw",
];

/// The optional standings time column (R-63).
const TIME_COLUMN: &str = "total_time";

/// The laps cell of a DNS row.
const DNS: &str = "DNS";

/// The four finished-ride cells that every member row of one entry repeats.
struct EntryStats {
    /// The entry's completed laps.
    laps: String,
    /// How many cards the entry collected.
    cards: String,
    /// The entry's best hand as prose, or blank when it collected no cards.
    best_hand: String,
    /// The entry's total time in Python's float rendering.
    total_time: String,
}

/// Write *roster* as the unified roster CSV at *path*.
///
/// One row per rider, in entry then rider order. A rider's own plate renders
/// under `rider_pooled`, the entry's plate under `team_relay`, and an entry's
/// notes render on its first rider row only. A `placed` slice appends the
/// finished ride's `laps,cards,best_hand,total_time` columns.
///
/// # Errors
///
/// Return [`CsvIoError::NotFinished`] when *placed* is given but the ride is not
/// finished, [`CsvIoError::MissingStandings`] when an entry has no placed row,
/// [`CsvIoError::Standings`] when a non-empty card set has no hand name, and
/// [`CsvIoError::Io`] when the file cannot be written.
pub fn export(roster: &Roster, path: &Path, placed: Option<&[Placed]>) -> Result<(), CsvIoError> {
    let records = roster_records(roster, placed)?;
    let bytes = write_records(&records)?;
    write_bytes_atomic(path, &bytes)
}

/// Write *placed* as the §15 standings CSV at *path*.
///
/// One row per placed entry, in the order given: a DNS row carries a blank
/// place, `DNS` laps and no hand, draw or time, a DNF row is written as it
/// stands, and `show_times` appends the optional `total_time` column.
///
/// # Errors
///
/// Return [`CsvIoError::Standings`] when a non-empty card set has no hand name,
/// and [`CsvIoError::Io`] when the file cannot be written.
pub fn export_standings(
    placed: &[Placed],
    path: &Path,
    show_times: bool,
) -> Result<(), CsvIoError> {
    let mut records = vec![standings_header(show_times)];
    for row in placed {
        records.push(standings_row(row, show_times)?);
    }
    let bytes = write_records(&records)?;
    write_bytes_atomic(path, &bytes)
}

/// Build the roster CSV records, header first.
fn roster_records(
    roster: &Roster,
    placed: Option<&[Placed]>,
) -> Result<Vec<Vec<String>>, CsvIoError> {
    let index = match placed {
        None => None,
        Some(rows) => {
            require_finished(roster)?;
            Some(index_by_plate(rows))
        }
    };
    let mut records = vec![roster_header(index.is_some())];
    for entry in roster.entries() {
        let stats = entry_stats_for(entry, index.as_ref())?;
        records.extend(entry_rows(roster.plate_model(), entry, stats.as_ref()));
    }
    Ok(records)
}

/// Refuse the standings columns while the ride is not finished.
fn require_finished(roster: &Roster) -> Result<(), CsvIoError> {
    let status = roster.status();
    if status == RideStatus::Finished {
        return Ok(());
    }
    Err(CsvIoError::NotFinished {
        status: status.as_str().to_owned(),
    })
}

/// Index the placed rows by plate; the first row for a plate wins.
fn index_by_plate(rows: &[Placed]) -> BTreeMap<&str, &Placed> {
    let mut index = BTreeMap::new();
    for row in rows {
        index.entry(row.result.plate.as_str()).or_insert(row);
    }
    index
}

/// Return *entry*'s finished-ride cells, or refuse a missing placed row.
fn entry_stats_for(
    entry: &Entry,
    index: Option<&BTreeMap<&str, &Placed>>,
) -> Result<Option<EntryStats>, CsvIoError> {
    let Some(index) = index else {
        return Ok(None);
    };
    let Some(row) = index.get(entry.plate.as_str()).copied() else {
        return Err(CsvIoError::MissingStandings {
            plate: entry.plate.clone(),
        });
    };
    Ok(Some(entry_stats(&row.result)?))
}

/// Build one entry's finished-ride cells.
fn entry_stats(result: &EntryResult) -> Result<EntryStats, CsvIoError> {
    let best_hand = if result.cards.is_empty() {
        String::new()
    } else {
        hand_name(&result.hand)?
    };
    Ok(EntryStats {
        laps: result.laps.to_string(),
        cards: result.cards.len().to_string(),
        best_hand,
        total_time: python_float_repr(result.total_time),
    })
}

/// Build *entry*'s rows, appending *stats* to each when the ride is finished.
fn entry_rows(model: PlateModel, entry: &Entry, stats: Option<&EntryStats>) -> Vec<Vec<String>> {
    let mut rows = Vec::with_capacity(entry.riders.len());
    for (index, rider) in entry.riders.iter().enumerate() {
        let mut row = vec![
            rider.first_name.clone(),
            rider.last_name.clone(),
            entry.kind.as_str().to_owned(),
            team_cell(entry),
            plate_cell(model, entry, rider),
            notes_cell(entry, index),
            rider.sex.clone().unwrap_or_default(),
        ];
        if let Some(stats) = stats {
            row.push(stats.laps.clone());
            row.push(stats.cards.clone());
            row.push(stats.best_hand.clone());
            row.push(stats.total_time.clone());
        }
        rows.push(row);
    }
    rows
}

/// Build the roster header, with the finished-ride columns when asked.
fn roster_header(with_stats: bool) -> Vec<String> {
    let mut header: Vec<String> = ROSTER_COLUMNS
        .iter()
        .map(|column| (*column).to_owned())
        .collect();
    if with_stats {
        header.extend(STATS_COLUMNS.iter().map(|column| (*column).to_owned()));
    }
    header
}

/// Return the team name cell: the entry's name for a team, blank for a solo.
fn team_cell(entry: &Entry) -> String {
    match entry.kind {
        EntryType::Solo => String::new(),
        EntryType::Team => entry.display_name.clone(),
    }
}

/// Return the plate cell under the ride's plate model.
fn plate_cell(model: PlateModel, entry: &Entry, rider: &Rider) -> String {
    match model {
        PlateModel::RiderPooled => rider.plate.clone().unwrap_or_default(),
        PlateModel::TeamRelay => entry.plate.clone(),
    }
}

/// Return the notes cell: the entry's notes on its first row, blank after it.
fn notes_cell(entry: &Entry, rider_index: usize) -> String {
    if rider_index == 0 {
        return entry.notes.clone();
    }
    String::new()
}

/// Build the standings header, with the time column when asked.
fn standings_header(show_times: bool) -> Vec<String> {
    let mut header: Vec<String> = STANDINGS_COLUMNS
        .iter()
        .map(|column| (*column).to_owned())
        .collect();
    if show_times {
        header.push(TIME_COLUMN.to_owned());
    }
    header
}

/// Build one standings row.
fn standings_row(placed: &Placed, show_times: bool) -> Result<Vec<String>, CsvIoError> {
    let result = &placed.result;
    let mut row = vec![
        place_cell(placed),
        result.plate.clone(),
        result.name.clone(),
        result.kind.as_str().to_owned(),
        result.sex.clone().unwrap_or_default(),
        laps_cell(placed),
        hand_cell(result)?,
        draw_cell(placed),
    ];
    if show_times {
        row.push(total_time_cell(placed));
    }
    Ok(row)
}

/// Return the place cell: the number, or blank for a DNS row.
fn place_cell(placed: &Placed) -> String {
    match placed.place {
        Some(place) => place.to_string(),
        None => String::new(),
    }
}

/// Return the laps cell: the number, or the literal `DNS` for a DNS row.
fn laps_cell(placed: &Placed) -> String {
    if placed.is_dns() {
        return DNS.to_owned();
    }
    placed.result.laps.to_string()
}

/// Return the hand cell, or blank when the entry collected no cards.
fn hand_cell(result: &EntryResult) -> Result<String, CsvIoError> {
    if result.cards.is_empty() {
        return Ok(String::new());
    }
    Ok(hand_name(&result.hand)?)
}

/// Return the draw cell: the tie-break card's code, or blank.
fn draw_cell(placed: &Placed) -> String {
    if placed.is_dns() {
        return String::new();
    }
    placed
        .result
        .tiebreak_card
        .as_ref()
        .map_or_else(String::new, Card::code)
}

/// Return the total-time cell: the Python float rendering, or blank for a DNS
/// row.
fn total_time_cell(placed: &Placed) -> String {
    if placed.is_dns() {
        return String::new();
    }
    python_float_repr(placed.result.total_time)
}

#[cfg(test)]
mod tests;
