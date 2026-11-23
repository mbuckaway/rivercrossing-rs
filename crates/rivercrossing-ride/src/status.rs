// SPDX-License-Identifier: GPL-3.0-only

//! The ride lifecycle state.

use std::str::FromStr;

use crate::error::RideError;

/// A ride's lifecycle state (spec §3).
///
/// The stored spelling of each state is lowercase, so a value read from the
/// `ride.status` column round-trips through [`FromStr`] unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RideStatus {
    /// The ride is set up but not started.
    Draft,
    /// The clock is running.
    Running,
    /// The ride is finished and its results are final.
    Finished,
    /// The ride is reopened for corrections.
    Reopened,
}

impl RideStatus {
    /// Return the stored spelling of this status.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Running => "running",
            Self::Finished => "finished",
            Self::Reopened => "reopened",
        }
    }
}

impl FromStr for RideStatus {
    type Err = RideError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "draft" => Ok(Self::Draft),
            "running" => Ok(Self::Running),
            "finished" => Ok(Self::Finished),
            "reopened" => Ok(Self::Reopened),
            other => Err(RideError::IllegalState {
                message: format!("unknown ride status: {other}"),
            }),
        }
    }
}

#[cfg(test)]
mod tests;
