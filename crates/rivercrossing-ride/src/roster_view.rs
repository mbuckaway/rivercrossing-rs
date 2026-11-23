// SPDX-License-Identifier: GPL-3.0-only

//! The read and write seam that the ride engine uses to reach a roster.
//!
//! `rivercrossing-roster` implements [`RosterView`], so this crate never
//! depends on the roster crate. The engine reads owned snapshots and writes
//! through the trait's mutators.

use crate::error::RideError;
use crate::status::RideStatus;

/// The stable identity of one roster entry.
///
/// The id never changes, even when a mid-ride move re-derives the entry's plate.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntryId(pub String);

/// One rider as the ride engine reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiderSnapshot {
    /// The rider's own plate, when the plate model gives the rider one.
    pub plate: Option<String>,
    /// The rider's full name.
    pub full_name: String,
    /// The rider's sex spelling, when the roster records it.
    pub sex: Option<String>,
}

/// One entry as the ride engine reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntrySnapshot {
    /// The entry's stable identity.
    pub id: EntryId,
    /// The plate that the entry currently carries.
    pub plate: String,
    /// The name that the entry list shows.
    pub display_name: String,
    /// True when the entry is a team.
    pub is_team: bool,
    /// True when the entry is marked DNF.
    pub is_dnf: bool,
    /// The entry's riders.
    pub riders: Vec<RiderSnapshot>,
}

/// One reason that the roster is not ready to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartViolation {
    /// The plate of the entry that violates the start rule.
    pub plate: String,
    /// Why the entry blocks the start.
    pub reason: String,
}

/// The roster operations that the ride engine calls.
pub trait RosterView {
    /// Return a snapshot of every entry.
    fn entries(&self) -> Vec<EntrySnapshot>;

    /// Return the entry that *plate* resolves to, when one exists.
    fn resolve_plate(&self, plate: &str) -> Option<EntrySnapshot>;

    /// Return the entry that *id* names, when one exists.
    fn entry(&self, id: &EntryId) -> Option<EntrySnapshot>;

    /// Return every reason that the roster is not ready to start.
    fn start_violations(&self) -> Vec<StartViolation>;

    /// Set the ride status that the roster mirrors.
    fn set_ride_status(&mut self, status: RideStatus);

    /// Mark the entry as carrying recorded data.
    fn mark_has_data(&mut self, id: &EntryId);

    /// Mark as DNF either one rider of the entry or the whole entry.
    ///
    /// When *rider* is true, *plate* names the rider to mark. When it is
    /// false, the whole entry is marked and *plate* is recorded in the audit.
    #[expect(
        clippy::too_many_arguments,
        reason = "the trait keeps the roster method's explicit identity, plate, flag and reason"
    )]
    fn set_entry_dnf(&mut self, id: &EntryId, plate: &str, rider: bool, reason: &str);

    /// Move a rider into another entry.
    ///
    /// # Errors
    ///
    /// Return [`RideError::Roster`] when the roster refuses the move.
    #[expect(
        clippy::too_many_arguments,
        reason = "the trait keeps the roster method's explicit arguments; a parameter struct would force every implementor to unpack it"
    )]
    fn move_rider(
        &mut self,
        rider_plate: &str,
        to_entry: &EntryId,
        reason: &str,
    ) -> Result<(), RideError>;

    /// Move a rider out of a team into a new solo entry.
    ///
    /// Return the new entry's id, so the caller can re-attribute the rider's data.
    ///
    /// # Errors
    ///
    /// Return [`RideError::Roster`] when the roster refuses the move.
    fn extract_rider_to_solo(
        &mut self,
        rider_plate: &str,
        reason: &str,
    ) -> Result<EntryId, RideError>;
}
