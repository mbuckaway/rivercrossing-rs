// SPDX-License-Identifier: GPL-3.0-only

//! The results projection: the live standings snapshot, the DNF verdict,
//! the duplicate-crossing detector, and the venue's high-card tie-break draw.

use std::collections::BTreeMap;

use rivercrossing_cards::{Card, high_card_draw};
use rivercrossing_hands::compare::compare;
use rivercrossing_hands::{EvaluatedHand, best_hand};
use serde_json::json;

use super::{Crossing, RideEngine};
use crate::clock::{Clock, Timestamp};
use crate::event::{Event, EventAction};
use crate::roster_view::{EntryId, EntrySnapshot, RosterView};

/// The fixed salt that separates the tie-break draw from the shoe's shuffle.
const TIEBREAK_DRAW_SEED_XOR: u64 = 0x5F3A_6D1C_9E2B_0478;

/// Whether a snapshot entry is a solo rider or a team.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotKind {
    /// A solo entry.
    Solo,
    /// A team entry.
    Team,
}

impl SnapshotKind {
    /// Return the oracle's spelling of this kind.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Solo => "solo",
            Self::Team => "team",
        }
    }
}

/// One entry's live standings reading.
///
/// `cards` pools the credited cards that still score, capped at the ride's
/// `max_cards`; `hand` is the best five-card hand those cards make. A DNF
/// entry keeps its laps and times, while a DNF rider's cards are forfeited.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotEntry {
    /// The entry's stable identity.
    pub entry_id: EntryId,
    /// The plate that the entry currently carries.
    pub plate: String,
    /// The name that the entry list shows.
    pub name: String,
    /// Whether the entry is a solo rider or a team.
    pub kind: SnapshotKind,
    /// The entry's recorded lap count.
    pub laps: u32,
    /// The last crossing minus the gun, or zero before the first lap.
    pub total_time: f64,
    /// The shortest derived lap time, or zero with no laps.
    pub best_lap: f64,
    /// The scoring cards, in deal order, capped at `max_cards`.
    pub cards: Vec<Card>,
    /// The best five-card hand that the scoring cards make.
    pub hand: EvaluatedHand,
    /// True when the entry, or every one of its riders, is DNF.
    pub dnf: bool,
    /// A solo entry's lone rider's sex; always `None` for a team.
    pub sex: Option<String>,
    /// The entry's recorded tie-break card, or `None` before a draw.
    pub tiebreak_card: Option<Card>,
}

impl<V: RosterView, C: Clock> RideEngine<V, C> {
    /// Return one live result per roster entry, in roster order.
    ///
    /// The projection is pure: it reads the record and the roster and
    /// changes nothing. `cards` holds the credited cards whose rider is not
    /// DNF, capped at `max_cards`; `hand` scores exactly those cards.
    #[must_use]
    pub fn snapshot(&self) -> Vec<SnapshotEntry> {
        self.roster
            .entries()
            .into_iter()
            .map(|entry| self.snapshot_entry(entry))
            .collect()
    }

    /// Return the snapshot reading for one roster entry.
    fn snapshot_entry(&self, entry: EntrySnapshot) -> SnapshotEntry {
        let laps = self.laps.get(&entry.id).map_or(&[][..], Vec::as_slice);
        let times = self.lap_times(&entry.id);
        let total_time = laps
            .last()
            .zip(self.actual_start)
            .map_or(0.0, |(crossing, start)| {
                (crossing.crossed_at - start).as_seconds_f64()
            });
        let best_lap = times.iter().copied().reduce(f64::min).unwrap_or(0.0);
        let mut cards = self.scoring_cards(&entry.id);
        if let Some(limit) = self
            .config
            .max_cards
            .and_then(|max| usize::try_from(max).ok())
        {
            cards.truncate(limit);
        }
        let hand = best_hand(&cards);
        let dnf = self.entry_is_dnf(&entry);
        let kind = if entry.is_team {
            SnapshotKind::Team
        } else {
            SnapshotKind::Solo
        };
        let sex = if entry.is_team {
            None
        } else {
            entry.riders.first().and_then(|rider| rider.sex.clone())
        };
        let tiebreak_card = self.tiebreak.get(&entry.id).cloned();
        SnapshotEntry {
            entry_id: entry.id,
            plate: entry.plate,
            name: entry.display_name,
            kind,
            laps: u32::try_from(laps.len()).unwrap_or(u32::MAX),
            total_time,
            best_lap,
            cards,
            hand,
            dnf,
            sex,
            tiebreak_card,
        }
    }

    /// Return the credited cards of `entry_id` that still score.
    ///
    /// A DNF rider forfeits exactly the cards their own crossings credited;
    /// a card with no rider tag is never forfeited. The cards stay in the
    /// credited read that [`Self::credited_cards`] returns.
    fn scoring_cards(&self, entry_id: &EntryId) -> Vec<Card> {
        let Some(slots) = self.hand.get(entry_id) else {
            return Vec::new();
        };
        slots
            .iter()
            .filter(|(_, rider_plate)| {
                rider_plate
                    .as_deref()
                    .is_none_or(|plate| !self.dnf_riders.contains(plate))
            })
            .filter_map(|(deal_seq, _)| self.dealt.get(deal_seq).cloned())
            .collect()
    }

    /// Return whether *entry* is out of the results entirely.
    ///
    /// An entry-level DNF covers a solo or a relay team. A pooled team is out
    /// only when every one of its riders is out, because a rider's DNF has no
    /// roster column. A riderless entry is never all-DNF.
    #[must_use]
    pub fn entry_is_dnf(&self, entry: &EntrySnapshot) -> bool {
        if entry.is_dnf {
            return true;
        }
        !entry.riders.is_empty()
            && entry.riders.iter().all(|rider| {
                rider
                    .plate
                    .as_deref()
                    .is_some_and(|plate| self.dnf_riders.contains(plate))
            })
    }

    /// Return each live pair that shares one entry and one crossing instant.
    ///
    /// Pairs are `(older, newer)` in record order, and the pairs themselves
    /// are oldest instant first, then entry key. Two entries crossing
    /// together are never a pair.
    #[must_use]
    pub fn duplicate_crossings(&self) -> Vec<(Crossing, Crossing)> {
        let mut by_key: BTreeMap<(Timestamp, String), Vec<Crossing>> = BTreeMap::new();
        for crossing in &self.crossings {
            by_key
                .entry((crossing.crossed_at, crossing.entry_id.0.clone()))
                .or_default()
                .push(crossing.clone());
        }
        let mut pairs = Vec::new();
        for group in by_key.into_values() {
            pairs.extend(group_pairs(&group));
        }
        pairs
    }

    /// Group the active snapshot entries by exactly equal hand.
    ///
    /// One group per set of two or more active entries whose hands compare
    /// equal, in the first-appearance order of the group's earliest entry and
    /// in roster order within a group. A DNF entry is excluded outright.
    fn equal_hand_groups(&self) -> Vec<Vec<SnapshotEntry>> {
        let mut groups: Vec<Vec<SnapshotEntry>> = Vec::new();
        for result in self.snapshot() {
            if result.dnf {
                continue;
            }
            let matching = groups.iter_mut().find(|group| {
                group
                    .first()
                    .is_some_and(|first| compare(&first.hand, &result.hand).is_eq())
            });
            if let Some(group) = matching {
                group.push(result);
            } else {
                groups.push(vec![result]);
            }
        }
        groups.retain(|group| group.len() > 1);
        groups
    }

    /// Draw and record the venue's high-card tie-break.
    ///
    /// Every equal-hand group draws one card per entry from one fresh deck,
    /// seeded from the shoe's stored seed salted with the fixed constant, so
    /// the draw replays from the persisted seed and never deals from the
    /// shoe. A ride with no equal-hand group appends nothing.
    pub(super) fn record_tiebreak_draws(&mut self) {
        let groups = self.equal_hand_groups();
        if groups.is_empty() {
            return;
        }
        let tied: Vec<SnapshotEntry> = groups.into_iter().flatten().collect();
        let cards = high_card_draw(self.shoe.seed() ^ TIEBREAK_DRAW_SEED_XOR, tied.len());
        let mut draws = Vec::new();
        let mut labels = Vec::new();
        for (result, drawn) in tied.iter().zip(cards) {
            self.tiebreak.insert(result.entry_id.clone(), drawn.clone());
            draws.push(json!({
                "entry_id": result.entry_id.0.clone(),
                "card": drawn.code(),
            }));
            labels.push(format!("{} · {}", result.plate, drawn.code()));
        }
        self.append(Event {
            action: EventAction::TiebreakDraw,
            payload: json!({
                "draws": draws,
                "summary": labels.join(", "),
            }),
        });
    }
}

/// Return every ordered pair of one group, oldest first.
fn group_pairs(group: &[Crossing]) -> Vec<(Crossing, Crossing)> {
    group
        .iter()
        .enumerate()
        .flat_map(|(index, older)| {
            group
                .iter()
                .skip(index.saturating_add(1))
                .map(|newer| (older.clone(), newer.clone()))
        })
        .collect()
}
