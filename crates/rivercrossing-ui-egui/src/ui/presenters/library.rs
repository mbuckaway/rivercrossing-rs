// SPDX-License-Identifier: GPL-3.0-only

//! The ride library's rows and its delete and duplicate copy, with no
//! rendering.
//!
//! The rules and the copy follow the Python oracle's `ui/views/ride_library.py`
//! and `ui/views/dialogs.py`: the four canvas columns, the byte-exact
//! type-to-confirm delete, and the duplicate prompt that names the ride.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the EPIC-12c views wire these decisions in a later phase; phase 1 unit-tests them"
    )
)]

use rivercrossing_store::RideRow;

#[cfg(test)]
mod tests;

/// One rendered ride-library row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RideRowModel {
    /// The ride's name.
    pub(crate) name: String,
    /// The event date, `YYYY-MM-DD`.
    pub(crate) date: String,
    /// The lifecycle status, upper case.
    pub(crate) status: String,
    /// The number of live entries.
    pub(crate) entries: u32,
}

/// Return the library rows, in the order the store returns them.
///
/// The store lists rides oldest first by creation, and the operator's own
/// header sort is the view's concern, so this mapping never reorders.
#[must_use]
pub(crate) fn rows(store_rows: &[RideRow]) -> Vec<RideRowModel> {
    store_rows
        .iter()
        .map(|row| RideRowModel {
            name: row.name.clone(),
            date: date_text(row),
            status: row.status.as_str().to_uppercase(),
            entries: row.entries,
        })
        .collect()
}

/// Return the delete confirm's instruction, which names the ride.
///
/// The sentence is the oracle's own destroy notice plus the type-to-confirm
/// instruction that arms the delete button.
#[must_use]
pub(crate) fn delete_requires(row: &RideRow) -> String {
    format!(
        "Deletes \"{}\" and all its data. Type the ride name to confirm.",
        row.name
    )
}

/// Return whether *typed* matches the ride's name exactly, after a trim.
#[must_use]
pub(crate) fn delete_matches(row: &RideRow, typed: &str) -> bool {
    typed.trim() == row.name
}

/// Return the Duplicate Ride prompt's message, which names the ride.
#[must_use]
pub(crate) fn duplicate_confirm(ride_name: &str) -> String {
    format!(
        "Duplicate \"{ride_name}\" as a new DRAFT ride?\n\n\
         Copies the ride's setup and full rider list — no timing data."
    )
}

/// Return *row*'s event date as `YYYY-MM-DD`.
fn date_text(row: &RideRow) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        row.event_date.year(),
        u8::from(row.event_date.month()),
        row.event_date.day()
    )
}
