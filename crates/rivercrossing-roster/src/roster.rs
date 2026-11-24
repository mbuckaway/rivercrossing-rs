// SPDX-License-Identifier: GPL-3.0-only

//! The roster aggregate: the entries, riders and teams of one ride.
//!
//! A roster holds the ride-wide settings that every later mutation validates
//! against, the current ride state that the lock matrix consults, and the
//! append-only audit log. It is a store-less, in-memory model; the store
//! persists it.
//!
//! Every mutator speaks in the entry's stable [`EntryId`], never in a reference
//! to an entry the roster owns, so a caller can hold an id across a mutation.
//! A plate change names the member rider by its own plate, for the same reason.

use std::collections::BTreeSet;

use rivercrossing_cards::seeded_card_codes;
use rivercrossing_ride::EntryId;
use serde_json::{Value, json};

use crate::enums::{EntryMode, EntryType, PlateModel, RideStatus};
use crate::error::RosterError;
use crate::lock;
use crate::logos;
use crate::plates;
use crate::types::{AuditEvent, Entry, Rider};

/// The smallest team that the ride setup accepts (R-12).
pub const MIN_TEAM_SIZE: u32 = 2;
/// The largest team that the ride setup accepts (R-12).
const MAX_TEAM_SIZE_LIMIT: u32 = 10;

/// The fields that a create-team form collects.
///
/// The five create paths read one `TeamFields`, so the CSV bulk path, the
/// rider editor and the empty-team form cannot drift apart.
#[derive(Debug, Clone, Default)]
pub struct TeamFields<'a> {
    /// The name that the entry list shows.
    pub display_name: &'a str,
    /// The team's riders, in display order.
    pub riders: Vec<Rider>,
    /// The caller's explicit plate, when the path takes one.
    pub plate: Option<&'a str>,
    /// The caller's logo card; `None` auto-assigns the next seeded one.
    pub logo_card: Option<&'a str>,
}

/// The fields that an entry editor can change.
///
/// A `None` field keeps the entry's current value.
#[derive(Debug, Clone, Copy, Default)]
pub struct EntryEdit<'a> {
    /// The new name that the entry list shows.
    pub display_name: Option<&'a str>,
    /// The new operator note.
    pub notes: Option<&'a str>,
}

/// The fields that a rider editor can change.
///
/// The names always take the supplied value. A `None` sex clears the stored
/// one, so a blank registration field written back through an editor removes
/// a stale `"M"` or `"F"`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RiderEdit<'a> {
    /// The rider's new first name.
    pub first_name: &'a str,
    /// The rider's new last name.
    pub last_name: &'a str,
    /// The rider's new sex spelling, or `None` to clear it.
    pub sex: Option<&'a str>,
}

/// One plate change through the model-correct dispatcher.
#[derive(Debug, Clone, Copy)]
pub struct PlateChange<'a> {
    /// The entry whose plate changes.
    pub entry: &'a EntryId,
    /// The member rider's own plate, on a `rider_pooled` team.
    pub rider_plate: Option<&'a str>,
    /// The new plate.
    pub plate: &'a str,
}

/// The entries and riders of one ride, with the ride's hard rules.
///
/// The constructor refuses a team size outside 2..10 and builds the team-logo
/// card deck when the ride has a shuffle seed. Every later mutation validates
/// against the stored settings and appends one [`AuditEvent`].
///
/// A clone is a stage-and-swap snapshot for an atomic import: build the
/// replacement roster to the side, then swap it in. The clone carries the audit
/// log, so it must replace the roster outright and never merge into it, because
/// a merge would duplicate the events the original already recorded.
#[derive(Debug, Clone)]
pub struct Roster {
    /// Every entry, in creation order.
    entries: Vec<Entry>,
    /// Every dissolved entry that carried recorded data, in dissolve order.
    retired: Vec<Entry>,
    /// Every audit event, oldest first.
    audit: Vec<AuditEvent>,
    /// The ride lifecycle state that this roster mirrors.
    status: RideStatus,
    /// The ride's team policy (R-11).
    entry_mode: EntryMode,
    /// The ride's largest team (R-12).
    max_team_size: u32,
    /// The ride's plate policy (R-16).
    plate_model: PlateModel,
    /// The team-logo shuffle seed, when the ride has one.
    team_logo_seed: Option<u64>,
    /// The seeded natural-card deck that the logo pick draws from.
    logo_codes: Vec<String>,
}

impl Roster {
    /// Build an empty roster under the given ride-wide settings.
    ///
    /// # Errors
    ///
    /// Return [`RosterError::TeamSize`] when `max_team_size` is outside 2..10.
    #[expect(
        clippy::too_many_arguments,
        reason = "the four settings are keyword-only in the oracle; a parameter struct would add a type with one caller"
    )]
    pub fn new(
        entry_mode: EntryMode,
        max_team_size: u32,
        plate_model: PlateModel,
        team_logo_seed: Option<u64>,
    ) -> Result<Self, RosterError> {
        require_max_team_size(max_team_size)?;
        Ok(Self {
            entries: Vec::new(),
            retired: Vec::new(),
            audit: Vec::new(),
            status: RideStatus::Draft,
            entry_mode,
            max_team_size,
            plate_model,
            team_logo_seed,
            logo_codes: build_logo_codes(team_logo_seed),
        })
    }

    /// Return every entry, in creation order.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Return every retired entry, in dissolve order.
    #[must_use]
    pub fn retired_entries(&self) -> &[Entry] {
        &self.retired
    }

    /// Return every audit event, oldest first.
    #[must_use]
    pub fn audit_log(&self) -> &[AuditEvent] {
        &self.audit
    }

    /// Return the audit events in order, then clear the log.
    ///
    /// The store drains the events into the audit table after it saves a
    /// roster edit, so a later drain persists only what was recorded since.
    pub fn take_audit_log(&mut self) -> Vec<AuditEvent> {
        std::mem::take(&mut self.audit)
    }

    /// Return this roster's ride-lifecycle state.
    #[must_use]
    pub fn status(&self) -> RideStatus {
        self.status
    }

    /// Set this roster's ride-lifecycle state.
    ///
    /// The ride engine owns which transitions are legal; this setter only
    /// records the current state for the lock matrix to consult.
    pub fn set_status(&mut self, status: RideStatus) {
        self.status = status;
    }

    /// Return this ride's team policy (R-11).
    #[must_use]
    pub fn entry_mode(&self) -> EntryMode {
        self.entry_mode
    }

    /// Return this ride's largest team (R-12).
    #[must_use]
    pub fn max_team_size(&self) -> u32 {
        self.max_team_size
    }

    /// Return this ride's plate policy (R-16).
    #[must_use]
    pub fn plate_model(&self) -> PlateModel {
        self.plate_model
    }

    /// Return this ride's team-logo shuffle seed, when it has one.
    #[must_use]
    pub fn team_logo_seed(&self) -> Option<u64> {
        self.team_logo_seed
    }

    // ---------------------------------------------------------- ride shape

    /// Replace the ride's entry mode, largest team and plate model.
    ///
    /// The ride-setup seam: the existing entries and the audit log are left
    /// exactly as they are, and nothing is audited.
    ///
    /// # Errors
    ///
    /// Return [`RosterError::TeamSize`] when `max_team_size` is outside 2..10.
    #[expect(
        clippy::too_many_arguments,
        reason = "the three settings are keyword-only in the oracle and mirror Roster::new's own shape"
    )]
    pub fn update_shape(
        &mut self,
        entry_mode: EntryMode,
        max_team_size: u32,
        plate_model: PlateModel,
    ) -> Result<(), RosterError> {
        require_max_team_size(max_team_size)?;
        self.entry_mode = entry_mode;
        self.max_team_size = max_team_size;
        self.plate_model = plate_model;
        Ok(())
    }

    // ------------------------------------------------------------- lookups

    /// Return the entry that *plate* names, under the ride's plate model.
    #[must_use]
    pub fn resolve_plate(&self, plate: &str) -> Option<&Entry> {
        plates::resolve_plate(&self.entries, self.plate_model, plate)
    }

    /// Return the entry that *key* names, live or retired.
    #[must_use]
    pub fn entry_by_key(&self, key: &str) -> Option<&Entry> {
        self.entries
            .iter()
            .chain(self.retired.iter())
            .find(|entry| entry.key == key)
    }

    /// Return one past the highest numeric plate in use.
    #[must_use]
    pub fn next_free_plate(&self) -> String {
        plates::next_free_plate(&self.entries)
    }

    /// Return one past the highest numeric plate in use or in *reserved*.
    ///
    /// The namespace is the same one that [`Self::next_free_plate`] scans:
    /// every entry's plate and every rider's plate. A reserved plate counts
    /// even when no entry carries it, so a stage-and-swap import can keep a
    /// plate for an entry it has not inserted yet. A non-numeric plate is
    /// ignored, and an empty namespace returns `"1"`.
    #[must_use]
    pub fn next_free_plate_skipping(&self, reserved: &BTreeSet<String>) -> String {
        plates::next_free_plate_skipping(&self.entries, reserved)
    }

    /// Return the next unused seeded logo card, scanning just past *after*.
    #[must_use]
    pub fn next_team_logo_card(&self, after: Option<&str>) -> Option<String> {
        let claimed = logos::claimed_cards(&self.entries);
        logos::next_logo_card(&self.logo_codes, &claimed, after).map(str::to_owned)
    }

    // ------------------------------------------------------- store restore

    /// Restore persisted entries wholesale (the store's load seam).
    ///
    /// This is reconstruction, not mutation: the entries are appended with no
    /// validation and no audit event, because they came from a consistent
    /// persisted state. Re-logging a restore as a live edit would pollute the
    /// audit log with create events that never happened.
    pub fn load_entries(&mut self, entries: Vec<Entry>) {
        self.entries.extend(entries);
    }

    /// Restore persisted retired entries wholesale (the store's load seam).
    ///
    /// This is [`Self::load_entries`]'s counterpart for the retired list, with
    /// the same reconstruction-not-mutation contract: the entries are appended
    /// in dissolve order, silently.
    pub fn load_retired_entries(&mut self, entries: Vec<Entry>) {
        self.retired.extend(entries);
    }

    // ------------------------------------------------------------ creates

    /// Create a solo entry for *rider* on *plate*.
    ///
    /// The plate is shaped by the ride's plate model: a `team_relay` entry
    /// carries it and its rider carries none, while a `rider_pooled` entry
    /// mirrors it on the rider. A new plate is admitted in any ride state.
    ///
    /// # Errors
    ///
    /// Return [`RosterError::PlateShape`] when *plate* is blank or not a whole
    /// number, and [`RosterError::DuplicatePlate`] when another entry or rider
    /// already claims it.
    pub fn create_solo_entry(
        &mut self,
        mut rider: Rider,
        plate: &str,
    ) -> Result<EntryId, RosterError> {
        rider.plate = Some(plate.to_owned());
        let shaped = self.shape_and_validate(std::slice::from_mut(&mut rider), Some(plate))?;
        let display_name = rider.full_name();
        let entry = Entry::new(
            shaped.entry_plate.clone(),
            display_name.clone(),
            EntryType::Solo,
            vec![rider],
        );
        let id = entry.id();
        self.entries.push(entry);
        self.log(
            "create_solo_entry",
            json!({"plate": shaped.entry_plate, "name": display_name}),
        );
        Ok(id)
    }

    /// Create a team of 2..=`max_team_size` riders.
    ///
    /// The bulk path is not lock-gated. A `rider_pooled` team adopts its lowest
    /// rider's plate; a `team_relay` team carries the caller's plate and clears
    /// its riders'. A `None` logo card auto-assigns the next unused seeded card.
    ///
    /// # Errors
    ///
    /// Return [`RosterError::SoloOnlyRide`] on a solo-only ride,
    /// [`RosterError::TeamSize`] when the team size is outside its bound,
    /// [`RosterError::PlateShape`] when the plates do not fit the model, and
    /// [`RosterError::DuplicatePlate`] when a plate is already claimed.
    pub fn create_team_entry(&mut self, fields: TeamFields<'_>) -> Result<EntryId, RosterError> {
        if self.entry_mode == EntryMode::Solo {
            return Err(RosterError::SoloOnlyRide);
        }
        let TeamFields {
            display_name,
            mut riders,
            plate,
            logo_card,
        } = fields;
        let size = u32::try_from(riders.len()).unwrap_or(u32::MAX);
        if !(MIN_TEAM_SIZE..=self.max_team_size).contains(&size) {
            return Err(RosterError::TeamSize {
                message: format!(
                    "team size must be between {MIN_TEAM_SIZE} and {}, got {size}",
                    self.max_team_size
                ),
            });
        }
        let shaped = self.shape_and_validate(&mut riders, plate)?;
        let logo = self.resolved_logo_card(logo_card);
        let mut entry = Entry::new(
            shaped.entry_plate.clone(),
            display_name.to_owned(),
            EntryType::Team,
            riders,
        );
        entry.logo_card = logo;
        let id = entry.id();
        self.entries.push(entry);
        self.log(
            "create_team_entry",
            json!({"plate": shaped.entry_plate, "display_name": display_name, "team_size": size}),
        );
        Ok(id)
    }

    /// Create a transient team of exactly one rider, DRAFT only.
    ///
    /// The rider editor adds one rider at a time, so a size-1 team is allowed
    /// while the ride stays in draft; the start-time check defers its size
    /// floor. A `None` logo card auto-assigns the next unused seeded card.
    ///
    /// # Errors
    ///
    /// Return [`RosterError::Locked`] once the ride has left draft,
    /// [`RosterError::SoloOnlyRide`] on a solo-only ride,
    /// [`RosterError::TeamSize`] when the field does not hold exactly one
    /// rider, [`RosterError::PlateShape`] when the plates do not fit the
    /// model, and [`RosterError::DuplicatePlate`] when a plate is already
    /// claimed.
    pub fn create_team_entry_of_one(
        &mut self,
        fields: TeamFields<'_>,
    ) -> Result<EntryId, RosterError> {
        let TeamFields {
            display_name,
            mut riders,
            plate,
            logo_card,
        } = fields;
        self.require_structure_editable()?;
        if self.entry_mode == EntryMode::Solo {
            return Err(RosterError::SoloOnlyRide);
        }
        if riders.len() != 1 {
            return Err(RosterError::TeamSize {
                message: format!(
                    "create_team_entry_of_one takes exactly one rider, got {}",
                    riders.len()
                ),
            });
        }
        let shaped = self.shape_and_validate(&mut riders, plate)?;
        let logo = self.resolved_logo_card(logo_card);
        let mut entry = Entry::new(
            shaped.entry_plate.clone(),
            display_name.to_owned(),
            EntryType::Team,
            riders,
        );
        entry.logo_card = logo;
        let id = entry.id();
        self.entries.push(entry);
        self.log(
            "create_team_entry_of_one",
            json!({"plate": shaped.entry_plate, "display_name": display_name, "team_size": 1}),
        );
        Ok(id)
    }

    /// Create a team with zero riders, DRAFT only.
    ///
    /// A `team_relay` empty team carries its explicit plate, or the next free
    /// one. A `rider_pooled` empty team claims the next free plate
    /// provisionally; a caller-supplied plate is refused because the model
    /// derives the plate from the riders that later join.
    ///
    /// # Errors
    ///
    /// Return [`RosterError::Locked`] once the ride has left draft,
    /// [`RosterError::SoloOnlyRide`] on a solo-only ride,
    /// [`RosterError::TeamSize`] when the field carries riders,
    /// [`RosterError::PlateShape`] when the plate does not fit the model, and
    /// [`RosterError::DuplicatePlate`] when a plate is already claimed.
    pub fn create_empty_team(&mut self, fields: TeamFields<'_>) -> Result<EntryId, RosterError> {
        let TeamFields {
            display_name,
            riders,
            plate,
            logo_card,
        } = fields;
        self.require_structure_editable()?;
        if self.entry_mode == EntryMode::Solo {
            return Err(RosterError::SoloOnlyRide);
        }
        if !riders.is_empty() {
            return Err(RosterError::TeamSize {
                message: format!("create_empty_team takes no riders, got {}", riders.len()),
            });
        }
        let entry_plate = plates::empty_team_plate(&self.entries, self.plate_model, plate)?;
        let logo = self.resolved_logo_card(logo_card);
        let mut entry = Entry::new(
            entry_plate.clone(),
            display_name.to_owned(),
            EntryType::Team,
            Vec::new(),
        );
        entry.logo_card = logo;
        let id = entry.id();
        self.entries.push(entry);
        self.log(
            "create_empty_team",
            json!({"plate": entry_plate, "display_name": display_name, "team_size": 0}),
        );
        Ok(id)
    }

    // ------------------------------------------------------------- editing

    /// Rename an entry and/or edit its notes.
    ///
    /// A retired entry is editable too, because replay may still name it.
    ///
    /// # Errors
    ///
    /// Return [`RosterError::EntryNotFound`] when *id* names no entry here.
    pub fn update_entry(&mut self, id: &EntryId, edit: EntryEdit<'_>) -> Result<(), RosterError> {
        let plate = {
            let Some(entry) = self.entry_mut(id) else {
                return Err(entry_not_found(id));
            };
            if let Some(display_name) = edit.display_name {
                display_name.clone_into(&mut entry.display_name);
            }
            if let Some(notes) = edit.notes {
                notes.clone_into(&mut entry.notes);
            }
            entry.plate.clone()
        };
        let mut payload = serde_json::Map::new();
        payload.insert("plate".to_owned(), Value::String(plate));
        if let Some(display_name) = edit.display_name {
            payload.insert(
                "display_name".to_owned(),
                Value::String(display_name.to_owned()),
            );
        }
        if let Some(notes) = edit.notes {
            payload.insert("notes".to_owned(), Value::String(notes.to_owned()));
        }
        self.log("update_entry", Value::Object(payload));
        Ok(())
    }

    /// Rename one rider, and set or clear their sex.
    ///
    /// A name fix is open in every ride state, because spec S3 keeps it so: the
    /// method never consults the lock matrix. A solo entry mirrors the rider's
    /// new full name into its display name. A retired entry is editable too,
    /// because replay may still name it.
    ///
    /// # Errors
    ///
    /// Return [`RosterError::EntryNotFound`] when *entry* names no entry here,
    /// and [`RosterError::InvalidMove`] when the entry has no rider at *index*.
    #[expect(
        clippy::too_many_arguments,
        reason = "the seam fixes the entry, the rider index and the edit; a parameter struct would force every caller to unpack it"
    )]
    pub fn update_rider(
        &mut self,
        entry: &EntryId,
        index: usize,
        edit: RiderEdit<'_>,
    ) -> Result<(), RosterError> {
        let Some(target) = self.entry_mut(entry) else {
            return Err(entry_not_found(entry));
        };
        let Some(rider) = target.riders.get_mut(index) else {
            return Err(RosterError::InvalidMove {
                message: format!("entry {} has no rider at index {index}", entry.0),
            });
        };
        edit.first_name.clone_into(&mut rider.first_name);
        edit.last_name.clone_into(&mut rider.last_name);
        rider.sex = edit.sex.map(str::to_owned);
        let full_name = rider.full_name();
        if target.kind == EntryType::Solo {
            full_name.clone_into(&mut target.display_name);
        }
        self.log(
            "update_rider",
            json!({"entry_key": entry.0.as_str(), "full_name": full_name}),
        );
        Ok(())
    }

    /// Delete a live entry when the lock matrix allows it.
    ///
    /// A deleted entry is removed outright, never retired, so its plate becomes
    /// free for a later entry.
    ///
    /// # Errors
    ///
    /// Return [`RosterError::EntryNotFound`] when *id* names no live entry, and
    /// [`RosterError::Locked`] when the entry carries recorded data or the ride
    /// has left draft.
    pub fn delete_entry(&mut self, id: &EntryId) -> Result<(), RosterError> {
        let status = self.status;
        let Some(entry) = self.entries.iter().find(|entry| entry.key == id.0) else {
            return Err(entry_not_found(id));
        };
        if !lock::can_delete_entry(status, entry.has_data) {
            return Err(RosterError::Locked {
                message: delete_refusal(status, entry.has_data),
            });
        }
        let plate = entry.plate.clone();
        let display_name = entry.display_name.clone();
        self.entries.retain(|entry| entry.key != id.0);
        self.log(
            "delete_entry",
            json!({"plate": plate, "display_name": display_name}),
        );
        Ok(())
    }

    /// Flag an entry as carrying recorded data (R-15).
    ///
    /// A retired entry is accepted, because replay marks the retired entry that
    /// a pre-move crossing row names.
    ///
    /// # Errors
    ///
    /// Return [`RosterError::EntryNotFound`] when *id* names no entry here.
    pub fn mark_has_data(&mut self, id: &EntryId) -> Result<(), RosterError> {
        let Some(entry) = self.entry_mut(id) else {
            return Err(entry_not_found(id));
        };
        entry.has_data = true;
        let plate = entry.plate.clone();
        self.log("mark_has_data", json!({"plate": plate}));
        Ok(())
    }

    // -------------------------------------------------------- plate changes

    /// Change a solo entry's plate, in either plate model.
    ///
    /// A `rider_pooled` ride mirrors the new plate on the entry's rider, so the
    /// two stay in lock step. The method assigns and audits even when the plate
    /// matches the current value; only [`Self::change_plate`] treats that as a
    /// no-op.
    ///
    /// # Errors
    ///
    /// Return [`RosterError::EntryNotFound`] when *id* names no live entry,
    /// [`RosterError::Locked`] once the ride has left draft,
    /// [`RosterError::InvalidMove`] when the entry is not a solo one,
    /// [`RosterError::PlateShape`] when *plate* is blank or not a whole number,
    /// and [`RosterError::DuplicatePlate`] when another entry or rider claims
    /// it.
    pub fn change_solo_plate(&mut self, id: &EntryId, plate: &str) -> Result<(), RosterError> {
        let status = self.status;
        let model = self.plate_model;
        let in_use = plates::plates_in_use_owned(&self.entries);
        let Some(index) = self.entries.iter().position(|entry| entry.key == id.0) else {
            return Err(entry_not_found(id));
        };
        if !lock::can_edit_structure(status) {
            return Err(plates_locked(status));
        }
        #[expect(
            clippy::indexing_slicing,
            reason = "the index came from the same, unmodified entry vector"
        )]
        let entry = &mut self.entries[index];
        if entry.kind != EntryType::Solo {
            return Err(RosterError::InvalidMove {
                message: "change_solo_plate requires a solo entry".to_owned(),
            });
        }
        plates::require_plate_nonempty(plate)?;
        plates::require_whole_plate(plate)?;
        let old_plate = entry.plate.clone();
        plates::require_plate_change_free(&in_use, plate, &old_plate)?;
        let display_name = entry.display_name.clone();
        plate.clone_into(&mut entry.plate);
        if model == PlateModel::RiderPooled {
            for rider in &mut entry.riders {
                rider.plate = Some(plate.to_owned());
            }
        }
        self.log(
            "change_solo_plate",
            json!({
                "display_name": display_name,
                "old_plate": old_plate,
                "new_plate": plate,
                "reason": format!("{old_plate} → {plate}"),
            }),
        );
        Ok(())
    }

    /// Change one `rider_pooled` team member's own plate (S1, R-20).
    ///
    /// The owning team re-derives its plate from its lowest member afterwards.
    ///
    /// # Errors
    ///
    /// Return [`RosterError::RiderNotFound`] when *`rider_plate`* is on no entry,
    /// [`RosterError::Locked`] once the ride has left draft,
    /// [`RosterError::InvalidMove`] when the ride is not `rider_pooled` or the
    /// rider is not on a team, [`RosterError::PlateShape`] when *plate* is
    /// blank or not a whole number, and [`RosterError::DuplicatePlate`] when
    /// another entry or rider claims it.
    pub fn change_pooled_rider_plate(
        &mut self,
        rider_plate: &str,
        plate: &str,
    ) -> Result<(), RosterError> {
        let status = self.status;
        let model = self.plate_model;
        let in_use = plates::plates_in_use_owned(&self.entries);
        let Some(index) = self.entries.iter().position(|entry| {
            entry
                .riders
                .iter()
                .any(|rider| rider.plate.as_deref() == Some(rider_plate))
        }) else {
            return Err(rider_not_found());
        };
        if !lock::can_edit_structure(status) {
            return Err(plates_locked(status));
        }
        #[expect(
            clippy::indexing_slicing,
            reason = "the index came from the same, unmodified entry vector"
        )]
        let entry = &mut self.entries[index];
        if model != PlateModel::RiderPooled || entry.kind != EntryType::Team {
            return Err(RosterError::InvalidMove {
                message: "change_pooled_rider_plate requires a rider_pooled team member".to_owned(),
            });
        }
        plates::require_plate_nonempty(plate)?;
        plates::require_whole_plate(plate)?;
        let old_plate = entry_rider_plate(entry, rider_plate).unwrap_or_default();
        plates::require_plate_change_free(&in_use, plate, &old_plate)?;
        let rider_name = rename_member(entry, rider_plate, plate);
        plates::recompute_pooled_plate(entry);
        self.log(
            "change_pooled_rider_plate",
            json!({
                "rider_name": rider_name,
                "old_plate": old_plate,
                "new_plate": plate,
                "reason": format!("{old_plate} → {plate}"),
            }),
        );
        Ok(())
    }

    /// Change a `team_relay` team's plate (S1, R-20).
    ///
    /// # Errors
    ///
    /// Return [`RosterError::EntryNotFound`] when *id* names no live entry,
    /// [`RosterError::Locked`] once the ride has left draft,
    /// [`RosterError::InvalidMove`] when the entry is not a team or the ride is
    /// not `team_relay`, [`RosterError::PlateShape`] when *plate* is blank or
    /// not a whole number, and [`RosterError::DuplicatePlate`] when another
    /// entry or rider claims it.
    pub fn change_team_plate(&mut self, id: &EntryId, plate: &str) -> Result<(), RosterError> {
        let status = self.status;
        let model = self.plate_model;
        let in_use = plates::plates_in_use_owned(&self.entries);
        let Some(index) = self.entries.iter().position(|entry| entry.key == id.0) else {
            return Err(entry_not_found(id));
        };
        if !lock::can_edit_structure(status) {
            return Err(plates_locked(status));
        }
        #[expect(
            clippy::indexing_slicing,
            reason = "the index came from the same, unmodified entry vector"
        )]
        let entry = &mut self.entries[index];
        if entry.kind != EntryType::Team {
            return Err(RosterError::InvalidMove {
                message: "change_team_plate requires a team entry; use change_solo_plate"
                    .to_owned(),
            });
        }
        if model != PlateModel::TeamRelay {
            return Err(RosterError::InvalidMove {
                message: "a rider_pooled team's plate is derived from its riders; use change_pooled_rider_plate instead"
                    .to_owned(),
            });
        }
        plates::require_plate_nonempty(plate)?;
        plates::require_whole_plate(plate)?;
        let old_plate = entry.plate.clone();
        plates::require_plate_change_free(&in_use, plate, &old_plate)?;
        let display_name = entry.display_name.clone();
        plate.clone_into(&mut entry.plate);
        self.log(
            "change_team_plate",
            json!({
                "display_name": display_name,
                "old_plate": old_plate,
                "new_plate": plate,
                "reason": format!("{old_plate} → {plate}"),
            }),
        );
        Ok(())
    }

    /// Change a plate through the plate model's own primitive.
    ///
    /// A solo entry goes through [`Self::change_solo_plate`]; a `rider_pooled`
    /// team changes the named member's own plate through
    /// [`Self::change_pooled_rider_plate`]; a `team_relay` team changes the
    /// entry's own plate through [`Self::change_team_plate`]. An unchanged
    /// plate is a no-op here, so an unconditional save never rewrites a plate
    /// or logs a spurious audit event.
    ///
    /// # Errors
    ///
    /// Return [`RosterError::EntryNotFound`] when the entry or the rider's
    /// entry is no member, [`RosterError::RiderNotFound`] when a `rider_pooled`
    /// team names no member, and whatever the dispatched primitive returns.
    pub fn change_plate(&mut self, change: PlateChange<'_>) -> Result<(), RosterError> {
        let Some(kind) = self.entry_by_key(&change.entry.0).map(|entry| entry.kind) else {
            return Err(entry_not_found(change.entry));
        };
        match kind {
            EntryType::Solo => self.change_solo_target(change),
            EntryType::Team => self.change_team_target(change),
        }
    }

    /// Dispatch a solo entry's plate change, skipping an unchanged value.
    fn change_solo_target(&mut self, change: PlateChange<'_>) -> Result<(), RosterError> {
        if change.plate == self.current_plate(change.entry) {
            return Ok(());
        }
        self.change_solo_plate(change.entry, change.plate)
    }

    /// Dispatch a team's plate change by the ride's plate model.
    fn change_team_target(&mut self, change: PlateChange<'_>) -> Result<(), RosterError> {
        if self.plate_model != PlateModel::RiderPooled {
            if change.plate == self.current_plate(change.entry) {
                return Ok(());
            }
            return self.change_team_plate(change.entry, change.plate);
        }
        let Some(rider_plate) = change.rider_plate else {
            return Err(RosterError::RiderNotFound {
                description: "a rider_pooled team plate change requires the member rider"
                    .to_owned(),
            });
        };
        if self.member_plate(change.entry, rider_plate).as_deref() == Some(change.plate) {
            return Ok(());
        }
        self.change_pooled_rider_plate(rider_plate, change.plate)
    }

    /// Return the plate that *id* currently names, or an empty string.
    fn current_plate(&self, id: &EntryId) -> String {
        self.entry_by_key(&id.0)
            .map_or_else(String::new, |entry| entry.plate.clone())
    }

    /// Return the plate that the member *`rider_plate`* currently carries on
    /// *id*'s entry.
    fn member_plate(&self, id: &EntryId, rider_plate: &str) -> Option<String> {
        self.entry_by_key(&id.0).and_then(|entry| {
            entry
                .riders
                .iter()
                .find(|rider| rider.plate.as_deref() == Some(rider_plate))
                .and_then(|rider| rider.plate.clone())
        })
    }

    // ------------------------------------------------------------ helpers

    /// Borrow the live or retired entry that *id* names.
    pub(crate) fn entry_mut(&mut self, id: &EntryId) -> Option<&mut Entry> {
        if let Some(index) = self.entries.iter().position(|entry| entry.key == id.0) {
            return self.entries.get_mut(index);
        }
        self.retired.iter_mut().find(|entry| entry.key == id.0)
    }

    /// Return the live entries as a mutable list, for the moves' member edits.
    ///
    /// The caller keeps each index it took from [`Self::entries`], so it edits
    /// the entry it already validated.
    pub(crate) fn entries_mut(&mut self) -> &mut Vec<Entry> {
        &mut self.entries
    }

    /// Return the retired entries as a mutable list, for a move's destination.
    pub(crate) fn retired_mut(&mut self) -> &mut Vec<Entry> {
        &mut self.retired
    }

    /// Return the seeded logo deck, or an empty deck when the ride has no seed.
    pub(crate) fn logo_deck(&self) -> &[String] {
        &self.logo_codes
    }

    /// Remove a now-empty entry from the live list (E3.2).
    ///
    /// An empty entry has no plate owner, so it ceases to exist. An entry that
    /// carries recorded data is *retired* instead of discarded, so replay can
    /// still resolve the crossings filed under its key; a data-less entry is
    /// dropped outright. The logged action names the entry kind: a team logs
    /// `dissolve_team_entry`, any other entry logs `dissolve_entry`.
    ///
    /// A key that names no live entry does nothing, because a caller dissolves
    /// only an entry it has just emptied.
    pub(crate) fn dissolve_emptied(&mut self, id: &EntryId) {
        let Some(index) = self.entries.iter().position(|entry| entry.key == id.0) else {
            return;
        };
        let entry = self.entries.remove(index);
        let action = if entry.kind == EntryType::Team {
            "dissolve_team_entry"
        } else {
            "dissolve_entry"
        };
        self.log(
            action,
            json!({"plate": entry.plate, "display_name": entry.display_name}),
        );
        if entry.has_data {
            self.retired.push(entry);
        }
    }

    /// Shape the riders'/plate by the model, then refuse a claimed plate.
    fn shape_and_validate(
        &self,
        riders: &mut [Rider],
        plate: Option<&str>,
    ) -> Result<plates::EntryPlates, RosterError> {
        let shaped = plates::shape_entry_plate(self.plate_model, riders, plate)?;
        let claims: Vec<&str> = shaped.claims.iter().map(String::as_str).collect();
        plates::require_plates_free(&self.entries, &claims)?;
        Ok(shaped)
    }

    /// Return *supplied*, else the next unused auto-assigned logo card.
    fn resolved_logo_card(&self, supplied: Option<&str>) -> Option<String> {
        supplied
            .map(str::to_owned)
            .or_else(|| self.next_team_logo_card(None))
    }

    /// Refuse a new team once the ride has left draft.
    fn require_structure_editable(&self) -> Result<(), RosterError> {
        if lock::can_edit_structure(self.status) {
            return Ok(());
        }
        Err(RosterError::Locked {
            message: format!(
                "a new team cannot be started once the ride is {}",
                self.status.as_str()
            ),
        })
    }

    /// Append one audit event naming *action* and its payload.
    pub(crate) fn log(&mut self, action: &str, payload: Value) {
        self.audit.push(AuditEvent {
            action: action.to_owned(),
            payload,
        });
    }
}

/// Check the ride-wide team ceiling (R-12).
fn require_max_team_size(max_team_size: u32) -> Result<(), RosterError> {
    if (MIN_TEAM_SIZE..=MAX_TEAM_SIZE_LIMIT).contains(&max_team_size) {
        return Ok(());
    }
    Err(RosterError::TeamSize {
        message: format!(
            "max_team_size must be between {MIN_TEAM_SIZE} and {MAX_TEAM_SIZE_LIMIT}, got {max_team_size}"
        ),
    })
}

/// Return the seeded logo deck for `team_logo_seed`, or an empty deck.
fn build_logo_codes(team_logo_seed: Option<u64>) -> Vec<String> {
    let Some(seed) = team_logo_seed else {
        return Vec::new();
    };
    seeded_card_codes(seed, 1)
        .iter()
        .map(rivercrossing_cards::Card::code)
        .collect()
}

/// Return the refusal for an entry that *id* does not name.
pub(crate) fn entry_not_found(id: &EntryId) -> RosterError {
    RosterError::EntryNotFound { key: id.0.clone() }
}

/// Return the plate that the member *`rider_plate`* currently carries.
fn entry_rider_plate(entry: &Entry, rider_plate: &str) -> Option<String> {
    entry
        .riders
        .iter()
        .find(|rider| rider.plate.as_deref() == Some(rider_plate))
        .and_then(|rider| rider.plate.clone())
}

/// Assign *plate* to the member *`rider_plate`* names, and return its name.
fn rename_member(entry: &mut Entry, rider_plate: &str, plate: &str) -> String {
    let mut rider_name = String::new();
    for rider in &mut entry.riders {
        if rider.plate.as_deref() == Some(rider_plate) {
            rider_name = rider.full_name();
            rider.plate = Some(plate.to_owned());
        }
    }
    rider_name
}

/// Return the refusal for a rider plate that names no member.
pub(crate) fn rider_not_found() -> RosterError {
    RosterError::RiderNotFound {
        description: "rider is not on any entry in this roster".to_owned(),
    }
}

/// Return the lock refusal for a plate change.
fn plates_locked(status: RideStatus) -> RosterError {
    RosterError::Locked {
        message: format!(
            "plates cannot be changed once the ride is {}",
            status.as_str()
        ),
    }
}

/// Return why the entry is currently undeletable (R-15).
pub(crate) fn delete_refusal(status: RideStatus, has_data: bool) -> String {
    if has_data {
        return "entry has recorded data; DNF or void it instead of deleting".to_owned();
    }
    format!(
        "entries can no longer be deleted once the ride is {}",
        status.as_str()
    )
}

#[cfg(test)]
mod tests;
