// SPDX-License-Identifier: GPL-3.0-only

//! The rider editor's decisions and the CSV import/export wiring.
//!
//! The rules and the copy follow the Python oracle's
//! `ui/presenters/riders.py`. The module holds no view: every function takes
//! the roster (and, for a live team change, the ride engine) and returns the
//! value the view renders or the refusal it shows.
//!
//! A rider's plate is shaped by the ride's plate model. `team_relay` riders
//! carry no plate of their own, so a relay team change never rewrites the
//! destination's plate; every other plate write goes through the roster's own
//! [`Roster::change_plate`] dispatch, so the shape rule has one home.

use std::path::Path;

use rivercrossing_csvio as csvio;
use rivercrossing_ride::{EntryId, PlateModel, RideEngine, RideStatus, SystemClock};
use rivercrossing_roster::enums::EntryType;
use rivercrossing_roster::error::RosterError;
use rivercrossing_roster::names::canonical_person_name;
use rivercrossing_roster::roster::{PlateChange, RiderEdit, Roster};
use rivercrossing_roster::types::{Entry, Rider};
use rivercrossing_standings::types::Placed;

#[cfg(test)]
mod tests;

/// The refusal for a form that leaves either name blank (the oracle's copy).
const NAMES_REQUIRED: &str = "First name and last name are required";

/// The prefix of the audit reason an editor-driven move records.
const MOVE_REASON_PREFIX: &str = "Rider Editor: moved rider to";

/// The audit reason that the roster-only writes record; the roster ignores it.
const MOVE_REASON: &str = "rider editor: move";

/// The team choice that names a solo entry.
const SOLO_CHOICE: &str = "solo";

/// The Team cell that a solo row renders (the oracle's `SOLO_TEAM_TEXT`).
const SOLO_TEAM_CELL: &str = "solo";

/// The rider editor's team choice: solo, or one existing team by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TeamChoice {
    /// No team: the rider is a solo entry.
    Solo,
    /// The display name of the team that the rider joins.
    Team(String),
}

/// The Add/Edit rider dialog's form values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RiderFormValues {
    /// The plate field.
    pub(crate) plate: String,
    /// The first-name field.
    pub(crate) first_name: String,
    /// The last-name field.
    pub(crate) last_name: String,
    /// The team choice.
    pub(crate) team: TeamChoice,
    /// The sex choice; `None` is the dialog's blank item.
    pub(crate) sex: Option<String>,
}

/// One rider-editor row: the Plate, Name and Team cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RiderRow {
    /// The Plate cell, shaped by the ride's plate model.
    pub(crate) plate: String,
    /// The rider's full name.
    pub(crate) name: String,
    /// The Team cell: the team's name, or [`SOLO_TEAM_CELL`].
    pub(crate) team: String,
}

/// The CSV preview dialog's view model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CsvPreviewModel {
    /// The one-line summary of the previewed file.
    pub(crate) summary: String,
    /// The blocking conflicts, in file order.
    pub(crate) conflicts: Vec<String>,
    /// Whether the Import button is enabled.
    pub(crate) can_import: bool,
}

// ---------------------------------------------------------------- create

/// Create *form*'s entry, or refuse with a message.
///
/// A solo choice creates a solo entry; a team choice folds the new rider onto
/// the existing team through [`Roster::add_rider_to_team`], so the team-size
/// ceiling refuses before anything changes. Both names land in their
/// canonical display case.
///
/// # Errors
///
/// Return the names-required message for a blank first or last name, the
/// no-such-team message when the choice names no live team, and the roster's
/// own refusal (a duplicate plate, a full team, a solo-only ride) otherwise.
pub(crate) fn add_rider(roster: &mut Roster, form: &RiderFormValues) -> Result<EntryId, String> {
    require_names(form)?;
    let rider = Rider {
        first_name: canonical_person_name(&form.first_name),
        last_name: canonical_person_name(&form.last_name),
        plate: Some(form.plate.clone()),
        sex: form.sex.clone(),
        ..Rider::default()
    };
    match &form.team {
        TeamChoice::Solo => roster
            .create_solo_entry(rider, &form.plate)
            .map_err(|error| error.to_string()),
        TeamChoice::Team(name) => {
            let target = find_team(roster, name)?;
            roster
                .add_rider_to_team(rider, &target)
                .map_err(|error| error.to_string())?;
            Ok(target)
        }
    }
}

/// Apply *form* to the rider at `rider_index` on *entry*.
///
/// The team step runs first, because a move changes which entry owns the
/// plate; the plate and the names follow. A relay team change skips the plate
/// step, because the plate belongs to the destination entry. A refusal from an
/// earlier step leaves the later ones untouched.
///
/// # Errors
///
/// Return the names-required message for a blank name, the no-such-team
/// message when the choice names no live team, and whatever the roster's own
/// mutators refuse (a duplicate plate, a full team, a locked ride).
#[expect(
    clippy::too_many_arguments,
    reason = "the write-back fixes the roster, the record, the rider index and the form; a parameter struct would force every caller to unpack it"
)]
pub(crate) fn edit_rider(
    roster: &mut Roster,
    entry: &EntryId,
    rider_index: usize,
    form: &RiderFormValues,
) -> Result<(), String> {
    require_names(form)?;
    let (kind, display_name, rider_plate) = entry_facts(roster, entry, rider_index)?;
    let current_team = (kind == EntryType::Team).then_some(display_name.as_str());
    let team_changed = !team_matches(&form.team, current_team);
    let target = if team_changed {
        apply_team_change(
            roster,
            entry,
            rider_index,
            rider_plate.as_deref(),
            &form.team,
        )?
    } else {
        entry.clone()
    };
    let index = if team_changed {
        last_rider_index(roster, &target)
    } else {
        rider_index
    };
    apply_rider_fields(
        roster,
        &target,
        index,
        rider_plate.as_deref(),
        form,
        team_changed,
    )
}

/// Return whether *form*'s team change is the live engine's to make.
///
/// Two conditions, both required: the form names a different team than
/// `current_team` (the entry's team name, or `None` for a solo), and the ride
/// is RUNNING or REOPENED. DRAFT and FINISHED keep the roster-only write-back.
#[must_use]
pub(crate) fn is_live_move(
    engine: &RideEngine<Roster, SystemClock>,
    form: &RiderFormValues,
    current_team: Option<&str>,
) -> bool {
    let live = matches!(engine.state(), RideStatus::Running | RideStatus::Reopened);
    live && !team_matches(&form.team, current_team)
}

/// Move `rider_plate` through *engine*, behind the view's own confirmation.
///
/// A RUNNING ride whose clock still runs is refused with the engine's own
/// instruction before anything changes. On acceptance the engine performs the
/// pooled move, so the rider's laps and cards follow them. The view asks the
/// operator first; this function only refuses and moves.
///
/// # Errors
///
/// Return `Stop the ride first` for a running, unstopped ride, and the
/// engine's own refusal (an unknown team, a non-pooled ride, a blank reason)
/// otherwise.
pub(crate) fn commit_live_move(
    engine: &mut RideEngine<Roster, SystemClock>,
    rider_plate: &str,
    form: &RiderFormValues,
) -> Result<String, String> {
    if engine.state() == RideStatus::Running && !engine.stopped() {
        return Err("Stop the ride first".to_owned());
    }
    let reason = move_reason(&form.team);
    match &form.team {
        TeamChoice::Solo => engine
            .extract_rider_to_solo(rider_plate, &reason)
            .map_err(|error| error.to_string())?,
        TeamChoice::Team(name) => engine
            .move_rider(rider_plate, name, &reason)
            .map_err(|error| error.to_string())?,
    };
    Ok(reason)
}

// ------------------------------------------------------------------ rows

/// Return every `(entry, rider)` row, in the roster's own order.
#[must_use]
pub(crate) fn rows(roster: &Roster) -> Vec<RiderRow> {
    let relay = roster.plate_model() == PlateModel::TeamRelay;
    roster
        .entries()
        .iter()
        .flat_map(|entry| {
            entry.riders.iter().map(move |rider| RiderRow {
                plate: if relay {
                    entry.plate.clone()
                } else {
                    rider.plate.clone().unwrap_or_default()
                },
                name: rider.full_name(),
                team: team_cell(entry),
            })
        })
        .collect()
}

/// Narrow *rows* to the rows whose Plate, Name or Team cell holds *search*.
///
/// The match is a case-insensitive substring over the three cells; a blank
/// search keeps every row. The survivors keep the given order.
#[must_use]
pub(crate) fn filter_rows(rows: &[RiderRow], search: &str) -> Vec<RiderRow> {
    let needle = search.trim().to_lowercase();
    if needle.is_empty() {
        return rows.to_vec();
    }
    rows.iter()
        .filter(|row| row_matches(row, &needle))
        .cloned()
        .collect()
}

// ---------------------------------------------------------------- delete

/// Return the delete confirmation for *entry*.
///
/// The oracle names the rider, not the entry. A solo entry's display name is
/// its rider's full name, so a one-rider entry names that rider; a
/// multi-rider team has no single rider to name and names the team instead.
#[must_use]
pub(crate) fn delete_message(entry: &Entry) -> String {
    let name = match entry.riders.as_slice() {
        [rider] => rider.full_name(),
        _ => entry.display_name.clone(),
    };
    format!("Delete \"{name}\" from this ride?")
}

/// Remove the rider at `rider_index` from *entry*.
///
/// A solo entry goes entirely; a team loses only that member and survives with
/// the riders left.
///
/// # Errors
///
/// Return the roster's own refusal: an unknown entry, an index past the
/// team, or the delete lock (recorded data, or a ride past DRAFT).
pub(crate) fn delete_rider(
    roster: &mut Roster,
    entry: &EntryId,
    rider_index: usize,
) -> Result<(), String> {
    roster
        .remove_rider(entry, rider_index)
        .map_err(|error| error.to_string())
}

// ------------------------------------------------------------------- csv

/// Preview *path* against *roster* and build the dialog's view model.
///
/// Nothing is written. A conflict disables Import; a warning only appends its
/// count to the summary.
///
/// # Errors
///
/// Return the could-not-read message when *path* cannot be read, or the CSV
/// reader's own refusal (a non-`UTF-8` or malformed payload).
pub(crate) fn csv_preview(
    path: &Path,
    roster: &Roster,
    options: csvio::ImportOptions,
) -> Result<(csvio::ImportPreview, CsvPreviewModel), String> {
    let preview = csvio::preview(path, roster, options)
        .map_err(|error| format!("Could not read {}: {error}", file_label(path)))?;
    let conflicts: Vec<String> = preview
        .conflicts
        .iter()
        .map(|problem| problem.message.clone())
        .collect();
    let summary = preview_summary(path, &preview, conflicts.len());
    let model = CsvPreviewModel {
        summary,
        can_import: conflicts.is_empty(),
        conflicts,
    };
    Ok((preview, model))
}

/// Commit a previewed import onto *roster*.
///
/// # Errors
///
/// Return the CSV crate's own refusal: a preview that still holds conflicts, a
/// preview that no longer matches the roster, or a roster mutation that the
/// ride's state now locks.
pub(crate) fn csv_import(
    roster: &mut Roster,
    preview: &csvio::ImportPreview,
) -> Result<csvio::ImportReport, String> {
    csvio::commit(roster, preview).map_err(|error| error.to_string())
}

/// Export *roster* to *path* as CSV, with the optional finished-ride stats.
///
/// # Errors
///
/// Return the CSV crate's own refusal: a stats row that names no entry, or a
/// file that cannot be written.
pub(crate) fn csv_export(
    roster: &Roster,
    path: &Path,
    placed: Option<&[Placed]>,
) -> Result<(), String> {
    csvio::export(roster, path, placed).map_err(|error| error.to_string())
}

// --------------------------------------------------------------- helpers

/// Refuse a form that leaves either name blank.
fn require_names(form: &RiderFormValues) -> Result<(), String> {
    if form.first_name.trim().is_empty() || form.last_name.trim().is_empty() {
        return Err(NAMES_REQUIRED.to_owned());
    }
    Ok(())
}

/// Return whether *choice* names the rider's current team.
fn team_matches(choice: &TeamChoice, current: Option<&str>) -> bool {
    match (choice, current) {
        (TeamChoice::Solo, None) => true,
        (TeamChoice::Team(name), Some(team)) => name == team,
        _ => false,
    }
}

/// Return the entry's kind, display name and the rider's own plate.
fn entry_facts(
    roster: &Roster,
    entry: &EntryId,
    rider_index: usize,
) -> Result<(EntryType, String, Option<String>), String> {
    let Some(target) = roster.entry_by_key(&entry.0) else {
        return Err(entry_not_found(entry));
    };
    let Some(rider) = target.riders.get(rider_index) else {
        return Err(rider_missing(entry, rider_index));
    };
    Ok((
        target.kind,
        target.display_name.clone(),
        rider.plate.clone(),
    ))
}

/// Return the id of the live team whose display name is *name*.
fn find_team(roster: &Roster, name: &str) -> Result<EntryId, String> {
    roster
        .entries()
        .iter()
        .find(|entry| entry.kind == EntryType::Team && entry.display_name == name)
        .map(Entry::id)
        .ok_or_else(|| format!("no team named \"{name}\""))
}

/// Apply the form's team choice to the rider, and return the entry that now
/// owns them.
#[expect(
    clippy::too_many_arguments,
    reason = "the team step fixes the roster, the record, the rider index, that rider's plate and the choice; a parameter struct would add a type with one caller"
)]
fn apply_team_change(
    roster: &mut Roster,
    entry: &EntryId,
    rider_index: usize,
    rider_plate: Option<&str>,
    choice: &TeamChoice,
) -> Result<EntryId, String> {
    match choice {
        TeamChoice::Solo => extract_to_solo(roster, entry, rider_plate),
        TeamChoice::Team(name) => join_team(roster, entry, rider_index, name),
    }
}

/// Move a team member into their own solo entry, or keep a solo entry.
fn extract_to_solo(
    roster: &mut Roster,
    entry: &EntryId,
    rider_plate: Option<&str>,
) -> Result<EntryId, String> {
    if kind_of(roster, entry) == Some(EntryType::Solo) {
        return Ok(entry.clone());
    }
    roster
        .extract_rider_to_solo(rider_plate.unwrap_or_default(), MOVE_REASON)
        .map_err(|error| error.to_string())
}

/// Move a team member onto another team, or fold a solo rider onto a team.
#[expect(
    clippy::too_many_arguments,
    reason = "the join fixes the roster, the source entry, the rider index and the destination name; a parameter struct would add a type with one caller"
)]
fn join_team(
    roster: &mut Roster,
    entry: &EntryId,
    rider_index: usize,
    name: &str,
) -> Result<EntryId, String> {
    let target = find_team(roster, name)?;
    if kind_of(roster, entry) == Some(EntryType::Team) {
        roster
            .move_rider_at(entry, rider_index, &target, MOVE_REASON)
            .map_err(|error| error.to_string())?;
        return Ok(target);
    }
    let rider = roster
        .entry_by_key(&entry.0)
        .and_then(|source| source.riders.get(rider_index).cloned())
        .ok_or_else(|| rider_missing(entry, rider_index))?;
    require_team_room(roster, &target)?;
    roster
        .delete_entry(entry)
        .map_err(|error| error.to_string())?;
    roster
        .add_rider_to_team(rider, &target)
        .map_err(|error| error.to_string())?;
    Ok(target)
}

/// Refuse a join that would exceed the destination team's ceiling.
///
/// The check runs before a solo entry is deleted, so a refused join can never
/// strand the rider.
fn require_team_room(roster: &Roster, target: &EntryId) -> Result<(), String> {
    let size = roster
        .entry_by_key(&target.0)
        .map_or(0, |entry| entry.riders.len());
    let next_size = u32::try_from(size + 1).unwrap_or(u32::MAX);
    if next_size > roster.max_team_size() {
        return Err(format!(
            "team size must be at most {}, got {next_size}",
            roster.max_team_size()
        ));
    }
    Ok(())
}

/// Write the form's plate and names onto the entry that now owns the rider.
#[expect(
    clippy::too_many_arguments,
    reason = "the write-back fixes the roster, the entry, the rider index, the plate and the form that supplies the names and sex"
)]
fn apply_rider_fields(
    roster: &mut Roster,
    entry: &EntryId,
    rider_index: usize,
    rider_plate: Option<&str>,
    form: &RiderFormValues,
    team_changed: bool,
) -> Result<(), String> {
    let skip_plate = team_changed && roster.plate_model() == PlateModel::TeamRelay;
    if !skip_plate {
        let change = PlateChange {
            entry,
            rider_plate,
            plate: &form.plate,
        };
        roster
            .change_plate(change)
            .map_err(|error| error.to_string())?;
    }
    let first = canonical_person_name(&form.first_name);
    let last = canonical_person_name(&form.last_name);
    roster
        .update_rider(
            entry,
            rider_index,
            RiderEdit {
                first_name: &first,
                last_name: &last,
                sex: form.sex.as_deref(),
            },
        )
        .map_err(|error| error.to_string())
}

/// Return the entry's kind, when it is a member of the roster.
fn kind_of(roster: &Roster, entry: &EntryId) -> Option<EntryType> {
    roster.entry_by_key(&entry.0).map(|target| target.kind)
}

/// Return the index of the final rider on *entry*, where a move lands.
fn last_rider_index(roster: &Roster, entry: &EntryId) -> usize {
    roster
        .entry_by_key(&entry.0)
        .map_or(0, |target| target.riders.len().saturating_sub(1))
}

/// Return the refusal for an entry that *id* does not name.
fn entry_not_found(entry: &EntryId) -> String {
    RosterError::EntryNotFound {
        key: entry.0.clone(),
    }
    .to_string()
}

/// Return the refusal for an entry that holds no rider at `rider_index`.
fn rider_missing(entry: &EntryId, rider_index: usize) -> String {
    RosterError::InvalidMove {
        message: format!("entry {} has no rider at index {rider_index}", entry.0),
    }
    .to_string()
}

/// Return the audit reason that an editor-driven move records.
fn move_reason(choice: &TeamChoice) -> String {
    let chosen = match choice {
        TeamChoice::Solo => SOLO_CHOICE,
        TeamChoice::Team(name) => name,
    };
    format!("{MOVE_REASON_PREFIX} {chosen}")
}

/// Return one row's Team cell: the team's name, or the solo text.
fn team_cell(entry: &Entry) -> String {
    if entry.kind == EntryType::Team {
        return entry.display_name.clone();
    }
    SOLO_TEAM_CELL.to_owned()
}

/// Return whether one row holds *needle* in any of its three cells.
fn row_matches(row: &RiderRow, needle: &str) -> bool {
    row.plate.to_lowercase().contains(needle)
        || row.name.to_lowercase().contains(needle)
        || row.team.to_lowercase().contains(needle)
}

/// Return the preview's one-line summary.
fn preview_summary(path: &Path, preview: &csvio::ImportPreview, conflicts: usize) -> String {
    let summary = format!(
        "{} → {} riders · {} teams · {conflicts} conflicts",
        file_label(path),
        preview.riders,
        preview.teams
    );
    if preview.warnings.is_empty() {
        return summary;
    }
    format!("{summary} · {} warnings", preview.warnings.len())
}

/// Return *path*'s final component, or its full rendering.
fn file_label(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}
