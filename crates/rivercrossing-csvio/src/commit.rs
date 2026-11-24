// SPDX-License-Identifier: GPL-3.0-only

//! The atomic CSV import commit: apply a conflict-free preview through the
//! roster's audited mutators, or leave the roster untouched.
//!
//! The commit stages every mutation on a clone of the roster and swaps the
//! clone in only when every mutation succeeds, so a refused import leaves no
//! partial roster and no partial audit trail behind (R-21, R-24). A preview
//! that still holds conflicts, or that was computed against a different roster
//! state, is refused before the first mutation.
//!
//! The two plate models reshape differently: a `team_relay` import matches each
//! entry by its own plate and recreates a team whose composition changed, while
//! a `rider_pooled` import matches each rider by their own plate and reshapes
//! the pooled teams around them. No entry is ever deleted merely because the
//! file omits it.

use rivercrossing_ride::EntryId;
use rivercrossing_roster::enums::{EntryType, PlateModel};
use rivercrossing_roster::roster::{EntryEdit, RiderEdit, Roster, TeamFields};
use rivercrossing_roster::types::{AuditEvent, Entry, Rider};

use crate::error::CsvIoError;
use crate::model::{ImportPreview, ImportReport, ParsedEntry, ParsedRider};
use crate::preview::{fingerprint, relay_composition_changed};

/// The audit reason that a pooled membership change records.
const IMPORT_REASON: &str = "csv import";

/// The rider index of a solo entry's only rider.
const SOLO_RIDER_INDEX: usize = 0;

/// Apply *preview* to *roster* through its audited mutators.
///
/// The commit refuses a preview that still holds conflicts, and a preview that
/// no longer matches the roster it was computed against. Every mutation is
/// staged on a clone, so a refusal at any point leaves *roster* and its audit
/// log exactly as they were.
///
/// # Errors
///
/// Return [`CsvIoError::ConflictsPresent`] when the preview still holds
/// blocking conflicts, [`CsvIoError::StalePreview`] when the roster changed
/// after the preview was computed, and [`CsvIoError::Roster`] when a roster
/// mutator refuses a staged change.
pub fn commit(roster: &mut Roster, preview: &ImportPreview) -> Result<ImportReport, CsvIoError> {
    if !preview.conflicts.is_empty() {
        return Err(CsvIoError::ConflictsPresent {
            count: preview.conflicts.len(),
        });
    }
    if fingerprint(roster) != preview.fingerprint {
        return Err(CsvIoError::StalePreview);
    }
    let mut staged = roster.clone();
    let mark = staged.audit_log().len();
    let counts = apply(&mut staged, preview)?;
    let audit_events: Vec<AuditEvent> = staged.audit_log().iter().skip(mark).cloned().collect();
    *roster = staged;
    let Counts {
        inserted,
        updated,
        moved,
        extracted,
        joined,
    } = counts;
    Ok(ImportReport {
        inserted,
        updated,
        moved,
        extracted,
        joined,
        audit_events,
    })
}

/// Apply every parsed entry under the roster's plate model.
fn apply(roster: &mut Roster, preview: &ImportPreview) -> Result<Counts, CsvIoError> {
    match roster.plate_model() {
        PlateModel::TeamRelay => commit_relay(roster, &preview.entries),
        PlateModel::RiderPooled => commit_pooled(roster, &preview.entries),
    }
}

/// The per-entry and per-rider counts of one commit.
#[derive(Debug, Clone, Copy, Default)]
struct Counts {
    /// The number of inserted entries.
    inserted: usize,
    /// The number of edited entries.
    updated: usize,
    /// The number of riders moved between teams.
    moved: usize,
    /// The number of riders extracted from a team.
    extracted: usize,
    /// The number of riders joined to a team.
    joined: usize,
}

// ------------------------------------------------------------------- relay

/// What the relay commit does with one parsed entry.
enum RelayStep {
    /// Create the entry.
    Insert,
    /// Rename the matched solo rider and update its notes.
    Solo(SoloEdit),
    /// Delete the matched entry and create it afresh.
    Reshape(EntryId),
    /// Apply the sex, name and note edits to the matched team.
    Team(TeamEdits),
}

/// Apply every parsed relay entry: insert, reshape, or rename it.
fn commit_relay(roster: &mut Roster, entries: &[ParsedEntry]) -> Result<Counts, CsvIoError> {
    let mut counts = Counts::default();
    for parsed in entries {
        match relay_step(roster, parsed) {
            RelayStep::Insert => {
                insert_relay_entry(roster, parsed)?;
                counts.inserted += 1;
            }
            RelayStep::Reshape(id) => {
                roster.delete_entry(&id)?;
                insert_relay_entry(roster, parsed)?;
                counts.updated += 1;
            }
            RelayStep::Solo(edit) => {
                if apply_solo_edits(roster, &edit, parsed)? {
                    counts.updated += 1;
                }
            }
            RelayStep::Team(edit) => {
                if apply_team_edits(roster, &edit, parsed)? {
                    counts.updated += 1;
                }
            }
        }
    }
    Ok(counts)
}

/// Return what the relay commit does with *parsed*.
fn relay_step(roster: &Roster, parsed: &ParsedEntry) -> RelayStep {
    let Some(existing) = roster.resolve_plate(&parsed.plate) else {
        return RelayStep::Insert;
    };
    if existing.kind == EntryType::Solo && parsed.kind == EntryType::Solo {
        return RelayStep::Solo(SoloEdit::from_entry(existing, parsed));
    }
    if relay_composition_changed(existing, parsed) {
        return RelayStep::Reshape(existing.id());
    }
    RelayStep::Team(TeamEdits::from_entry(existing, parsed))
}

/// Create *parsed* as a fresh relay entry, keeping its own plate.
///
/// A one-rider team goes through the draft-only one-rider form, because the
/// preview admits that shape as an under-min warning while the operator is
/// still assembling the team.
fn insert_relay_entry(roster: &mut Roster, parsed: &ParsedEntry) -> Result<(), CsvIoError> {
    let id = match parsed.kind {
        EntryType::Solo => roster.create_solo_entry(solo_rider(parsed), &parsed.plate)?,
        EntryType::Team => {
            let riders: Vec<Rider> = parsed.riders.iter().map(relay_member).collect();
            let single = riders.len() == 1;
            let fields = TeamFields {
                display_name: &parsed.display_name,
                riders,
                plate: Some(&parsed.plate),
                logo_card: None,
            };
            if single {
                roster.create_team_entry_of_one(fields)?
            } else {
                roster.create_team_entry(fields)?
            }
        }
    };
    if !parsed.notes.is_empty() {
        roster.update_entry(&id, notes_edit(&parsed.notes))?;
    }
    Ok(())
}

// ------------------------------------------------------------------ pooled

/// Apply every parsed pooled entry: insert, rename, move, or join.
fn commit_pooled(roster: &mut Roster, entries: &[ParsedEntry]) -> Result<Counts, CsvIoError> {
    let mut counts = Counts::default();
    for parsed in entries {
        match parsed.kind {
            EntryType::Solo => commit_pooled_solo(roster, parsed, &mut counts)?,
            EntryType::Team => commit_pooled_team(roster, parsed, &mut counts)?,
        }
    }
    Ok(counts)
}

/// Insert, rename, or extract-to-solo one parsed pooled solo row.
fn commit_pooled_solo(
    roster: &mut Roster,
    parsed: &ParsedEntry,
    counts: &mut Counts,
) -> Result<(), CsvIoError> {
    let Some(owner) = roster.resolve_plate(&parsed.plate) else {
        return insert_pooled_solo(roster, parsed, counts);
    };
    if owner.kind == EntryType::Team {
        let member = owner
            .riders
            .iter()
            .find(|rider| rider.plate.as_deref() == Some(parsed.plate.as_str()))
            .cloned();
        let id = roster.extract_rider_to_solo(&parsed.plate, IMPORT_REASON)?;
        let edit = SoloEdit {
            id,
            rider: rider_differs(member.as_ref(), parsed.riders.first()),
            notes: !parsed.notes.is_empty(),
        };
        if apply_solo_edits(roster, &edit, parsed)? {
            counts.updated += 1;
        }
        counts.extracted += 1;
        return Ok(());
    }
    let edit = SoloEdit::from_entry(owner, parsed);
    if apply_solo_edits(roster, &edit, parsed)? {
        counts.updated += 1;
    }
    Ok(())
}

/// Insert one brand-new pooled solo entry, then set its notes.
fn insert_pooled_solo(
    roster: &mut Roster,
    parsed: &ParsedEntry,
    counts: &mut Counts,
) -> Result<(), CsvIoError> {
    let id = roster.create_solo_entry(solo_rider(parsed), &parsed.plate)?;
    if !parsed.notes.is_empty() {
        roster.update_entry(&id, notes_edit(&parsed.notes))?;
    }
    counts.inserted += 1;
    Ok(())
}

/// Insert a brand-new pooled team, or move its riders onto an existing one.
fn commit_pooled_team(
    roster: &mut Roster,
    parsed: &ParsedEntry,
    counts: &mut Counts,
) -> Result<(), CsvIoError> {
    let Some(edit) = pooled_target(roster, parsed) else {
        return form_pooled_team(roster, parsed, counts);
    };
    let joins = join_plan(roster, &edit.id, parsed);
    let (moved, joined) = apply_joins(roster, &edit.id, joins)?;
    counts.moved += moved;
    counts.joined += joined;
    if apply_team_edits(roster, &edit, parsed)? {
        counts.updated += 1;
    }
    Ok(())
}

/// Return the existing team that most of *parsed*'s members belong to.
///
/// A tie goes to the team first encountered in member order. The returned
/// edits carry the target's name and note differences and no sex edits: a
/// pooled re-import never rewrites an existing member's sex.
fn pooled_target(roster: &Roster, parsed: &ParsedEntry) -> Option<TeamEdits> {
    let mut counts: Vec<(&Entry, usize)> = Vec::new();
    for parsed_rider in &parsed.riders {
        let Some(owner) = roster.resolve_plate(plate_of(parsed_rider)) else {
            continue;
        };
        if owner.kind != EntryType::Team {
            continue;
        }
        match counts.iter_mut().find(|(entry, _)| entry.key == owner.key) {
            Some((_, count)) => *count += 1,
            None => counts.push((owner, 1)),
        }
    }
    let mut best: Option<(&Entry, usize)> = None;
    for candidate in counts {
        if best.is_none_or(|(_, best_count)| candidate.1 > best_count) {
            best = Some(candidate);
        }
    }
    let (entry, _) = best?;
    Some(TeamEdits {
        id: entry.id(),
        sexes: Vec::new(),
        display_name: entry.display_name != parsed.display_name,
        notes: entry.notes != parsed.notes,
    })
}

/// Return the plan for every member of *parsed* against *target*.
fn join_plan(roster: &Roster, target: &EntryId, parsed: &ParsedEntry) -> Vec<Join> {
    parsed
        .riders
        .iter()
        .map(|parsed_rider| join_step(roster, target, parsed_rider))
        .collect()
}

/// Return what the pooled join does with one parsed member.
fn join_step(roster: &Roster, target: &EntryId, parsed: &ParsedRider) -> Join {
    match owner_of(roster, plate_of(parsed)) {
        Owner::Solo(id, rider) => Join::Convert(id, rider),
        Owner::Team(id) if id == *target => Join::Keep,
        Owner::Team(_) => Join::Move(plate_of(parsed).to_owned()),
        Owner::New => Join::Add(rider_from(parsed)),
    }
}

/// Apply every planned join and return the moved and joined rider counts.
fn apply_joins(
    roster: &mut Roster,
    target: &EntryId,
    joins: Vec<Join>,
) -> Result<(usize, usize), CsvIoError> {
    let mut moved = 0;
    let mut joined = 0;
    for join in joins {
        match join {
            Join::Keep => {}
            Join::Move(plate) => {
                roster.move_rider(&plate, target, IMPORT_REASON)?;
                moved += 1;
            }
            Join::Convert(id, rider) => {
                roster.delete_entry(&id)?;
                roster.add_rider_to_team(rider, target)?;
                joined += 1;
            }
            Join::Add(rider) => {
                roster.add_rider_to_team(rider, target)?;
                joined += 1;
            }
        }
    }
    Ok((moved, joined))
}

/// Create a brand-new pooled team from new and converted riders.
fn form_pooled_team(
    roster: &mut Roster,
    parsed: &ParsedEntry,
    counts: &mut Counts,
) -> Result<(), CsvIoError> {
    let riders = converted_members(roster, parsed)?;
    let id = create_pooled_team(roster, parsed, riders)?;
    if !parsed.notes.is_empty() {
        roster.update_entry(&id, notes_edit(&parsed.notes))?;
    }
    counts.inserted += 1;
    Ok(())
}

/// Return the team's riders, dissolving each solo entry that already holds one.
fn converted_members(roster: &mut Roster, parsed: &ParsedEntry) -> Result<Vec<Rider>, CsvIoError> {
    let mut riders = Vec::new();
    for parsed_rider in &parsed.riders {
        match owner_of(roster, plate_of(parsed_rider)) {
            Owner::Solo(id, rider) => {
                roster.delete_entry(&id)?;
                riders.push(rider);
            }
            // A team owner cannot reach this path: its plate would have made
            // that team the target. A fresh rider on a claimed plate then
            // fails the create loudly instead of silently reusing it.
            Owner::New | Owner::Team(_) => riders.push(rider_from(parsed_rider)),
        }
    }
    Ok(riders)
}

/// Create the pooled team, through the one-rider form when it holds one rider.
fn create_pooled_team(
    roster: &mut Roster,
    parsed: &ParsedEntry,
    riders: Vec<Rider>,
) -> Result<EntryId, CsvIoError> {
    let single = riders.len() == 1;
    let fields = TeamFields {
        display_name: &parsed.display_name,
        riders,
        plate: None,
        logo_card: None,
    };
    let id = if single {
        roster.create_team_entry_of_one(fields)?
    } else {
        roster.create_team_entry(fields)?
    };
    Ok(id)
}

// ----------------------------------------------------------- apply edits

/// Apply a matched solo's rider rename and note edit, and return whether any
/// field changed.
fn apply_solo_edits(
    roster: &mut Roster,
    edit: &SoloEdit,
    parsed: &ParsedEntry,
) -> Result<bool, CsvIoError> {
    if edit.rider {
        roster.update_rider(&edit.id, SOLO_RIDER_INDEX, solo_rider_edit(parsed))?;
    }
    if edit.notes {
        roster.update_entry(&edit.id, notes_edit(&parsed.notes))?;
    }
    Ok(edit.changed())
}

/// Apply a matched team's sex, name and note edits, and return whether any
/// field changed.
fn apply_team_edits(
    roster: &mut Roster,
    edit: &TeamEdits,
    parsed: &ParsedEntry,
) -> Result<bool, CsvIoError> {
    for sex in &edit.sexes {
        roster.update_rider(&edit.id, sex.index, sex.rider_edit())?;
    }
    if edit.display_name || edit.notes {
        roster.update_entry(
            &edit.id,
            EntryEdit {
                display_name: edit.display_name.then_some(parsed.display_name.as_str()),
                notes: edit.notes.then_some(parsed.notes.as_str()),
            },
        )?;
    }
    Ok(edit.changed())
}

// ------------------------------------------------------------------ plans

/// The rider-rename and note edits that a matched solo row asks for.
struct SoloEdit {
    /// The matched solo entry.
    id: EntryId,
    /// Whether the rider's names or sex differ.
    rider: bool,
    /// Whether the notes differ.
    notes: bool,
}

impl SoloEdit {
    /// Build the edit that a matched solo entry needs.
    fn from_entry(entry: &Entry, parsed: &ParsedEntry) -> Self {
        Self {
            id: entry.id(),
            rider: rider_differs(entry.riders.first(), parsed.riders.first()),
            notes: entry.notes != parsed.notes,
        }
    }

    /// Return whether the edit changes any field.
    fn changed(&self) -> bool {
        self.rider || self.notes
    }
}

/// One matched rider's sex edit.
struct SexEdit {
    /// The rider's index on the entry.
    index: usize,
    /// The rider's current first name.
    first_name: String,
    /// The rider's current last name.
    last_name: String,
    /// The sex that the file names.
    sex: Option<String>,
}

impl SexEdit {
    /// Return the rider edit that sets this rider's sex.
    fn rider_edit(&self) -> RiderEdit<'_> {
        RiderEdit {
            first_name: &self.first_name,
            last_name: &self.last_name,
            sex: self.sex.as_deref(),
        }
    }
}

/// The sex, name and note edits that a matched team asks for.
struct TeamEdits {
    /// The matched entry.
    id: EntryId,
    /// The riders whose sex differs, in rider order.
    sexes: Vec<SexEdit>,
    /// Whether the display name differs.
    display_name: bool,
    /// Whether the notes differ.
    notes: bool,
}

impl TeamEdits {
    /// Build the edit that a matched team needs.
    fn from_entry(entry: &Entry, parsed: &ParsedEntry) -> Self {
        let sexes = entry
            .riders
            .iter()
            .zip(&parsed.riders)
            .enumerate()
            .filter(|(_, (rider, parsed_rider))| rider.sex != parsed_rider.sex)
            .map(|(index, (rider, parsed_rider))| SexEdit {
                index,
                first_name: rider.first_name.clone(),
                last_name: rider.last_name.clone(),
                sex: parsed_rider.sex.clone(),
            })
            .collect();
        Self {
            id: entry.id(),
            sexes,
            display_name: entry.display_name != parsed.display_name,
            notes: entry.notes != parsed.notes,
        }
    }

    /// Return whether the edit changes any field.
    fn changed(&self) -> bool {
        !self.sexes.is_empty() || self.display_name || self.notes
    }
}

/// The live owner of one parsed pooled member's plate.
enum Owner {
    /// No entry holds the plate.
    New,
    /// A team holds the plate.
    Team(EntryId),
    /// A solo entry holds the plate, with its only rider.
    Solo(EntryId, Rider),
}

/// Return the live owner of *plate*, when one exists.
fn owner_of(roster: &Roster, plate: &str) -> Owner {
    let Some(entry) = roster.resolve_plate(plate) else {
        return Owner::New;
    };
    if entry.kind == EntryType::Team {
        return Owner::Team(entry.id());
    }
    Owner::Solo(
        entry.id(),
        entry.riders.first().cloned().unwrap_or_default(),
    )
}

/// What the pooled join does with one parsed member.
enum Join {
    /// The member already belongs to the target team.
    Keep,
    /// Move the member from another team, named by their plate.
    Move(String),
    /// Delete the member's solo entry, then add the rider to the target.
    Convert(EntryId, Rider),
    /// Add a rider that no entry holds.
    Add(Rider),
}

// ---------------------------------------------------------------- helpers

/// Return whether the roster rider and the parsed rider differ.
///
/// The preview builds every solo entry around exactly one rider, so the
/// missing-rider fallback is unreachable; it keeps the commit panic-free.
fn rider_differs(existing: Option<&Rider>, parsed: Option<&ParsedRider>) -> bool {
    existing.zip(parsed).is_none_or(|(existing, parsed)| {
        existing.first_name != parsed.first_name
            || existing.last_name != parsed.last_name
            || existing.sex != parsed.sex
    })
}

/// Return *parsed*'s own plate, or an empty string when it names none.
fn plate_of(parsed: &ParsedRider) -> &str {
    parsed.plate.as_deref().unwrap_or_default()
}

/// Build a roster rider from a parsed rider, keeping the parsed plate.
fn rider_from(parsed: &ParsedRider) -> Rider {
    Rider {
        first_name: parsed.first_name.clone(),
        last_name: parsed.last_name.clone(),
        plate: parsed.plate.clone(),
        sex: parsed.sex.clone(),
        ..Rider::default()
    }
}

/// Build a relay team member: the parsed rider without a plate.
fn relay_member(parsed: &ParsedRider) -> Rider {
    Rider {
        plate: None,
        ..rider_from(parsed)
    }
}

/// Return the one rider of a parsed solo entry.
///
/// The preview builds every solo entry around exactly one rider, so the
/// empty-rider fallback is unreachable; it keeps the commit panic-free.
fn solo_rider(parsed: &ParsedEntry) -> Rider {
    parsed
        .riders
        .first()
        .map_or_else(Rider::default, rider_from)
}

/// Return the rider edit that applies *parsed*'s solo rider fields.
///
/// The preview builds every solo entry around exactly one rider, so the
/// blank-name fallback is unreachable; it keeps the commit panic-free.
fn solo_rider_edit(parsed: &ParsedEntry) -> RiderEdit<'_> {
    let rider = parsed.riders.first();
    RiderEdit {
        first_name: rider.map_or("", |rider| rider.first_name.as_str()),
        last_name: rider.map_or("", |rider| rider.last_name.as_str()),
        sex: rider.and_then(|rider| rider.sex.as_deref()),
    }
}

/// Return the entry edit that sets an entry's notes.
fn notes_edit(notes: &str) -> EntryEdit<'_> {
    EntryEdit {
        display_name: None,
        notes: Some(notes),
    }
}

#[cfg(test)]
mod tests;
