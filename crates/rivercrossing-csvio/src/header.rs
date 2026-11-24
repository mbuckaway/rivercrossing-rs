// SPDX-License-Identifier: GPL-3.0-only

//! The header row of the roster CSV: each column resolves to a canonical field.
//!
//! The file's own header decides where each field lives (R-24, spec §7). The
//! matchers run in a fixed order, so a column that reads like two fields is
//! claimed by the earlier one, an unmatched column is ignored, and a repeated
//! field keeps its first column.

use crate::normalize::collapse_whitespace;

/// The row-1 error for a header that names no rider-name column.
const NO_NAME_COLUMN: &str = "missing or malformed header: no first or last name column";

/// The whole-header spellings that name the plate column.
const PLATE_HEADERS: [&str; 4] = ["plate", "plate number", "number", "bib"];

/// The delimited words that name the sex column.
const SEX_TOKENS: [&str; 1] = ["sex"];

/// The delimited words that name the notes column.
const NOTE_TOKENS: [&str; 2] = ["note", "notes"];

/// The column index of each canonical field in the file's header row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct HeaderMap {
    /// The TEAMNAME column.
    pub team_name: Option<usize>,
    /// The TYPE column.
    pub kind: Option<usize>,
    /// The FIRSTNAME column.
    pub first_name: Option<usize>,
    /// The LASTNAME column.
    pub last_name: Option<usize>,
    /// The SEX column.
    pub sex: Option<usize>,
    /// The PLATE column.
    pub plate: Option<usize>,
    /// The NOTES column.
    pub notes: Option<usize>,
}

/// Resolve each header cell to its canonical column.
///
/// # Errors
///
/// Return the row-1 message when no column names a first name or a last name.
pub(crate) fn resolve(cells: &[String]) -> Result<HeaderMap, String> {
    let mut map = HeaderMap::default();
    for (index, cell) in cells.iter().enumerate() {
        claim_column(&mut map, index, cell.trim());
    }
    if map.first_name.is_none() && map.last_name.is_none() {
        return Err(NO_NAME_COLUMN.to_owned());
    }
    Ok(map)
}

/// Claim *index* for the first free canonical field that *header* matches.
fn claim_column(map: &mut HeaderMap, index: usize, header: &str) {
    if map.team_name.is_none() && is_team_name(header) {
        map.team_name = Some(index);
    } else if map.kind.is_none() && is_type(header) {
        map.kind = Some(index);
    } else if map.first_name.is_none() && is_first_name(header) {
        map.first_name = Some(index);
    } else if map.last_name.is_none() && is_last_name(header) {
        map.last_name = Some(index);
    } else if map.sex.is_none() && is_sex(header) {
        map.sex = Some(index);
    } else if map.plate.is_none() && is_plate(header) {
        map.plate = Some(index);
    } else if map.notes.is_none() && is_notes(header) {
        map.notes = Some(index);
    }
}

/// Return *header* with its whitespace removed and its letters lowercased.
fn compact(header: &str) -> String {
    header.split_whitespace().collect::<String>().to_lowercase()
}

/// Return whether *header* names the team-name column.
fn is_team_name(header: &str) -> bool {
    compact(header).contains("teamname")
}

/// Return whether *header* names the type column.
fn is_type(header: &str) -> bool {
    let folded = header.to_lowercase();
    folded == "type" || (folded.contains("solo") && folded.contains("team"))
}

/// Return whether *header* names the first-name column.
fn is_first_name(header: &str) -> bool {
    compact(header).contains("firstname")
}

/// Return whether *header* names the last-name column.
fn is_last_name(header: &str) -> bool {
    compact(header).contains("lastname")
}

/// Return whether *header* names the sex column.
fn is_sex(header: &str) -> bool {
    has_token(header, &SEX_TOKENS)
}

/// Return whether *header* names the plate column.
fn is_plate(header: &str) -> bool {
    PLATE_HEADERS.contains(&collapse_whitespace(header).to_lowercase().as_str())
}

/// Return whether *header* names the notes column.
fn is_notes(header: &str) -> bool {
    has_token(header, &NOTE_TOKENS)
}

/// Return whether *header* carries one of *tokens* as a delimited word.
fn has_token(header: &str, tokens: &[&str]) -> bool {
    header
        .to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .any(|token| tokens.contains(&token))
}

#[cfg(test)]
mod tests;
