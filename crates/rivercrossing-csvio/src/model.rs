// SPDX-License-Identifier: GPL-3.0-only

//! The public data model of a CSV import: the options, the parsed rows, the
//! preview with its problems, and the commit report.

use std::path::PathBuf;

use rivercrossing_roster::enums::EntryType;
use rivercrossing_roster::types::AuditEvent;

/// The two explicit, off-by-default import options (R-21, R-78).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ImportOptions {
    /// Map a blank or unrecognised sex cell to `M`.
    pub map_unknown_sex_to_male: bool,
    /// Import a one-rider team row as a solo entry.
    pub convert_teams_of_one_to_solo: bool,
}

/// One parsed rider row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRider {
    /// The rider's first name.
    pub first_name: String,
    /// The rider's last name.
    pub last_name: String,
    /// The rider's plate, when the file names one.
    pub plate: Option<String>,
    /// The rider's sex spelling, when the file names one.
    pub sex: Option<String>,
}

/// One parsed entry: a solo rider or a team of riders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEntry {
    /// The entry's plate.
    pub plate: String,
    /// The name that the entry list shows.
    pub display_name: String,
    /// Whether the entry is a solo rider or a team.
    pub kind: EntryType,
    /// The entry's riders, in file order.
    pub riders: Vec<ParsedRider>,
    /// The entry's notes, joined from its member rows.
    pub notes: String,
}

/// One kind of preview conflict or warning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProblemCode {
    /// The header row maps no name column.
    Header,
    /// A row names a plate or team but no rider name.
    MissingName,
    /// Two rows name the same plate.
    DuplicatePlate,
    /// A type cell names no known entry kind.
    UnknownType,
    /// A sex cell names no known sex.
    InvalidSex,
    /// A plate cell is not numeric.
    NonDigitPlate,
    /// A team holds too few riders.
    TeamUnderMin,
    /// A team holds too many riders.
    TeamOverMax,
    /// A team's rows name different plates.
    TeamRowsDifferentPlates,
    /// A team row appears on a solo-only ride.
    SoloOnlyTeam,
    /// A re-import would break the started ride's structure.
    BlockedReshape,
    /// Two riders share a case-insensitive name.
    DuplicateRiderName,
    /// Two team names differ only in punctuation or spacing.
    NearDuplicateTeamName,
}

/// One preview problem: where it sits and what it means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportProblem {
    /// The one-based file row, or zero for a whole-file problem.
    pub row: usize,
    /// The kind of problem.
    pub code: ProblemCode,
    /// The operator-facing sentence.
    pub message: String,
}

/// The roster state that a preview was computed against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreviewFingerprint {
    /// The roster's audit-log length.
    pub audit_len: usize,
    /// The roster's entry count.
    pub entries: usize,
    /// The roster's rider count.
    pub riders: usize,
}

/// The result of previewing a CSV file without writing anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPreview {
    /// The file that was previewed.
    pub source_path: PathBuf,
    /// The number of parsed riders.
    pub riders: usize,
    /// The number of parsed teams.
    pub teams: usize,
    /// The parsed entries, in file order.
    pub entries: Vec<ParsedEntry>,
    /// The blocking conflicts, in file order.
    pub conflicts: Vec<ImportProblem>,
    /// The non-blocking warnings, in file order.
    pub warnings: Vec<ImportProblem>,
    /// The roster state that a commit must still match.
    pub(crate) fingerprint: PreviewFingerprint,
}

/// The result of committing a preview.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportReport {
    /// The number of inserted entries.
    pub inserted: usize,
    /// The number of renamed entries.
    pub updated: usize,
    /// The number of moved riders.
    pub moved: usize,
    /// The number of riders extracted from a reshaped team.
    pub extracted: usize,
    /// The number of riders joined to a team.
    pub joined: usize,
    /// The audit events that the commit appended.
    pub audit_events: Vec<AuditEvent>,
}

#[cfg(test)]
mod tests;
