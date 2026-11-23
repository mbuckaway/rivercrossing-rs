// SPDX-License-Identifier: GPL-3.0-only

//! The start gate: the reasons that the roster is not ready to start.
//!
//! One rule is deferred to start time. A team below the ride's minimum size is
//! allowed transiently in draft, because the rider editor and the mid-ride
//! move both pass through a smaller team, but it must be repaired before the
//! ride starts. The upper bound is never deferred: the create and move paths
//! enforce it immediately.

use rivercrossing_ride::StartViolation;

use crate::enums::EntryType;
use crate::roster::{MIN_TEAM_SIZE, Roster};

#[cfg(test)]
mod tests;

impl Roster {
    /// Return every reason that this roster is not ready to start.
    ///
    /// Report one violation per live team whose rider count is below the
    /// minimum of two, in entry creation order. Solos, retired entries and an
    /// empty roster produce no violation. The reported plate is the entry's
    /// current plate, so a pooled team reports the plate it re-derived from
    /// its riders.
    #[must_use]
    pub fn validate_for_start(&self) -> Vec<StartViolation> {
        self.entries()
            .iter()
            .filter(|entry| entry.kind == EntryType::Team)
            .filter(|entry| entry.team_size() < MIN_TEAM_SIZE as usize)
            .map(|entry| StartViolation {
                plate: entry.plate.clone(),
                reason: format!(
                    "team size must be at least {MIN_TEAM_SIZE}, got {}",
                    entry.team_size()
                ),
            })
            .collect()
    }
}
