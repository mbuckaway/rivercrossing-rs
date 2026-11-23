// SPDX-License-Identifier: GPL-3.0-only

//! Crossing recording, the deal ledger, and the read queries.

use rivercrossing_cards::{Card, CardsError};
use serde_json::json;

use super::{Crossing, CrossingResult, HeldCrossing, RideEngine, format_timestamp, illegal};
use crate::clock::{Clock, Timestamp};
use crate::config::RideConfig;
use crate::error::RideError;
use crate::event::{Event, EventAction};
use crate::roster_view::{EntryId, EntrySnapshot, RosterView};
use crate::status::RideStatus;

impl<V: RosterView, C: Clock> RideEngine<V, C> {
    /// Record one completed lap for *plate*.
    ///
    /// A refusal comes back as an `accepted: false` result, never an error:
    /// the ride is not RUNNING, the ride is stopped, or the plate resolves to
    /// no entry. A success records the lap, deals one card, and flags a lap
    /// shorter than the ride's minimum. The short-lap policy alone decides
    /// whether that card is held for review or credited to the hand.
    #[must_use]
    pub fn record_crossing(&mut self, plate: &str, at: Option<Timestamp>) -> CrossingResult {
        let entry = match self.admit_crossing(plate) {
            Ok(entry) => entry,
            Err(refusal) => return refusal,
        };
        let crossed_at = at.unwrap_or_else(|| self.clock.now());
        let Some(start) = self.actual_start else {
            return CrossingResult::refused(plate, "ride has not started");
        };
        self.record_for_entry(entry, plate, crossed_at, start)
    }

    /// Record one lap for the entry that a replayed event's key names.
    ///
    /// The replay seam supplies the entry by its stable key, so a
    /// mid-ride re-plating cannot move the crossing. It skips the live
    /// entry gates that the recorded action already cleared.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the ride has no start
    /// instant or the shoe refuses the deal.
    #[expect(
        clippy::too_many_arguments,
        reason = "the replay seam takes the entry, the typed plate and the crossing instant"
    )]
    pub(super) fn record_crossing_into(
        &mut self,
        entry: &EntrySnapshot,
        plate: &str,
        crossed_at: Timestamp,
    ) -> Result<(), RideError> {
        let start = self.require_actual_start()?;
        let result = self.record_for_entry(entry.clone(), plate, crossed_at, start);
        if result.accepted {
            return Ok(());
        }
        Err(illegal(
            result
                .reason
                .unwrap_or_else(|| "crossing refused".to_owned()),
        ))
    }

    /// Return the entry that *plate* names, or the refusal result.
    fn admit_crossing(&self, plate: &str) -> Result<EntrySnapshot, CrossingResult> {
        if self.state != RideStatus::Running {
            return Err(CrossingResult::refused(plate, "ride is not running"));
        }
        if self.stopped {
            return Err(CrossingResult::refused(plate, "ride is stopped"));
        }
        self.roster
            .resolve_plate(plate)
            .ok_or_else(|| CrossingResult::refused(plate, "unknown_plate"))
    }

    /// Record one lap for the already-resolved entry.
    #[expect(
        clippy::too_many_arguments,
        reason = "the private recorder takes the entry, the typed plate, the crossing instant and the gun"
    )]
    fn record_for_entry(
        &mut self,
        entry: EntrySnapshot,
        plate: &str,
        crossed_at: Timestamp,
        start: Timestamp,
    ) -> CrossingResult {
        let entry_id = entry.id.clone();
        let previous = self
            .laps
            .get(&entry_id)
            .and_then(|laps| laps.last())
            .map_or(start, |crossing| crossing.crossed_at);
        let seq = self.next_lap_number(&entry_id);
        let card = match self.deal_card() {
            Ok(card) => card,
            Err(error) => return CrossingResult::refused(plate, error.to_string()),
        };
        let deal_seq = self.next_deal_sequence();
        let crossing = Crossing {
            entry_id: entry_id.clone(),
            seq,
            crossed_at,
            rider_plate: Some(plate.to_owned()),
            deal_seq,
        };
        self.insert_crossing(&crossing);
        self.dealt.insert(deal_seq, card.clone());
        self.roster.mark_has_data(&entry_id);
        let lap_time = (crossed_at - previous).as_seconds_f64();
        let short = lap_time < f64::from(self.config.min_lap_s);
        if short && self.config.hold_short_laps {
            self.held.insert(deal_seq, card.clone());
        } else {
            self.credit(&entry_id, deal_seq, plate);
        }
        self.append_crossing_event(&entry, &crossing);
        CrossingResult {
            accepted: true,
            plate: plate.to_owned(),
            entry_plate: Some(entry.plate),
            entry_name: Some(entry.display_name),
            lap: seq,
            lap_time,
            reason: None,
            card: Some(card),
            flagged: short,
        }
    }

    /// Append the record event for one recorded crossing.
    fn append_crossing_event(&mut self, entry: &EntrySnapshot, crossing: &Crossing) {
        let plate = crossing.rider_plate.as_deref().unwrap_or_default();
        self.append(Event {
            action: EventAction::RecordCrossing,
            payload: json!({
                "plate": plate,
                "entry_id": crossing.entry_id.0.clone(),
                "lap": crossing.seq,
                "crossed_at": format_timestamp(crossing.crossed_at),
                "reason": crossing_reason(entry, plate),
            }),
        });
    }

    /// Deal the next shoe card, reshuffling and auditing when the cycle drains.
    ///
    /// # Errors
    ///
    /// Return the shoe's own error when it is closed, or when a closed shoe
    /// also has no card left to deal.
    pub(super) fn deal_card(&mut self) -> Result<Card, CardsError> {
        if self.shoe.remaining() == 0 {
            self.shoe.reshuffle()?;
            self.record_shoe_reshuffle();
        }
        self.shoe.deal().map(|(card, _index)| card)
    }

    /// Append the audit event for a reshuffle that just opened a new cycle.
    fn record_shoe_reshuffle(&mut self) {
        let jokers_added = self.shoe.jokers_in_cycle();
        let payload = json!({
            "cycle": self.shoe.cycle(),
            "jokers_added": jokers_added,
            "reason": format!("{jokers_added} jokers added"),
        });
        self.append(Event {
            action: EventAction::ShoeReshuffle,
            payload,
        });
    }

    /// Append *crossing* to the record and index it by entry and instant.
    ///
    /// Equal instants keep record order, because the search inserts after
    /// every crossing whose instant is at or before the new one.
    pub(super) fn insert_crossing(&mut self, crossing: &Crossing) {
        let entry_id = crossing.entry_id.clone();
        let position = self.laps.get(&entry_id).map_or(0, |laps| {
            laps.partition_point(|known| known.crossed_at <= crossing.crossed_at)
        });
        self.laps
            .entry(entry_id)
            .or_default()
            .insert(position, crossing.clone());
        self.crossings.push(crossing.clone());
    }

    /// Append one credited card to `entry_id`'s hand.
    #[expect(
        clippy::too_many_arguments,
        reason = "the private credit records one card, its deal number and its rider attribution"
    )]
    pub(super) fn credit(&mut self, entry_id: &EntryId, deal_seq: u64, rider_plate: &str) {
        self.hand
            .entry(entry_id.clone())
            .or_default()
            .push((deal_seq, Some(rider_plate.to_owned())));
    }

    /// Append one credited card with no rider tag.
    ///
    /// A manual bonus card carries no rider, so a per-rider DNF never
    /// forfeits it and a rider move leaves it with its entry.
    pub(super) fn credit_untagged(&mut self, entry_id: &EntryId, deal_seq: u64) {
        self.hand
            .entry(entry_id.clone())
            .or_default()
            .push((deal_seq, None));
    }

    /// Return the entry's next 1-based lap number.
    pub(super) fn next_lap_number(&self, entry_id: &EntryId) -> u32 {
        let count = self.laps.get(entry_id).map_or(0, Vec::len);
        u32::try_from(count).unwrap_or(u32::MAX).saturating_add(1)
    }

    /// Return the next deal sequence number and advance the counter.
    pub(super) fn next_deal_sequence(&mut self) -> u64 {
        let sequence = self.next_deal_seq;
        self.next_deal_seq = self.next_deal_seq.saturating_add(1);
        sequence
    }

    /// Return the lifecycle state.
    #[must_use]
    pub fn state(&self) -> RideStatus {
        self.state
    }

    /// Return whether the stop guard is set.
    #[must_use]
    pub fn stopped(&self) -> bool {
        self.stopped
    }

    /// Return the gun instant, or `None` before the ride starts.
    #[must_use]
    pub fn actual_start(&self) -> Option<Timestamp> {
        self.actual_start
    }

    /// Return the ride's live setup config.
    #[must_use]
    pub fn config(&self) -> &RideConfig {
        &self.config
    }

    /// Return every event, oldest first.
    #[must_use]
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// Return every recorded crossing, oldest first.
    #[must_use]
    pub fn crossings(&self) -> &[Crossing] {
        &self.crossings
    }

    /// Return each derived lap time for `entry_id`, earliest lap first.
    ///
    /// Lap 1 derives from the gun and every later lap from the previous
    /// crossing, so a back-dated start recomputes lap 1 alone. An entry with
    /// no laps, or a ride with no start instant, returns an empty list.
    #[must_use]
    pub fn lap_times(&self, entry_id: &EntryId) -> Vec<f64> {
        let Some(start) = self.actual_start else {
            return Vec::new();
        };
        let Some(laps) = self.laps.get(entry_id) else {
            return Vec::new();
        };
        let mut previous = start;
        let mut times = Vec::with_capacity(laps.len());
        for crossing in laps {
            times.push((crossing.crossed_at - previous).as_seconds_f64());
            previous = crossing.crossed_at;
        }
        times
    }

    /// Return seconds since the gun, read from the clock.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the ride has never started.
    pub fn elapsed(&self) -> Result<f64, RideError> {
        let start = self.require_actual_start()?;
        Ok((self.clock.now() - start).as_seconds_f64())
    }

    /// Return seconds until the planned duration elapses.
    ///
    /// The value goes negative once the planned duration passes.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the ride has never started.
    pub fn remaining(&self) -> Result<f64, RideError> {
        Ok(f64::from(self.config.planned_duration_s) - self.elapsed()?)
    }

    /// Return the recorded final elapsed of a closed ride.
    ///
    /// Return zero when the ride never started or never finished.
    #[must_use]
    pub fn closed_elapsed(&self) -> f64 {
        match (self.finished_at, self.actual_start) {
            (Some(finished), Some(start)) => (finished - start).as_seconds_f64(),
            _ => 0.0,
        }
    }

    /// Return the number of cards left in the shoe's current cycle.
    #[must_use]
    pub fn shoe_remaining(&self) -> usize {
        self.shoe.remaining()
    }

    /// Return the number of cards in the shoe's current cycle.
    #[must_use]
    pub fn shoe_total(&self) -> usize {
        self.shoe.remaining() + self.shoe.dealt()
    }

    /// Return how many entries the roster holds now.
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.roster.entries().len()
    }

    /// Return how many active entries are out on the loop.
    ///
    /// An entry is on course when it is not DNF and its lap count is odd.
    #[must_use]
    pub fn on_course(&self) -> usize {
        self.roster
            .entries()
            .iter()
            .filter(|entry| {
                let odd = self
                    .laps
                    .get(&entry.id)
                    .is_some_and(|laps| laps.len() % 2 == 1);
                !entry.is_dnf && odd
            })
            .count()
    }

    /// Return whether the closed ride's results are self-test unverified.
    ///
    /// Only a FINISHED ride can read true, and a clean re-finish clears it.
    #[must_use]
    pub fn self_test_unverified(&self) -> bool {
        self.state == RideStatus::Finished && !self.self_test_failed_checks.is_empty()
    }

    /// Return every held crossing with its card, oldest first.
    #[must_use]
    pub fn held_crossings(&self) -> Vec<HeldCrossing> {
        self.held
            .iter()
            .filter_map(|(&deal_seq, card)| {
                self.crossings
                    .iter()
                    .find(|crossing| crossing.deal_seq == deal_seq)
                    .map(|crossing| HeldCrossing {
                        crossing: crossing.clone(),
                        card: card.clone(),
                    })
            })
            .collect()
    }

    /// Return every credited card for `entry_id`, in deal order.
    #[must_use]
    pub fn credited_cards(&self, entry_id: &EntryId) -> Vec<Card> {
        self.hand.get(entry_id).map_or_else(Vec::new, |entries| {
            entries
                .iter()
                .filter_map(|(deal_seq, _rider_plate)| self.dealt.get(deal_seq).cloned())
                .collect()
        })
    }
}

impl CrossingResult {
    /// Return the refusal result for *plate* and *reason*.
    fn refused(plate: &str, reason: impl Into<String>) -> Self {
        Self {
            accepted: false,
            plate: plate.to_owned(),
            entry_plate: None,
            entry_name: None,
            lap: 0,
            lap_time: 0.0,
            reason: Some(reason.into()),
            card: None,
            flagged: false,
        }
    }
}

/// Return the crossing reason: the rider who crossed, then the entry.
///
/// A plate that names no rider falls back to the plate itself, so the reason
/// is never blank.
fn crossing_reason(entry: &EntrySnapshot, plate: &str) -> String {
    let rider_name = entry
        .riders
        .iter()
        .find(|rider| rider.plate.as_deref() == Some(plate))
        .map(|rider| rider.full_name.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| plate.to_owned());
    let team = if entry.is_team {
        entry.display_name.as_str()
    } else {
        "solo"
    };
    format!("{rider_name} · {team}")
}
