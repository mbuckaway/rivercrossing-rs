// SPDX-License-Identifier: GPL-3.0-only

//! The cell-level rules of the roster CSV: whitespace, the team merge key, the
//! rider-name case, the sex and entry-kind folding, and the notes join.
//!
//! The preview applies these rules to every cell it reads (R-21, R-24), so the
//! commit writes the same values that the operator previewed.

use rivercrossing_roster::enums::EntryType;

/// The token that a male sex cell folds to.
const MALE: &str = "M";

/// The token that a female sex cell folds to.
const FEMALE: &str = "F";

/// The separator between two joined notes.
const NOTE_SEPARATOR: &str = "; ";

/// The conflict message for a team row that names no team.
const MISSING_TEAM_NAME: &str = "missing name";

/// Return *raw* trimmed with each internal whitespace run collapsed.
#[must_use]
pub(crate) fn collapse_whitespace(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Return the case-folded team merge key (R-24).
///
/// The key trims, collapses internal whitespace and lowercases, so `"Full
/// Send"` and `"full   send"` share one key. The key keeps the space that the
/// file spells, so `BNBA1` and `BNBA 1` stay two teams.
#[must_use]
pub(crate) fn normalize_team_name(raw: &str) -> String {
    collapse_whitespace(raw).to_lowercase()
}

/// Return the team name as the file spells it, whitespace-collapsed.
#[must_use]
pub(crate) fn display_team_name(raw: &str) -> String {
    collapse_whitespace(raw)
}

/// Return the rider name's canonical display case.
///
/// The roster's own rule decides the case, so import and the rider editor
/// spell a name the same way.
#[must_use]
pub(crate) fn canonical_person_name(raw: &str) -> String {
    rivercrossing_roster::names::canonical_person_name(raw)
}

/// One sex cell after folding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SexCell {
    /// A known cell: the folded token, or no value for a blank cell.
    Known(Option<String>),
    /// A non-blank cell that names no known sex, kept as the file spells it.
    Invalid(String),
}

/// Fold *raw*'s sex cell, applying the map-unknown-to-male opt-in (W10).
///
/// A blank cell is no value, and an unrecognised value is invalid, unless the
/// opt-in is on, which maps both to `M`.
#[must_use]
pub(crate) fn classify_sex(raw: &str, map_unknown_to_male: bool) -> SexCell {
    let cell = raw.trim();
    if cell.is_empty() {
        return if map_unknown_to_male {
            SexCell::Known(Some(MALE.to_owned()))
        } else {
            SexCell::Known(None)
        };
    }
    match cell.to_lowercase().as_str() {
        "male" | "m" => SexCell::Known(Some(MALE.to_owned())),
        "female" | "f" => SexCell::Known(Some(FEMALE.to_owned())),
        _ if map_unknown_to_male => SexCell::Known(Some(MALE.to_owned())),
        _ => SexCell::Invalid(cell.to_owned()),
    }
}

/// Derive the entry kind from the type cell and the team-name cell.
///
/// A blank cell names a team when the row names one, else a solo entry.
///
/// # Errors
///
/// Return `missing name` when the cell names a team but the row names no team,
/// and `unknown entry type "<value>"` when the cell names no known kind.
pub(crate) fn derive_kind(type_cell: &str, team_name: &str) -> Result<EntryType, String> {
    let cell = type_cell.trim();
    if cell.is_empty() {
        return Ok(if team_name.trim().is_empty() {
            EntryType::Solo
        } else {
            EntryType::Team
        });
    }
    match cell.to_lowercase().as_str() {
        "solo" => Ok(EntryType::Solo),
        "team" if !team_name.trim().is_empty() => Ok(EntryType::Team),
        "team" => Err(MISSING_TEAM_NAME.to_owned()),
        _ => Err(format!("unknown entry type {cell:?}")),
    }
}

/// Join the non-blank notes with `"; "` in slice order.
#[must_use]
pub(crate) fn join_notes(notes: &[&str]) -> String {
    notes
        .iter()
        .map(|note| note.trim())
        .filter(|note| !note.is_empty())
        .collect::<Vec<_>>()
        .join(NOTE_SEPARATOR)
}

#[cfg(test)]
mod tests;
