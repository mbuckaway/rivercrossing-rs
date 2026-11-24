// SPDX-License-Identifier: GPL-3.0-only

//! The exporter's error type.

use rivercrossing_standings::StandingsError;

/// A refusal from the HTML exporter.
///
/// Each variant names the stage that refused, so a caller can tell a bad
/// plate from an unnameable hand, a template failure and an unreadable
/// record.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// A roster plate is not numeric, so the record cannot hold it as an
    /// integer.
    #[error("plate '{plate}' is not numeric; htmlexport.render() emits plates as integers")]
    PlateNotNumeric {
        /// The plate spelling that the caller supplied.
        plate: String,
    },
    /// The standings crate refused to name a hand.
    #[error(transparent)]
    HandName(#[from] StandingsError),
    /// A template could not render.
    #[error("template: {0}")]
    Template(String),
    /// A `race-data` record could not be read or written.
    #[error("record: {0}")]
    Json(String),
}

#[cfg(test)]
mod tests;
