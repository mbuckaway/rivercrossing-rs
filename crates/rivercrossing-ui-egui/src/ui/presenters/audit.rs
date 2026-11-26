// SPDX-License-Identifier: GPL-3.0-only

//! The audit trail dialog's rows and its two filters, with no rendering.
//!
//! The rules follow the Python oracle's `ui/presenters/audit.py`: the list is
//! newest first, the search matches a row's entry cell or the roster display
//! name that cell resolves to, and the action choice keeps exactly its own
//! rows. Both filters narrow the same query.

use std::cmp::Reverse;

use rivercrossing_ride::Timestamp;
use rivercrossing_roster::roster::Roster;
use rivercrossing_store::AuditRow;

#[cfg(test)]
mod tests;

/// The action choice that filters nothing (the oracle's `ALL_ACTIONS`).
pub(crate) const ALL_ACTIONS: &str = "All actions";

/// The action filter's label and action pairs, in the dialog's declared order.
///
/// Every audited action appears exactly once: the engine's own event actions
/// plus the roster mutations that the store persists.
pub(crate) const ACTION_CHOICES: &[(&str, &str)] = &[
    ("Record Crossing", "record_crossing"),
    ("Add Crossing at Time", "add_crossing_at"),
    ("Edit Crossing", "edit_crossing"),
    ("Void Crossing", "void_crossing"),
    ("Undo Last Crossing", "undo"),
    ("Reassign Crossing", "reassign"),
    ("Record Miss", "record_miss"),
    ("Assign Plate to Miss", "assign_plate_to_miss"),
    ("Deal Bonus Card", "deal_manual"),
    ("Confirm Held Card", "confirm_held"),
    ("Void Held Card", "void_held"),
    ("Return to Held", "return_to_held"),
    ("Void Card", "void_card"),
    ("Move Rider", "move_rider"),
    ("Add Rider to Team", "add_rider_to_team"),
    ("Extract Rider to Solo", "extract_rider_to_solo"),
    ("Change Solo Plate", "change_solo_plate"),
    ("Change Pooled Rider Plate", "change_pooled_rider_plate"),
    ("Change Team Plate", "change_team_plate"),
    ("Remove Rider", "remove_rider"),
    ("Mark DNF", "dnf"),
    ("Shoe Reshuffle", "shoe_reshuffle"),
    ("High-card Draw", "tiebreak_draw"),
    ("Start Ride", "start"),
    ("Continue Ride", "continue"),
    ("Set Start Time", "set_start_time"),
    ("Stop Ride", "stop"),
    ("Finish Ride", "finish"),
    ("Reopen Ride", "reopen"),
];

/// One rendered audit-trail row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuditRowModel {
    /// The instant, local `HH:MM:SS`.
    pub(crate) at: String,
    /// The stored action spelling.
    pub(crate) action: String,
    /// The entry's display name, else the stored entry cell.
    pub(crate) entry: String,
    /// The stored entry cell: a plate, a display name or a summary.
    pub(crate) entry_plate: String,
    /// The payload's reason.
    pub(crate) reason: String,
}

/// Sort *rows* newest first.
///
/// The store already returns its rows newest first; the sort guards the
/// invariant against a source that does not. Equal instants keep their input
/// order.
pub(crate) fn newest_first(rows: &mut [AuditRow]) {
    rows.sort_by_key(|row| Reverse(row.at));
}

/// Return the audit rows as view models, newest first.
///
/// Each row's stored entry cell resolves through the roster, so a pooled
/// rider's plate shows its team's display name. A cell that names no entry
/// keeps its stored text.
#[must_use]
pub(crate) fn rows(store_rows: &[AuditRow], roster: &Roster) -> Vec<AuditRowModel> {
    let mut ordered = store_rows.to_vec();
    newest_first(&mut ordered);
    ordered
        .iter()
        .map(|row| AuditRowModel {
            at: clock_text(row.at),
            action: row.action.clone(),
            entry: display_name(roster, &row.entry),
            entry_plate: row.entry.clone(),
            reason: row.reason.clone(),
        })
        .collect()
}

/// Return the rows that *search* and *action* keep, in the same order.
///
/// The search is a case-insensitive substring match on the entry cell or its
/// display name; a blank search keeps every row. [`ALL_ACTIONS`] keeps every
/// action; any other value keeps only rows whose own action matches exactly.
/// Both filters apply together.
#[must_use]
pub(crate) fn filter(rows: &[AuditRowModel], search: &str, action: &str) -> Vec<AuditRowModel> {
    let needle = search.trim().to_lowercase();
    rows.iter()
        .filter(|row| matches_search(row, &needle) && matches_action(row, action))
        .cloned()
        .collect()
}

/// Return whether *row* survives the search *needle*.
fn matches_search(row: &AuditRowModel, needle: &str) -> bool {
    needle.is_empty()
        || row.entry_plate.to_lowercase().contains(needle)
        || row.entry.to_lowercase().contains(needle)
}

/// Return whether *row* survives the action filter.
fn matches_action(row: &AuditRowModel, action: &str) -> bool {
    action == ALL_ACTIONS || row.action == action
}

/// Return the display name that *plate* resolves to, or the stored cell.
fn display_name(roster: &Roster, plate: &str) -> String {
    roster
        .resolve_plate(plate)
        .map_or_else(|| plate.to_owned(), |entry| entry.display_name.clone())
}

/// Return *at* rendered as local `HH:MM:SS`.
///
/// The instant's own offset is the display zone; the caller passes a localized
/// instant, the store's own convention.
fn clock_text(at: Timestamp) -> String {
    format!("{:02}:{:02}:{:02}", at.hour(), at.minute(), at.second())
}
