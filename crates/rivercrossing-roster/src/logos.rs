// SPDX-License-Identifier: GPL-3.0-only

//! The team logo: the card drawn from the ride's seeded sequence when a team is
//! created, and the card the team editor picks.
//!
//! The deck is the ride's deterministic shuffle of the natural cards, so the
//! same seed gives the same deck in every run. A pick skips every card that a
//! live team already shows, so two teams never share a logo.

use std::collections::BTreeSet;

use serde_json::json;

use crate::error::RosterError;
use crate::roster::{Roster, entry_not_found};
use crate::types::{Entry, EntryId};

/// Return every logo card that a live entry already holds.
pub(crate) fn claimed_cards(entries: &[Entry]) -> BTreeSet<&str> {
    entries
        .iter()
        .filter_map(|entry| entry.logo_card.as_deref())
        .collect()
}

/// Return the next free code in *codes*, scanning from just past *after*.
///
/// The scan starts at the code after *after* and wraps at the end of the deck,
/// so repeated picks walk the whole deck. An *after* code outside *codes* starts
/// the scan at the first code. Return `None` when every code is claimed or the
/// deck is empty.
pub(crate) fn next_logo_card<'a>(
    codes: &'a [String],
    claimed: &BTreeSet<&str>,
    after: Option<&str>,
) -> Option<&'a str> {
    let start = after
        .and_then(|card| codes.iter().position(|code| code == card))
        .map_or(0, |index| index + 1);
    codes
        .iter()
        .skip(start)
        .chain(codes.iter().take(start))
        .find(|code| !claimed.contains(code.as_str()))
        .map(String::as_str)
}

/// Return the codes that a random pick may draw: *codes* minus every claimed
/// code and minus *exclude*.
///
/// An *exclude* code outside *codes* removes nothing, which is what a staged
/// card from another deck does.
pub(crate) fn available_cards<'a>(
    codes: &'a [String],
    claimed: &BTreeSet<&str>,
    exclude: Option<&str>,
) -> Vec<&'a str> {
    codes
        .iter()
        .map(String::as_str)
        .filter(|code| !claimed.contains(*code))
        .filter(|code| Some(*code) != exclude)
        .collect()
}

/// Return the card that *pick* chooses from *available*.
///
/// *pick* receives the pool size and returns an index into the pool; an index
/// past the end wraps, so a caller's generator needs no bound of its own.
/// Return `None` when the pool is empty.
pub(crate) fn choose_card<'a>(
    available: &[&'a str],
    mut pick: impl FnMut(usize) -> usize,
) -> Option<&'a str> {
    if available.is_empty() {
        return None;
    }
    let index = pick(available.len()) % available.len();
    available.get(index).copied()
}

impl Roster {
    /// Return a free team logo card drawn from the seeded deck.
    ///
    /// The pool is the deck minus every card a live team already holds and
    /// minus *exclude*, the card the editor has staged. *pick* receives the
    /// pool size and returns an index into it. Return `None` when the ride has
    /// no seeded deck or the pool is empty.
    #[must_use]
    pub fn random_team_card(
        &self,
        exclude: Option<&str>,
        pick: impl FnMut(usize) -> usize,
    ) -> Option<String> {
        let claimed = claimed_cards(self.entries());
        choose_card(&available_cards(self.logo_deck(), &claimed, exclude), pick).map(str::to_owned)
    }

    /// Set the logo card of the entry that *entry* names.
    ///
    /// A retired entry is accepted, because the editor may still name it.
    ///
    /// # Errors
    ///
    /// Return [`RosterError::EntryNotFound`] when *entry* names no entry here.
    pub fn set_team_logo_card(&mut self, entry: &EntryId, code: &str) -> Result<(), RosterError> {
        let plate = {
            let Some(target) = self.entry_mut(entry) else {
                return Err(entry_not_found(entry));
            };
            target.logo_card = Some(code.to_owned());
            target.plate.clone()
        };
        self.log("set_team_logo_card", json!({"plate": plate, "code": code}));
        Ok(())
    }
}

#[cfg(test)]
mod tests;
