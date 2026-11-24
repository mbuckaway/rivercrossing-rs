// SPDX-License-Identifier: GPL-3.0-only

//! The page's per-kind section plan and the poster's `(heading, rows)` plan.
//!
//! [`sections`] splits a [`RacePayload`] into the podiums, top lists, laps
//! boards and full-field lists that the results page renders, then recomputes
//! the per-kind laps boards from the placed standings so one kind can never
//! crowd the other out of a combined board. [`poster_sections`] plans the
//! poster's cards. Both follow the Python oracle's `sections`,
//! `_sections_from_payload` and `_poster_sections`.

use std::collections::BTreeSet;

use rivercrossing_standings::{Placed, SnapshotKind};

use crate::payload::{LapsBoardSpec, TOP_SOLO_ONLY_ROWS, TOP_SOLO_ROWS, TOP_TEAM_ROWS, laps_rows};
use crate::types::{ExportOptions, LapsBoardRow, RacePayload, ResultRow, Sections};

/// The podium cards' row count.
const PODIUM_ROWS: usize = 3;

/// The poster's per-kind section length on a team event.
const POSTER_PODIUM_ROWS: usize = 3;

/// The poster's section length on a solo-only field.
const POSTER_SOLO_ONLY_ROWS: usize = 5;

/// Return whether a kind spelling names a team; display suffixes are allowed.
fn is_team_kind(kind: &str) -> bool {
    kind.to_uppercase().starts_with("TEAM")
}

/// Plan the page's sections from *payload* and *placed*.
///
/// Any team row makes the field a team event, so both kinds get a three-row
/// podium and a five-row top list; a solo-only field keeps one top ten. With
/// the laps board on, the per-kind boards are recomputed from *placed* at five
/// rows each (ten on a solo-only field); with it off, the payload's own plan
/// passes through unchanged. The time board always passes through flat.
#[must_use]
pub fn sections(payload: &RacePayload, placed: &[Placed]) -> Sections {
    let plan = sections_from_payload(payload);
    if !payload.options.laps_board {
        return plan;
    }
    let Some((laps_teams, laps_solo)) = recomputed_laps(&plan, placed, &payload.options) else {
        return plan;
    };
    Sections {
        laps_teams,
        laps_solo,
        ..plan
    }
}

/// Recompute both per-kind laps boards from *placed*.
///
/// Return `None` when a plate cannot be an integer; the caller then keeps the
/// payload's own plan, because `build_payload` has already refused that plate
/// in the normal flow.
fn recomputed_laps(
    plan: &Sections,
    placed: &[Placed],
    options: &ExportOptions,
) -> Option<(Vec<LapsBoardRow>, Vec<LapsBoardRow>)> {
    if plan.teams.is_empty() {
        let laps_solo = laps_rows(
            placed,
            SnapshotKind::Solo,
            LapsBoardSpec::new(TOP_SOLO_ONLY_ROWS, options),
        )
        .ok()?;
        return Some((Vec::new(), laps_solo));
    }
    let laps_teams = laps_rows(
        placed,
        SnapshotKind::Team,
        LapsBoardSpec::new(TOP_TEAM_ROWS, options),
    )
    .ok()?;
    let laps_solo = laps_rows(
        placed,
        SnapshotKind::Solo,
        LapsBoardSpec::new(TOP_SOLO_ROWS, options),
    )
    .ok()?;
    Some((laps_teams, laps_solo))
}

/// Plan the sections that only the payload's own rows decide.
pub(crate) fn sections_from_payload(payload: &RacePayload) -> Sections {
    let teams: Vec<ResultRow> = payload
        .results
        .iter()
        .filter(|row| is_team_kind(&row.entry_type))
        .cloned()
        .collect();
    let solo: Vec<ResultRow> = payload
        .results
        .iter()
        .filter(|row| !is_team_kind(&row.entry_type))
        .cloned()
        .collect();
    let ranked_teams: Vec<&ResultRow> = teams.iter().filter(|row| !row.dns).collect();
    let ranked_solo: Vec<&ResultRow> = solo.iter().filter(|row| !row.dns).collect();
    let top_solo = if teams.is_empty() {
        TOP_SOLO_ONLY_ROWS
    } else {
        TOP_SOLO_ROWS
    };
    let team_plates: BTreeSet<i64> = teams.iter().map(|row| row.plate).collect();
    Sections {
        podium_teams: take_rows(&ranked_teams, PODIUM_ROWS),
        podium_solo: take_rows(&ranked_solo, PODIUM_ROWS),
        top_teams: take_rows(&ranked_teams, TOP_TEAM_ROWS),
        top_solo: take_rows(&ranked_solo, top_solo),
        laps_teams: payload
            .laps_board
            .iter()
            .filter(|row| is_team_kind(&row.entry_type))
            .take(TOP_TEAM_ROWS)
            .cloned()
            .collect(),
        laps_solo: payload
            .laps_board
            .iter()
            .filter(|row| !is_team_kind(&row.entry_type))
            .take(top_solo)
            .cloned()
            .collect(),
        time_board: payload.time_board.clone(),
        teams,
        solo,
        team_plates,
    }
}

/// Plan the poster's `(heading, rows)` sections from *payload*.
///
/// Any team row makes it a team event: a titled section of the top three of
/// each kind, each yielded even when its rows are empty. A solo-only field gets
/// one untitled section of its top five. Rows come from the ranked list only,
/// so a DID-NOT-START row never reaches a card.
#[must_use]
pub fn poster_sections(payload: &RacePayload) -> Vec<(String, Vec<ResultRow>)> {
    let teams: Vec<&ResultRow> = payload
        .results
        .iter()
        .filter(|row| is_team_kind(&row.entry_type))
        .collect();
    let ranked_teams: Vec<&ResultRow> = teams.iter().copied().filter(|row| !row.dns).collect();
    let ranked_solo: Vec<&ResultRow> = payload
        .results
        .iter()
        .filter(|row| !is_team_kind(&row.entry_type) && !row.dns)
        .collect();
    if teams.is_empty() {
        return vec![(
            String::new(),
            take_rows(&ranked_solo, POSTER_SOLO_ONLY_ROWS),
        )];
    }
    vec![
        (
            "Teams".to_owned(),
            take_rows(&ranked_teams, POSTER_PODIUM_ROWS),
        ),
        (
            "Solo riders".to_owned(),
            take_rows(&ranked_solo, POSTER_PODIUM_ROWS),
        ),
    ]
}

/// Return the first *count* rows of *rows*, cloned.
fn take_rows(rows: &[&ResultRow], count: usize) -> Vec<ResultRow> {
    rows.iter().take(count).map(|row| (*row).clone()).collect()
}

#[cfg(test)]
mod tests;
