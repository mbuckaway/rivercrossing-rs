// SPDX-License-Identifier: GPL-3.0-only

//! The error type of the roster crate.
//!
//! Each message keeps the Python oracle's wording, so an operator-facing
//! refusal reads the same in the port as in the Python app. A roster failure
//! reaches the ride engine as [`RideError::Roster`], which carries the same
//! text.

use rivercrossing_ride::RideError;

/// A roster invariant violation.
///
/// The `message` fields carry the oracle's exact sentence for a refusal that
/// depends on run-time values, such as a team size or a lock state.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RosterError {
    /// A plate collides with another entry's or rider's plate (R-20).
    #[error("plate '{plate}' is already in use")]
    DuplicatePlate {
        /// The plate that the caller supplied.
        plate: String,
    },
    /// A team size, or the ride's `max_team_size`, is outside its bound (R-12).
    #[error("{message}")]
    TeamSize {
        /// Why the size is refused.
        message: String,
    },
    /// A team entry was attempted on a solo-only ride (R-11).
    #[error("this ride is solo-only; team entries are not allowed")]
    SoloOnlyRide,
    /// A plate value violates the ride's plate-model shape (spec S1).
    #[error("plate '{plate}' {reason}")]
    PlateShape {
        /// The plate that the caller supplied.
        plate: String,
        /// Why the plate shape is refused.
        reason: String,
    },
    /// The referenced entry is not a member of this roster.
    #[error("entry {key} is not a member of this roster")]
    EntryNotFound {
        /// The entry key that the caller supplied.
        key: String,
    },
    /// The referenced rider is not on any entry in this roster.
    #[error("{description}")]
    RiderNotFound {
        /// The oracle's sentence that names why the rider is not found.
        description: String,
    },
    /// A move would violate a structural entry invariant.
    #[error("{message}")]
    InvalidMove {
        /// Why the move is refused.
        message: String,
    },
    /// The lock matrix refuses a mutation (R-15, R-17).
    #[error("{message}")]
    Locked {
        /// Why the mutation is locked.
        message: String,
    },
}

impl From<RosterError> for RideError {
    /// Wrap a roster refusal as the ride engine's roster error.
    fn from(error: RosterError) -> Self {
        Self::Roster {
            message: error.to_string(),
        }
    }
}

#[cfg(test)]
mod tests;
