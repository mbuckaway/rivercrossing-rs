// SPDX-License-Identifier: GPL-3.0-only

//! The roster data types: the rider, the entry, and the audit event.
//!
//! The entry kind and the entry status belong to [`crate::enums`], which owns
//! their stored spellings; this module only stores one of each.

use serde_json::Value;

use crate::enums::{EntryStatus, EntryType};

pub use rivercrossing_ride::EntryId;

/// One rider on an entry (spec S2 `rider` table).
///
/// The operator enters a first name and a last name; [`Rider::full_name`]
/// joins them for every display and audit site. `plate` holds the rider's own
/// plate only under the `rider_pooled` plate model. `sex` is `"M"`, `"F"` or
/// `None` when the registration leaves it blank.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rider {
    /// The rider's first name, as entered.
    pub first_name: String,
    /// The rider's last name, as entered; empty for a one-word name.
    pub last_name: String,
    /// The rider's own plate, when the plate model gives the rider one.
    pub plate: Option<String>,
    /// The rider's sex spelling, when the roster records it.
    pub sex: Option<String>,
    /// The rider's display order on the entry.
    pub sort_order: i32,
}

impl Rider {
    /// Return the display name: the trimmed first and last names joined.
    ///
    /// A blank part contributes no separator, so a one-word rider renders
    /// without a trailing space. Whitespace inside a part survives unchanged.
    #[must_use]
    pub fn full_name(&self) -> String {
        join_name_parts(self.first_name.trim(), self.last_name.trim())
    }
}

/// Join two trimmed name parts with one space, skipping a blank part.
fn join_name_parts(first: &str, last: &str) -> String {
    match (first.is_empty(), last.is_empty()) {
        (true, true) => String::new(),
        (false, true) => first.to_owned(),
        (true, false) => last.to_owned(),
        (false, false) => format!("{first} {last}"),
    }
}

/// One roster entry (spec S2 `entry` table): a solo rider or a team.
///
/// `key` is the entry's stable identity. It is minted once and never edited,
/// while `plate` is mutable: a pooled team re-derives its plate from its
/// lowest-numbered rider, and a mid-ride move re-derives both teams' plates.
/// `has_data` is the permanent delete guard: once set, the entry is never
/// deleted, only marked DNF or void.
#[derive(Debug, Clone)]
pub struct Entry {
    /// The stable identity that the ride engine files crossings under.
    pub key: String,
    /// The plate that the entry currently carries.
    pub plate: String,
    /// The name that the entry list shows.
    pub display_name: String,
    /// The entry's kind.
    pub kind: EntryType,
    /// The entry's riders.
    pub riders: Vec<Rider>,
    /// The entry's standing.
    pub status: EntryStatus,
    /// The operator's notes.
    pub notes: String,
    /// True once the entry carries recorded data.
    pub has_data: bool,
    /// The team's logo card code, when one is assigned.
    pub logo_card: Option<String>,
}

impl Entry {
    /// Build an entry and mint its stable key.
    #[expect(
        clippy::too_many_arguments,
        reason = "the four fields mirror the oracle's positional constructor; a parameter struct would add a type with one caller"
    )]
    pub fn new(
        plate: impl Into<String>,
        display_name: impl Into<String>,
        kind: EntryType,
        riders: Vec<Rider>,
    ) -> Self {
        Self::restored(new_key(), plate, display_name, kind, riders)
    }

    /// Build an entry with a caller-supplied stable key.
    ///
    /// A reloaded ride restores each entry's key from the store, so the ride
    /// engine's recorded crossings still resolve.
    #[expect(
        clippy::too_many_arguments,
        reason = "the five fields mirror the oracle's positional constructor; a parameter struct would add a type with one caller"
    )]
    pub fn restored(
        key: impl Into<String>,
        plate: impl Into<String>,
        display_name: impl Into<String>,
        kind: EntryType,
        riders: Vec<Rider>,
    ) -> Self {
        Self {
            key: key.into(),
            plate: plate.into(),
            display_name: display_name.into(),
            kind,
            riders,
            status: EntryStatus::Active,
            notes: String::new(),
            has_data: false,
            logo_card: None,
        }
    }

    /// Return the entry's current rider count.
    #[must_use]
    pub fn team_size(&self) -> usize {
        self.riders.len()
    }

    /// Return the entry's stable identity.
    ///
    /// The ride engine files crossings under this id, so it survives every
    /// re-plating.
    #[must_use]
    pub fn id(&self) -> EntryId {
        EntryId(self.key.clone())
    }
}

/// Compare two entries by their stable key alone.
///
/// The key is the entry's identity: the plate, the display name and the
/// riders all change during a ride, while the key never does. The ride engine
/// files its crossings under the key, so a re-plated entry must still compare
/// equal to the entry that the engine recorded.
impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for Entry {}

/// One append-only audit record (spec S2 `audit` table shape).
///
/// `action` names the mutation; `payload` carries whatever that mutation needs
/// to explain it later.
#[derive(Debug, Clone, PartialEq)]
pub struct AuditEvent {
    /// The mutation's name.
    pub action: String,
    /// The mutation's recorded values.
    pub payload: Value,
}

/// Return a fresh entry key: a uuid4 in its 32-character lowercase hex form.
fn new_key() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

#[cfg(test)]
mod tests;
