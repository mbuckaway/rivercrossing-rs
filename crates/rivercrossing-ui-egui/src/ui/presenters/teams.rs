// SPDX-License-Identifier: GPL-3.0-only

//! The teams editor's decisions: the add/edit form, the logo pick and the
//! remove confirmation.
//!
//! The rules and the copy follow the Python oracle's
//! `ui/presenters/teams.py`. A team name is unique per ride under the roster's
//! own name key (trimmed, whitespace-collapsed, case-folded), so
//! `"Trail Blazers"` and `" trail blazers "` name the same team.

use rivercrossing_ride::EntryId;
use rivercrossing_roster::enums::{EntryType, PlateModel};
use rivercrossing_roster::error::RosterError;
use rivercrossing_roster::names::rider_name_key;
use rivercrossing_roster::roster::{EntryEdit, Roster, TeamFields};
use rivercrossing_roster::types::Entry;

#[cfg(test)]
mod tests;

/// The refusal for a blank team name (the oracle's copy).
const NAME_REQUIRED: &str = "enter a team name";

/// The refusal for a ride with no seeded card deck.
const NO_DECK: &str = "no card deck is available for this ride";

/// The refusal for a deck whose every card a team already claims.
const DECK_EXHAUSTED: &str = "every card logo is already in use by a team";

/// The Add/Edit team dialog's form values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TeamFormValues {
    /// The team-name field.
    pub(crate) name: String,
    /// The relay-plate field; ignored on a rider-pooled ride.
    pub(crate) relay_plate: String,
    /// The notes field.
    pub(crate) notes: String,
}

/// One teams-editor row: the team's name, its plate and its rider count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TeamRow {
    /// The team's display name.
    pub(crate) name: String,
    /// The plate that the team currently carries.
    pub(crate) plate: String,
    /// The team's current rider count.
    pub(crate) members: usize,
}

// ---------------------------------------------------------------- create

/// Create a zero-rider team from *form*.
///
/// A relay ride stores the form's whole-number plate; a pooled ride's empty
/// team claims the next free plate provisionally. A `None` logo card
/// auto-assigns the next unused seeded card. The notes are stored only when
/// the form supplies one.
///
/// # Errors
///
/// Return the names-required message for a blank name, the duplicate message
/// for a name another team already claims, and the roster's own refusal (a
/// solo-only ride, a ride past DRAFT, a blank or duplicate relay plate).
pub(crate) fn add_team(roster: &mut Roster, form: &TeamFormValues) -> Result<EntryId, String> {
    let name = form.name.trim();
    require_name(name)?;
    require_unique_name(roster, name, None)?;
    let relay_plate =
        (roster.plate_model() == PlateModel::TeamRelay).then_some(form.relay_plate.as_str());
    let id = roster
        .create_empty_team(TeamFields {
            display_name: name,
            riders: Vec::new(),
            plate: relay_plate,
            logo_card: None,
        })
        .map_err(|error| error.to_string())?;
    store_notes(roster, &id, &form.notes)?;
    Ok(id)
}

/// Store the form on the team that *entry* names.
///
/// Only the fields that actually changed are written, so an untouched form
/// commits without an audit event.
///
/// # Errors
///
/// Return the names-required message for a blank name, the duplicate message
/// for another team's name, and the roster's own refusal (an unknown entry, a
/// duplicate relay plate, a ride past DRAFT).
pub(crate) fn edit_team(
    roster: &mut Roster,
    entry: &EntryId,
    form: &TeamFormValues,
) -> Result<(), String> {
    let name = form.name.trim();
    require_name(name)?;
    require_unique_name(roster, name, Some(entry))?;
    let Some(current) = roster.entry_by_key(&entry.0) else {
        return Err(entry_not_found(entry));
    };
    let display_name = current.display_name.clone();
    let notes = current.notes.clone();
    let plate = current.plate.clone();
    if roster.plate_model() == PlateModel::TeamRelay && form.relay_plate != plate {
        roster
            .change_team_plate(entry, &form.relay_plate)
            .map_err(|error| error.to_string())?;
    }
    let mut edit = EntryEdit::default();
    let mut changed = false;
    if name != display_name {
        edit.display_name = Some(name);
        changed = true;
    }
    if form.notes != notes {
        edit.notes = Some(&form.notes);
        changed = true;
    }
    if changed {
        roster
            .update_entry(entry, edit)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

// ------------------------------------------------------------- pick card

/// Draw a free seeded logo card for the team that *entry* names, and store it.
///
/// The pick excludes every card another team holds and the card the entry
/// already shows, so the preview always changes. *seed* chooses the pool
/// index, so the same seed is deterministic.
///
/// # Errors
///
/// Return the no-deck message when the ride has no logo seed, the exhausted
/// message when every card is claimed, and the roster's refusal when *entry*
/// names no entry.
pub(crate) fn pick_card(roster: &mut Roster, entry: &EntryId, seed: u64) -> Result<String, String> {
    let exclude = roster
        .entry_by_key(&entry.0)
        .and_then(|target| target.logo_card.clone());
    let slug = usize::try_from(seed).unwrap_or(usize::MAX);
    let Some(code) = roster.random_team_card(exclude.as_deref(), |pool| slug % pool) else {
        return Err(logo_pick_refusal(roster).to_owned());
    };
    roster
        .set_team_logo_card(entry, &code)
        .map_err(|error| error.to_string())?;
    Ok(code)
}

// ------------------------------------------------------------------ rows

/// Return every team row, in creation order.
///
/// `single_member_only` hides every team that has more than one rider; a solo
/// entry is never a row.
#[must_use]
pub(crate) fn rows(roster: &Roster, single_member_only: bool) -> Vec<TeamRow> {
    roster
        .entries()
        .iter()
        .filter(|entry| entry.kind == EntryType::Team)
        .filter(|entry| !single_member_only || entry.team_size() == 1)
        .map(|entry| TeamRow {
            name: entry.display_name.clone(),
            plate: entry.plate.clone(),
            members: entry.team_size(),
        })
        .collect()
}

// ---------------------------------------------------------------- remove

/// Return the remove confirmation for *entry*.
#[must_use]
pub(crate) fn remove_message(entry: &Entry) -> String {
    format!("Remove the team \"{}\"?", entry.display_name)
}

/// Delete the team that *entry* names.
///
/// # Errors
///
/// Return the roster's own refusal: an unknown entry, or the delete lock
/// (recorded data, or a ride past DRAFT).
pub(crate) fn remove_team(roster: &mut Roster, entry: &EntryId) -> Result<(), String> {
    roster
        .delete_entry(entry)
        .map_err(|error| error.to_string())
}

// --------------------------------------------------------------- helpers

/// Refuse a blank team name.
fn require_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err(NAME_REQUIRED.to_owned());
    }
    Ok(())
}

/// Refuse a name that another team already claims.
///
/// *exclude* is the entry being renamed, which never collides with itself.
fn require_unique_name(
    roster: &Roster,
    name: &str,
    exclude: Option<&EntryId>,
) -> Result<(), String> {
    if duplicate_team_entry(roster, name, exclude).is_some() {
        return Err(format!("a team named \"{name}\" already exists"));
    }
    Ok(())
}

/// Return the team whose name key matches *name*, other than *exclude*.
fn duplicate_team_entry<'a>(
    roster: &'a Roster,
    name: &str,
    exclude: Option<&EntryId>,
) -> Option<&'a Entry> {
    let key = rider_name_key(name, "");
    roster.entries().iter().find(|entry| {
        entry.kind == EntryType::Team
            && exclude.is_none_or(|id| entry.key != id.0)
            && rider_name_key(&entry.display_name, "") == key
    })
}

/// Store the form's notes when it supplies one.
fn store_notes(roster: &mut Roster, entry: &EntryId, notes: &str) -> Result<(), String> {
    if notes.is_empty() {
        return Ok(());
    }
    roster
        .update_entry(
            entry,
            EntryEdit {
                notes: Some(notes),
                ..EntryEdit::default()
            },
        )
        .map_err(|error| error.to_string())
}

/// Return why the roster offered no logo card.
///
/// The two causes need different copy: no seed means no deck at all, while a
/// seeded ride that returns `None` has every card claimed.
fn logo_pick_refusal(roster: &Roster) -> &'static str {
    if roster.team_logo_seed().is_none() {
        return NO_DECK;
    }
    DECK_EXHAUSTED
}

/// Return the refusal for an entry that *id* does not name.
fn entry_not_found(entry: &EntryId) -> String {
    RosterError::EntryNotFound {
        key: entry.0.clone(),
    }
    .to_string()
}
