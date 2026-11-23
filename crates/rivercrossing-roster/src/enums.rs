// SPDX-License-Identifier: GPL-3.0-only

//! The roster enums.
//!
//! The ride-wide settings are re-exported from the ride crate so the roster
//! and the engine share one definition. The entry kind and the entry status
//! belong to the roster alone, so the crate defines them here.

use std::str::FromStr;

pub use rivercrossing_ride::{EntryMode, PlateModel, RideStatus};

use crate::error::RosterError;

/// An entry's kind (spec S2 `entry.type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryType {
    /// One rider, alone on course.
    Solo,
    /// A team of riders who share one placing.
    Team,
}

impl EntryType {
    /// Return the stored spelling of this entry kind.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Solo => "solo",
            Self::Team => "team",
        }
    }
}

impl FromStr for EntryType {
    type Err = RosterError;

    /// Read a stored entry-kind spelling.
    ///
    /// # Errors
    ///
    /// Return [`RosterError::InvalidMove`] when the text is not a known
    /// spelling.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "solo" => Ok(Self::Solo),
            "team" => Ok(Self::Team),
            other => Err(unknown_spelling("entry type", other)),
        }
    }
}

/// An entry's standing (spec S2 `entry.status`).
///
/// Marking an entry DNF is a live-ride concern; this crate stores only the
/// status that a created entry carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryStatus {
    /// The entry is still racing.
    Active,
    /// The entry did not finish.
    Dnf,
}

impl EntryStatus {
    /// Return the stored spelling of this entry status.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Dnf => "dnf",
        }
    }
}

impl FromStr for EntryStatus {
    type Err = RosterError;

    /// Read a stored entry-status spelling.
    ///
    /// # Errors
    ///
    /// Return [`RosterError::InvalidMove`] when the text is not a known
    /// spelling.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "active" => Ok(Self::Active),
            "dnf" => Ok(Self::Dnf),
            other => Err(unknown_spelling("entry status", other)),
        }
    }
}

/// Return the error for an unknown stored enum spelling.
///
/// The roster has no separate spelling variant, because a stored value that
/// the crate cannot read is a structural refusal like any other.
fn unknown_spelling(kind: &str, value: &str) -> RosterError {
    RosterError::InvalidMove {
        message: format!("unknown {kind}: {value}"),
    }
}

#[cfg(test)]
mod tests;
