// SPDX-License-Identifier: GPL-3.0-only

//! The error type of the ride crate.

/// A failure from the ride config, the state machine, or the roster seam.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RideError {
    /// A setup value is outside its spec-defined bound.
    #[error("invalid ride config: {message}")]
    Config {
        /// Why the config is invalid.
        message: String,
    },
    /// The ride is in a state that forbids the transition.
    #[error("illegal ride state: {message}")]
    IllegalState {
        /// Why the transition is forbidden.
        message: String,
    },
    /// The start gate refused a DRAFT ride.
    #[error("ride start blocked: {message}")]
    StartBlocked {
        /// The summary of the refusal.
        message: String,
        /// One reason per field or entry that blocks the start.
        reasons: Vec<String>,
    },
    /// A plate names no entry or rider in the roster.
    #[error("unknown plate: {plate}")]
    UnknownPlate {
        /// The plate that the caller supplied.
        plate: String,
    },
    /// An event action is not one of the known actions.
    #[error("unknown event action: {action}")]
    UnknownEventAction {
        /// The action spelling that the caller supplied.
        action: String,
    },
    /// An entry id names no entry in the roster.
    #[error("unknown entry: {key}")]
    UnknownEntry {
        /// The entry id that the caller supplied.
        key: String,
    },
    /// An entry has no crossing with that sequence number.
    #[error("no crossing with entry_id {entry_id} seq {seq}")]
    NoCrossing {
        /// The entry that holds the crossings.
        entry_id: String,
        /// The sequence number that the caller supplied.
        seq: u32,
    },
    /// A correction would give a crossing a zero or negative lap time.
    #[error(
        "lap time must be positive for entry {entry_label}: {crossed_at} is not after {previous}"
    )]
    LapTimeNotPositive {
        /// The operator-facing plate of the entry that owns the crossing.
        entry_label: String,
        /// The corrected crossing instant, as naive ISO-8601 text.
        crossed_at: String,
        /// The instant the lap is measured from, as naive ISO-8601 text.
        previous: String,
    },
    /// A mutation that requires a reason received a blank one.
    #[error("reason must not be empty")]
    EmptyReason,
    /// A move's destination names no team in the roster.
    #[error("unknown team: {name}")]
    UnknownTeam {
        /// The team name that the caller supplied.
        name: String,
    },
    /// An extraction names a rider who is not on a pooled team.
    #[error("plate {plate} is not a pooled team member")]
    NotPooledTeamMember {
        /// The rider plate that the caller supplied.
        plate: String,
    },
    /// The roster seam refused a mutation.
    #[error("roster refused the change: {message}")]
    Roster {
        /// Why the roster refused the change.
        message: String,
    },
}
