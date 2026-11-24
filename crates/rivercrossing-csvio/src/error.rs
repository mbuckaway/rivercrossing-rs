// SPDX-License-Identifier: GPL-3.0-only

//! The error type of the CSV import and export crate.

use rivercrossing_roster::error::RosterError;
use rivercrossing_standings::StandingsError;

/// A CSV import or export refusal.
///
/// Each stage of the pipeline names itself, so an operator reads where the
/// import or the export stopped. A wrapped roster or standings refusal keeps
/// that crate's own sentence as the display text.
#[derive(Debug, thiserror::Error)]
pub enum CsvIoError {
    /// A filesystem read or write failed.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// The payload is not valid `UTF-8`.
    #[error("csv data is not valid utf-8")]
    NotUtf8,
    /// The CSV reader rejected a record.
    #[error("{message}")]
    Malformed {
        /// The reader's sentence, prefixed with the stage.
        message: String,
    },
    /// The header row maps no name column.
    #[error("{message}")]
    Header {
        /// The header resolver's sentence.
        message: String,
    },
    /// The preview holds blocking conflicts.
    #[error("import has {count} blocking conflicts")]
    ConflictsPresent {
        /// The number of blocking conflicts.
        count: usize,
    },
    /// The roster refuses an import mutation.
    #[error("{message}")]
    Blocked {
        /// The roster's refusal sentence.
        message: String,
    },
    /// The committed preview no longer matches the roster.
    #[error("the preview is stale; preview the file again")]
    StalePreview,
    /// A standings export ran before the ride finished.
    #[error("the ride is not finished (status: {status})")]
    NotFinished {
        /// The ride's current status.
        status: String,
    },
    /// The standings hold no row for an entry.
    #[error("no standings row for plate '{plate}'")]
    MissingStandings {
        /// The plate that the standings do not hold.
        plate: String,
    },
    /// The roster refused a mutation.
    #[error(transparent)]
    Roster(#[from] RosterError),
    /// The standings refused a lookup.
    #[error(transparent)]
    Standings(#[from] StandingsError),
}

#[cfg(test)]
mod tests;
