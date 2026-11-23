// SPDX-License-Identifier: GPL-3.0-only

//! The rider moves: the pooled move between entries, the extraction to a solo
//! entry, the add of an unplaced rider onto a team, and the rider removal.
//!
//! A move is one audited membership change. The source may be a team or a solo
//! entry; the destination is always a team, live or retired. When the source
//! loses its last rider it dissolves: an empty entry has no plate owner, so it
//! ceases to exist. An entry that carries recorded data is retired instead of
//! discarded, so replay can still resolve the crossings filed under its key.

use rivercrossing_ride::EntryId;
use serde_json::json;

use crate::enums::{EntryType, PlateModel, RideStatus};
use crate::error::RosterError;
use crate::lock;
use crate::plates;
use crate::roster::{Roster, delete_refusal, entry_not_found, rider_not_found};
use crate::types::{Entry, Rider};

#[cfg(test)]
mod tests;

/// The list that holds one entry, and its index in that list.
#[derive(Debug, Clone, Copy)]
enum Slot {
    /// An index into the live entry list.
    Live(usize),
    /// An index into the retired entry list.
    Retired(usize),
}

/// The entry that a move validated as a destination.
#[derive(Debug, Clone, Copy)]
struct Destination {
    /// The list and index that hold the entry.
    slot: Slot,
    /// The entry's kind.
    kind: EntryType,
    /// The entry's current rider count.
    size: usize,
}

impl Roster {
    /// Move the rider that *`rider_plate`* names onto the team that *to* names.
    ///
    /// The destination is a team, live or retired: replay may still move a
    /// rider into an entry that an earlier move dissolved. A `rider_pooled`
    /// ride re-derives both entries' plates from their riders, so the moved
    /// rider's own plate travels with them. A solo source empties at once, and
    /// a source that loses its last rider dissolves. The ride engine records
    /// *reason* in its own event; this roster's audit keeps the oracle's
    /// payload.
    ///
    /// # Errors
    ///
    /// Return [`RosterError::Locked`] when the lock matrix forbids a move in
    /// the ride's current state and plate model,
    /// [`RosterError::EntryNotFound`] when *to* names no entry here,
    /// [`RosterError::InvalidMove`] when the destination is not a team or the
    /// move would exceed its max size, and [`RosterError::RiderNotFound`] when
    /// no live entry holds the rider's plate.
    #[expect(
        clippy::too_many_arguments,
        reason = "the trait seam fixes the rider plate, the destination and the audit reason; a parameter struct would force every RosterView implementor to unpack it"
    )]
    pub fn move_rider(
        &mut self,
        rider_plate: &str,
        to: &EntryId,
        _reason: &str,
    ) -> Result<(), RosterError> {
        let destination = self.move_destination(to)?;
        let Some((source_index, rider_index, _kind)) = self.rider_position(rider_plate) else {
            return Err(rider_not_found());
        };
        let model = self.plate_model();
        let source_key = self.entry_key(source_index);
        let rider = self.take_rider(source_index, rider_index);
        let rider_name = rider.full_name();
        let to_plate = self.place_rider(destination.slot, rider);
        if !self.source_is_empty(source_index) {
            recompute(model, self.entry_at(Slot::Live(source_index)));
        }
        let from_plate = self
            .entry_by_key(&source_key)
            .map_or_else(String::new, |entry| entry.plate.clone());
        self.log(
            "move_rider",
            json!({"rider_name": rider_name, "from_plate": from_plate, "to_plate": to_plate}),
        );
        if self.source_is_empty(source_index) {
            self.dissolve_emptied(&EntryId(source_key));
        }
        Ok(())
    }

    /// Convert the pooled team member that *`rider_plate`* names into a solo
    /// entry, and return the new entry's id.
    ///
    /// The rider's own plate becomes the new entry's, so the rider keeps the
    /// identity their crossings were filed under. The source team re-derives
    /// its adopted plate and dissolves when the rider was its last member.
    ///
    /// # Errors
    ///
    /// Return [`RosterError::RiderNotFound`] when no live entry holds the
    /// rider's plate, [`RosterError::Locked`] when the lock matrix forbids a
    /// conversion in the ride's current state and plate model, and
    /// [`RosterError::InvalidMove`] when the ride is not `rider_pooled` or the
    /// rider is not on a team.
    pub fn extract_rider_to_solo(
        &mut self,
        rider_plate: &str,
        _reason: &str,
    ) -> Result<EntryId, RosterError> {
        let status = self.status();
        let model = self.plate_model();
        if !lock::can_move_rider(status, model) {
            return Err(extract_locked(status, model));
        }
        let Some((source_index, rider_index, kind)) = self.rider_position(rider_plate) else {
            return Err(rider_not_found());
        };
        if model != PlateModel::RiderPooled || kind != EntryType::Team {
            return Err(RosterError::InvalidMove {
                message: "extract_rider_to_solo requires a rider_pooled team member".to_owned(),
            });
        }
        let source_key = self.entry_key(source_index);
        let (rider, from_display_name, emptied) = {
            let entries = self.entries_mut();
            #[expect(
                clippy::indexing_slicing,
                reason = "the index came from the same, unmodified entry vector"
            )]
            let source = &mut entries[source_index];
            let rider = source.riders.remove(rider_index);
            let from_display_name = source.display_name.clone();
            let emptied = source.riders.is_empty();
            if !emptied {
                recompute(model, source);
            }
            (rider, from_display_name, emptied)
        };
        let display_name = rider.full_name();
        let entry = Entry::new(
            rider_plate,
            display_name.clone(),
            EntryType::Solo,
            vec![rider],
        );
        let id = entry.id();
        self.entries_mut().push(entry);
        self.log(
            "extract_rider_to_solo",
            json!({
                "rider_name": display_name,
                "plate": rider_plate,
                "from_display_name": from_display_name,
            }),
        );
        if emptied {
            self.dissolve_emptied(&EntryId(source_key));
        }
        Ok(id)
    }

    /// Attach *rider*, not yet on any entry, onto the team that *to* names.
    ///
    /// A `rider_pooled` ride requires the new rider to carry a free,
    /// whole-number plate and re-derives the team's adopted plate; a
    /// `team_relay` team clears the rider to plateless.
    ///
    /// # Errors
    ///
    /// Return [`RosterError::InvalidMove`] when the rider is already on an
    /// entry, the destination is not a team, or a pooled rider carries no
    /// plate, [`RosterError::EntryNotFound`] when *to* names no entry here,
    /// [`RosterError::Locked`] when the lock matrix forbids the add,
    /// [`RosterError::TeamSize`] when the add would exceed `max_team_size`,
    /// [`RosterError::PlateShape`] when the plate is not a whole number, and
    /// [`RosterError::DuplicatePlate`] when the plate is already claimed.
    pub fn add_rider_to_team(&mut self, mut rider: Rider, to: &EntryId) -> Result<(), RosterError> {
        if self
            .entries()
            .iter()
            .any(|entry| entry.riders.contains(&rider))
        {
            return Err(RosterError::InvalidMove {
                message: "rider is already on an entry; use move_rider instead".to_owned(),
            });
        }
        let destination = self.add_destination(to)?;
        let model = self.plate_model();
        if model == PlateModel::RiderPooled {
            self.require_addable_plate(&rider)?;
        } else {
            rider.plate = None;
        }
        let rider_name = rider.full_name();
        let rider_plate = rider.plate.clone();
        let (to_plate, display_name) = {
            let target = self.entry_at(destination.slot);
            target.riders.push(rider);
            recompute(model, target);
            (target.plate.clone(), target.display_name.clone())
        };
        self.log(
            "add_rider_to_team",
            json!({
                "rider_name": rider_name,
                "rider_plate": rider_plate,
                "to_plate": to_plate,
                "display_name": display_name,
            }),
        );
        Ok(())
    }

    /// Remove the rider at *index* from the entry that *entry* names.
    ///
    /// A solo entry *is* its rider, so that case delegates to
    /// [`Self::delete_entry`] and keeps its refusal. A team loses only the
    /// named member and survives with the riders left: a pooled team re-derives
    /// its plate, and a team with no riders left dissolves. The index is what
    /// lets an unplated `team_relay` rider be removed, because those riders
    /// carry no plate to name them.
    ///
    /// # Errors
    ///
    /// Return [`RosterError::EntryNotFound`] when *entry* names no live entry,
    /// [`RosterError::RiderNotFound`] when the entry has no rider at *index*,
    /// and [`RosterError::Locked`] when the entry carries recorded data or the
    /// ride has left draft.
    pub fn remove_rider(&mut self, entry: &EntryId, index: usize) -> Result<(), RosterError> {
        let status = self.status();
        let Some((entry_index, kind, has_data, size)) = self.live_entry_facts(entry) else {
            return Err(entry_not_found(entry));
        };
        if kind == EntryType::Solo {
            return self.delete_entry(entry);
        }
        if index >= size {
            return Err(rider_not_found());
        }
        if !lock::can_delete_entry(status, has_data) {
            return Err(RosterError::Locked {
                message: delete_refusal(status, has_data),
            });
        }
        let model = self.plate_model();
        let (rider_name, plate, display_name, emptied) = {
            let entries = self.entries_mut();
            #[expect(
                clippy::indexing_slicing,
                reason = "the index came from the same, unmodified entry vector"
            )]
            let target = &mut entries[entry_index];
            let rider = target.riders.remove(index);
            let rider_name = rider.full_name();
            let emptied = target.riders.is_empty();
            if !emptied {
                recompute(model, target);
            }
            (
                rider_name,
                target.plate.clone(),
                target.display_name.clone(),
                emptied,
            )
        };
        if emptied {
            self.dissolve_emptied(entry);
        }
        self.log(
            "remove_rider",
            json!({"rider_name": rider_name, "plate": plate, "display_name": display_name}),
        );
        Ok(())
    }

    /// Validate a move's destination: the lock, the entry, its kind and its
    /// room for one more rider.
    fn move_destination(&self, to: &EntryId) -> Result<Destination, RosterError> {
        let status = self.status();
        let model = self.plate_model();
        if !lock::can_move_rider(status, model) {
            return Err(moves_locked(status, model));
        }
        let Some(destination) = self.destination(to) else {
            return Err(entry_not_found(to));
        };
        if destination.kind != EntryType::Team {
            return Err(RosterError::InvalidMove {
                message: "move_rider's destination must be a team entry".to_owned(),
            });
        }
        let next_size = u32::try_from(destination.size + 1).unwrap_or(u32::MAX);
        if next_size > self.max_team_size() {
            return Err(RosterError::InvalidMove {
                message: "move would exceed the destination team's max size".to_owned(),
            });
        }
        Ok(destination)
    }

    /// Validate an add's destination: the entry, the lock, its kind and its
    /// room for one more rider.
    fn add_destination(&self, to: &EntryId) -> Result<Destination, RosterError> {
        let Some(destination) = self.destination(to) else {
            return Err(entry_not_found(to));
        };
        let status = self.status();
        let model = self.plate_model();
        if !lock::can_move_rider(status, model) {
            return Err(moves_locked(status, model));
        }
        if destination.kind != EntryType::Team {
            return Err(RosterError::InvalidMove {
                message: "add_rider_to_team requires a team entry".to_owned(),
            });
        }
        let next_size = u32::try_from(destination.size + 1).unwrap_or(u32::MAX);
        if next_size > self.max_team_size() {
            return Err(RosterError::TeamSize {
                message: format!(
                    "team size must be at most {}, got {next_size}",
                    self.max_team_size()
                ),
            });
        }
        Ok(destination)
    }

    /// Refuse a pooled rider's plate unless it is whole and free.
    fn require_addable_plate(&self, rider: &Rider) -> Result<(), RosterError> {
        let Some(plate) = rider.plate.clone() else {
            return Err(RosterError::InvalidMove {
                message: "rider_pooled riders must each carry a plate".to_owned(),
            });
        };
        plates::require_whole_plate(&plate)?;
        plates::require_plates_free(self.entries(), &[plate.as_str()])
    }

    /// Take the rider at *`rider_index`* out of the live entry at
    /// *`entry_index`*.
    fn take_rider(&mut self, entry_index: usize, rider_index: usize) -> Rider {
        let entries = self.entries_mut();
        #[expect(
            clippy::indexing_slicing,
            reason = "the index came from the same, unmodified entry vector"
        )]
        let source = &mut entries[entry_index];
        source.riders.remove(rider_index)
    }

    /// Append *rider* to the entry that *slot* names, and return its plate.
    fn place_rider(&mut self, slot: Slot, rider: Rider) -> String {
        let model = self.plate_model();
        let target = self.entry_at(slot);
        target.riders.push(rider);
        recompute(model, target);
        target.plate.clone()
    }

    /// Return the live entry's position, kind, data flag and rider count.
    fn live_entry_facts(&self, id: &EntryId) -> Option<(usize, EntryType, bool, usize)> {
        self.entries()
            .iter()
            .enumerate()
            .find(|(_, target)| target.key == id.0)
            .map(|(position, target)| (position, target.kind, target.has_data, target.riders.len()))
    }

    /// Return the live or retired entry that *slot* names.
    ///
    /// The slot came from a lookup on the same, unmodified list, so its index
    /// is in range.
    fn entry_at(&mut self, slot: Slot) -> &mut Entry {
        match slot {
            Slot::Live(index) => {
                let entries = self.entries_mut();
                #[expect(
                    clippy::indexing_slicing,
                    reason = "the index came from the same, unmodified entry list"
                )]
                &mut entries[index]
            }
            Slot::Retired(index) => {
                let retired = self.retired_mut();
                #[expect(
                    clippy::indexing_slicing,
                    reason = "the index came from the same, unmodified retired list"
                )]
                &mut retired[index]
            }
        }
    }

    /// Return the live or retired entry that *id* names, with its kind and size.
    fn destination(&self, id: &EntryId) -> Option<Destination> {
        if let Some((index, entry)) = self
            .entries()
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.key == id.0)
        {
            return Some(Destination {
                slot: Slot::Live(index),
                kind: entry.kind,
                size: entry.riders.len(),
            });
        }
        let (index, entry) = self
            .retired_entries()
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.key == id.0)?;
        Some(Destination {
            slot: Slot::Retired(index),
            kind: entry.kind,
            size: entry.riders.len(),
        })
    }

    /// Return the live entry and rider positions of the rider *`rider_plate`*
    /// names, with the entry's kind.
    fn rider_position(&self, rider_plate: &str) -> Option<(usize, usize, EntryType)> {
        self.entries()
            .iter()
            .enumerate()
            .find_map(|(index, entry)| {
                entry
                    .riders
                    .iter()
                    .position(|rider| rider.plate.as_deref() == Some(rider_plate))
                    .map(|position| (index, position, entry.kind))
            })
    }

    /// Return the key of the live entry at *index*.
    fn entry_key(&self, index: usize) -> String {
        let entries = self.entries();
        #[expect(
            clippy::indexing_slicing,
            reason = "the index came from the same, unmodified entry vector"
        )]
        let key = entries[index].key.clone();
        key
    }

    /// Return `true` when the live entry at *index* has no riders.
    fn source_is_empty(&self, index: usize) -> bool {
        self.entries()
            .get(index)
            .is_some_and(|entry| entry.riders.is_empty())
    }
}

/// Re-derive a `rider_pooled` entry's plate, keeping a relay entry's own plate.
fn recompute(model: PlateModel, entry: &mut Entry) {
    if model == PlateModel::RiderPooled {
        plates::recompute_pooled_plate(entry);
    }
}

/// Return the lock refusal for a rider move.
fn moves_locked(status: RideStatus, model: PlateModel) -> RosterError {
    RosterError::Locked {
        message: format!(
            "rider moves are locked for a {} ride once {}",
            model.as_str(),
            status.as_str()
        ),
    }
}

/// Return the lock refusal for an extraction to solo.
fn extract_locked(status: RideStatus, model: PlateModel) -> RosterError {
    RosterError::Locked {
        message: format!(
            "a rider cannot be extracted to solo on a {} ride once {}",
            model.as_str(),
            status.as_str()
        ),
    }
}
