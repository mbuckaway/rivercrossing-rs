// SPDX-License-Identifier: GPL-3.0-only

//! The `RosterView` implementation that the ride engine reads and mutates.
//!
//! The trait keeps the ride crate free of a roster dependency. Reads return
//! owned snapshots, so the engine never borrows the roster across a mutation.
//! Writes delegate to the roster's own audited mutators, except
//! [`RosterView::set_entry_dnf`], which the trait declares infallible and which
//! therefore ignores an unknown id.

use rivercrossing_ride::{
    EntryId, EntrySnapshot, RideError, RiderSnapshot, RosterView, StartViolation,
};

use crate::enums::{EntryStatus, EntryType, RideStatus};
use crate::roster::Roster;
use crate::types::{Entry, Rider};

#[cfg(test)]
mod tests;

impl RosterView for Roster {
    /// Return a snapshot of every live entry, in creation order.
    ///
    /// Retired entries are excluded: a dissolved entry no longer races, even
    /// though [`RosterView::entry`] still resolves its key for replay.
    /// `display_name` is the stored name: the team name for a team, the
    /// rider's full name for a solo.
    fn entries(&self) -> Vec<EntrySnapshot> {
        Roster::entries(self).iter().map(snapshot).collect()
    }

    /// Return the live entry that *plate* resolves to, when one exists.
    ///
    /// An entry's own plate always matches. On a `rider_pooled` ride a pooled
    /// rider's own plate resolves to the entry that carries the rider; a
    /// `team_relay` rider carries no plate, so only the entry plate matches
    /// there.
    fn resolve_plate(&self, plate: &str) -> Option<EntrySnapshot> {
        Roster::resolve_plate(self, plate).map(snapshot)
    }

    /// Return the live or retired entry that *id* names, when one exists.
    ///
    /// A retired entry stays resolvable because replay may still name the key
    /// that a pre-move crossing was filed under.
    fn entry(&self, id: &EntryId) -> Option<EntrySnapshot> {
        self.entry_by_key(&id.0).map(snapshot)
    }

    /// Return the roster's start-gate violations.
    ///
    /// This is [`Roster::validate_for_start`] behind the trait seam.
    fn start_violations(&self) -> Vec<StartViolation> {
        self.validate_for_start()
    }

    /// Record the ride status that the roster mirrors for its lock matrix.
    fn set_ride_status(&mut self, status: RideStatus) {
        self.set_status(status);
    }

    /// Mark the live or retired entry as carrying recorded data.
    ///
    /// The roster's own method is fallible, while this trait method is not, so
    /// an unknown id is a silent no-op.
    fn mark_has_data(&mut self, id: &EntryId) {
        drop(Roster::mark_has_data(self, id));
    }

    /// Set the live or retired entry's status to DNF.
    ///
    /// The ride engine keeps a per-rider DNF set itself and never asks the
    /// roster to mark a single rider, so *rider* is ignored. *plate* is
    /// recorded in the audit; *reason* stays with the engine's own DNF event.
    /// An unknown id is a silent no-op, because the trait method cannot fail.
    fn set_entry_dnf(&mut self, id: &EntryId, plate: &str, _rider: bool, _reason: &str) {
        let Some(entry) = self.entry_mut(id) else {
            return;
        };
        entry.status = EntryStatus::Dnf;
        self.log("set_entry_dnf", serde_json::json!({ "plate": plate }));
    }

    /// Move a rider into another entry through the roster's audited move.
    ///
    /// # Errors
    ///
    /// Return [`RideError::Roster`] when the roster refuses the move, with the
    /// roster's own message preserved.
    fn move_rider(
        &mut self,
        rider_plate: &str,
        to_entry: &EntryId,
        reason: &str,
    ) -> Result<(), RideError> {
        Roster::move_rider(self, rider_plate, to_entry, reason).map_err(RideError::from)
    }

    /// Extract a pooled team member into a new solo entry.
    ///
    /// Return the new entry's id, so the caller can re-attribute the rider's
    /// recorded data.
    ///
    /// # Errors
    ///
    /// Return [`RideError::Roster`] when the roster refuses the extraction,
    /// with the roster's own message preserved.
    fn extract_rider_to_solo(
        &mut self,
        rider_plate: &str,
        reason: &str,
    ) -> Result<EntryId, RideError> {
        Roster::extract_rider_to_solo(self, rider_plate, reason).map_err(RideError::from)
    }
}

/// Return the ride engine's snapshot of one entry.
fn snapshot(entry: &Entry) -> EntrySnapshot {
    EntrySnapshot {
        id: entry.id(),
        plate: entry.plate.clone(),
        display_name: entry.display_name.clone(),
        is_team: entry.kind == EntryType::Team,
        is_dnf: entry.status == EntryStatus::Dnf,
        riders: entry.riders.iter().map(rider_snapshot).collect(),
    }
}

/// Return the ride engine's snapshot of one rider.
fn rider_snapshot(rider: &Rider) -> RiderSnapshot {
    RiderSnapshot {
        plate: rider.plate.clone(),
        full_name: rider.full_name(),
        sex: rider.sex.clone(),
    }
}
