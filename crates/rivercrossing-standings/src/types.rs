// SPDX-License-Identifier: GPL-3.0-only

//! The standings row types: one entry's reading and its ranked place.

use rivercrossing_cards::Card;
use rivercrossing_hands::EvaluatedHand;
use rivercrossing_ride::{EntryId, SnapshotEntry, SnapshotKind};

/// One entry's finished-ride reading.
///
/// The hand is precomputed by the ride engine's snapshot, so ranking orders by
/// it and never re-derives a hand from the cards. `sex` is a solo rider's
/// spelling and `None` for a team; `tiebreak_card` is `None` until the venue's
/// high-card draw has happened.
#[derive(Debug, Clone)]
pub struct EntryResult {
    /// The entry's stable identity.
    pub entry_id: EntryId,
    /// The plate that the entry currently carries.
    pub plate: String,
    /// The name that the entry list shows.
    pub name: String,
    /// Whether the entry is a solo rider or a team.
    pub kind: SnapshotKind,
    /// The entry's completed laps.
    pub laps: u32,
    /// The last crossing minus the gun, in seconds.
    pub total_time: f64,
    /// The shortest derived lap time, in seconds.
    pub best_lap: f64,
    /// The scoring cards, in deal order.
    pub cards: Vec<Card>,
    /// The best five-card hand that the cards make.
    pub hand: EvaluatedHand,
    /// True when the entry is DNF.
    pub dnf: bool,
    /// A solo rider's sex spelling; `None` for a team.
    pub sex: Option<String>,
    /// The card that the entry drew for the venue's tie-break.
    pub tiebreak_card: Option<Card>,
}

impl From<&SnapshotEntry> for EntryResult {
    /// Read one live snapshot row as a standings row.
    fn from(snapshot: &SnapshotEntry) -> Self {
        Self {
            entry_id: snapshot.entry_id.clone(),
            plate: snapshot.plate.clone(),
            name: snapshot.name.clone(),
            kind: snapshot.kind,
            laps: snapshot.laps,
            total_time: snapshot.total_time,
            best_lap: snapshot.best_lap,
            cards: snapshot.cards.clone(),
            hand: snapshot.hand.clone(),
            dnf: snapshot.dnf,
            sex: snapshot.sex.clone(),
            tiebreak_card: snapshot.tiebreak_card.clone(),
        }
    }
}

/// One ranked result: its 1-based place plus its tie markings.
///
/// `place` is `None` for a DNS row, which no ranker emits; only the team/solo
/// split appends one. `draw_required` marks a residual tie that no configured
/// tie-break resolved, and `tie_note` carries the note that the row shows.
#[derive(Debug, Clone)]
pub struct Placed {
    /// The entry's 1-based place, or `None` for a DNS row.
    pub place: Option<u32>,
    /// The entry's reading.
    pub result: EntryResult,
    /// The note that marks a residual tie, when one is unresolved.
    pub tie_note: Option<String>,
    /// True when the entry needs the venue's high-card draw.
    pub draw_required: bool,
}

impl Placed {
    /// Return true when this row is a DNS row.
    #[must_use]
    pub const fn is_dns(&self) -> bool {
        self.place.is_none()
    }
}

#[cfg(test)]
mod tests;
