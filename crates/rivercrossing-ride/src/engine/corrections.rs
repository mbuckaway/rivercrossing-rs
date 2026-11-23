// SPDX-License-Identifier: GPL-3.0-only

//! Undo, the held-card lifecycle, and the pending-miss queue.
//!
//! The engine keys every deal by its monotonic deal number, so two cards of
//! one code stay distinct: a void or a hold on one never touches its
//! same-code sibling. A miss is not a crossing, so it stays out of the
//! crossing record and deals no card until a plate is assigned.

use rivercrossing_cards::Card;
use serde_json::json;

use super::{Crossing, PendingMiss, RideEngine, VoidedCard, format_timestamp, illegal};
use crate::clock::{Clock, Timestamp};
use crate::error::RideError;
use crate::event::{Event, EventAction};
use crate::roster_view::{EntryId, EntrySnapshot, RosterView};
use crate::status::RideStatus;

impl<V: RosterView, C: Clock> RideEngine<V, C> {
    /// Undo the most recent crossing and restore its card to the shoe front.
    ///
    /// Undo reverses every card disposition: a credited card leaves the hand,
    /// a held card drops out of the hold queue without a credit, and a voided
    /// card returns to the live record. When the shoe is closed, or a later
    /// deal owns the front, the card retires with the shoe instead.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the ride is not RUNNING or
    /// REOPENED, or when no crossing is recorded.
    pub fn undo_last(&mut self) -> Result<Event, RideError> {
        if self.state != RideStatus::Running && self.state != RideStatus::Reopened {
            return Err(illegal(format!("cannot undo from {}", self.state.as_str())));
        }
        let Some(last) = self.crossings.last().cloned() else {
            return Err(illegal("no crossings to undo"));
        };
        let Some(card) = self.dealt.get(&last.deal_seq).cloned() else {
            return Err(illegal("last crossing has no dealt card"));
        };
        self.remove_crossing(&last);
        self.held.remove(&last.deal_seq);
        self.discard_credited(&last.entry_id, &card);
        self.dealt.remove(&last.deal_seq);
        self.voided.remove(&last.deal_seq);
        self.restitute_or_retire(&card);
        Ok(self.append(Event {
            action: EventAction::Undo,
            payload: json!({
                "entry_id": last.entry_id.0.clone(),
                "seq": last.seq,
                "crossed_at": format_timestamp(last.crossed_at),
                "card": card.code(),
                "reason": "Undo last crossing",
            }),
        }))
    }

    /// Drop *crossing* from the ride record and its entry's lap index.
    ///
    /// The entry's index key disappears with its last crossing, so an entry
    /// with no live lap keeps reporting an empty list.
    pub(super) fn remove_crossing(&mut self, crossing: &Crossing) {
        self.crossings.retain(|known| known != crossing);
        let entry_id = crossing.entry_id.clone();
        if let Some(laps) = self.laps.get_mut(&entry_id) {
            laps.retain(|known| known != crossing);
            let empty = laps.is_empty();
            if empty {
                self.laps.remove(&entry_id);
            }
        }
    }

    /// Return the undone card to the shoe front, or let it retire.
    ///
    /// A closed shoe or a later card at the front makes restitution
    /// impossible. The undo still reverses the record, so a failed
    /// restitution only retires the card with the shoe.
    fn restitute_or_retire(&mut self, card: &Card) {
        // A closed shoe or a later front card refuses restitution. Both
        // refusals retire the card with the shoe, and a replay reproduces the
        // same shoe point, so the undo reverses the record either way.
        if self.shoe.restitute(card.clone()).is_err() {
            // The card retires with the shoe; the undo still reverses the
            // crossing record and the credited hand.
        }
    }

    /// Remove one *card* from the entry's credited hand by value.
    ///
    /// The credited hand carries no crossing identity, so the first card of
    /// that value leaves. Live and replay credit in the same order, so both
    /// pick the same card. Return that card's deal slot and the card that
    /// was removed.
    ///
    /// Call it before the deal ledger forgets the slot: the match reads the
    /// card's value from the ledger, which `void_crossing` and
    /// `reassign_crossing` remove.
    pub(super) fn discard_credited(
        &mut self,
        entry_id: &EntryId,
        card: &Card,
    ) -> Option<(u64, Card)> {
        let dealt = &self.dealt;
        let hand = self.hand.get_mut(entry_id)?;
        let index = hand
            .iter()
            .position(|(deal_seq, _)| dealt.get(deal_seq) == Some(card))?;
        let (deal_seq, _) = hand.remove(index);
        Some((deal_seq, card.clone()))
    }

    /// Return every pending miss, oldest first.
    #[must_use]
    pub fn pending_misses(&self) -> Vec<PendingMiss> {
        self.pending_misses.clone()
    }

    /// Record a miss: a passing whose plate the scorer did not capture.
    ///
    /// The miss stays outside the crossing record, deals no card, and adds no
    /// lap until [`Self::assign_plate_to_miss`] resolves a plate.
    ///
    /// # Errors
    ///
    /// Return [`RideError::EmptyReason`] when *reason* is blank, and
    /// [`RideError::IllegalState`] when the ride is not RUNNING or REOPENED.
    pub fn record_miss(&mut self, crossed_at: Timestamp, reason: &str) -> Result<Event, RideError> {
        require_reason(reason)?;
        self.require_live_state("cannot record miss")?;
        self.miss_counter = self.miss_counter.saturating_add(1);
        let miss_seq = self.miss_counter;
        self.pending_misses.push(PendingMiss {
            miss_seq,
            crossed_at,
        });
        Ok(self.append(Event {
            action: EventAction::RecordMiss,
            payload: json!({
                "miss_seq": miss_seq,
                "crossed_at": format_timestamp(crossed_at),
                "reason": reason,
            }),
        }))
    }

    /// Resolve a pending miss to a real plate.
    ///
    /// The miss leaves the queue and the crossing is recorded, with its card
    /// dealt from the live shoe, at the miss's original instant. A correction
    /// credits straight into the hand, so a short assigned crossing is never
    /// held.
    ///
    /// # Errors
    ///
    /// Return [`RideError::EmptyReason`] when *reason* is blank,
    /// [`RideError::IllegalState`] when the ride is not RUNNING or REOPENED or
    /// when no pending miss carries `miss_seq`, [`RideError::UnknownPlate`]
    /// when `new_plate` resolves to no entry, and [`RideError::IllegalState`]
    /// when the shoe refuses the deal.
    #[expect(
        clippy::too_many_arguments,
        reason = "the correction takes the miss number, the resolved plate and the audit reason"
    )]
    pub fn assign_plate_to_miss(
        &mut self,
        miss_seq: u32,
        new_plate: &str,
        reason: &str,
    ) -> Result<Event, RideError> {
        require_reason(reason)?;
        self.require_live_state("cannot assign plate to miss")?;
        let entry = self.require_entry(new_plate)?;
        let miss = self.pending_miss(miss_seq)?;
        self.assign_plate_to_miss_into(&entry, &miss, new_plate, reason)
    }

    /// Resolve the pending *miss* onto the already-resolved *entry*.
    ///
    /// The replay seam supplies the entry by its stable key and the miss
    /// from its recorded number, so a re-plating cannot move the crossing.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the shoe refuses the deal.
    #[expect(
        clippy::too_many_arguments,
        reason = "the replay seam takes the entry, the miss, the plate and the reason"
    )]
    pub(super) fn assign_plate_to_miss_into(
        &mut self,
        entry: &EntrySnapshot,
        miss: &PendingMiss,
        new_plate: &str,
        reason: &str,
    ) -> Result<Event, RideError> {
        self.pending_misses
            .retain(|candidate| candidate.miss_seq != miss.miss_seq);
        self.record_correction_crossing(entry, miss.crossed_at, new_plate)?;
        Ok(self.append(Event {
            action: EventAction::AssignPlateToMiss,
            payload: json!({
                "miss_seq": miss.miss_seq,
                "new_plate": new_plate,
                "entry_id": entry.id.0.clone(),
                "crossed_at": format_timestamp(miss.crossed_at),
                "reason": reason,
            }),
        }))
    }

    /// Return the pending miss that `miss_seq` names.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when no pending miss matches.
    pub(super) fn pending_miss(&self, miss_seq: u32) -> Result<PendingMiss, RideError> {
        self.pending_misses
            .iter()
            .find(|miss| miss.miss_seq == miss_seq)
            .cloned()
            .ok_or_else(|| illegal(format!("no pending miss with miss_seq {miss_seq}")))
    }

    /// Record one correction crossing at `crossed_at` and credit its card.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the shoe refuses the deal.
    #[expect(
        clippy::too_many_arguments,
        reason = "the recorder takes the resolved entry, the crossing instant and the typed plate"
    )]
    fn record_correction_crossing(
        &mut self,
        entry: &EntrySnapshot,
        crossed_at: Timestamp,
        plate: &str,
    ) -> Result<(), RideError> {
        let seq = self.next_lap_number(&entry.id);
        let card = self
            .deal_card()
            .map_err(|error| illegal(error.to_string()))?;
        let deal_seq = self.next_deal_sequence();
        let crossing = Crossing {
            entry_id: entry.id.clone(),
            seq,
            crossed_at,
            rider_plate: Some(plate.to_owned()),
            deal_seq,
        };
        self.insert_crossing(&crossing);
        self.dealt.insert(deal_seq, card);
        self.credit(&entry.id, deal_seq, plate);
        self.roster.mark_has_data(&entry.id);
        Ok(())
    }

    /// Release *crossing*'s held card into its entry's hand.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the crossing's card is not
    /// currently held.
    pub fn confirm_held(&mut self, crossing: &Crossing) -> Result<Event, RideError> {
        let Some(card) = self.held.remove(&crossing.deal_seq) else {
            return Err(illegal("crossing's card is not held"));
        };
        let plate = crossing.rider_plate.as_deref().unwrap_or_default();
        self.credit(&crossing.entry_id, crossing.deal_seq, plate);
        Ok(self.append(Event {
            action: EventAction::ConfirmHeld,
            payload: json!({
                "entry_id": crossing.entry_id.0.clone(),
                "seq": crossing.seq,
                "card": card.code(),
            }),
        }))
    }

    /// Discard *crossing*'s held card and never credit it.
    ///
    /// The card joins the voided-card registry, so a later correction path
    /// reads it as out of the ride. The lap itself stays recorded.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the crossing's card is not
    /// currently held.
    pub fn void_held(&mut self, crossing: &Crossing) -> Result<Event, RideError> {
        let Some(card) = self.held.remove(&crossing.deal_seq) else {
            return Err(illegal("crossing's card is not held"));
        };
        self.voided.insert(
            crossing.deal_seq,
            VoidedCard {
                card: card.clone(),
                crossing: None,
            },
        );
        Ok(self.append(Event {
            action: EventAction::VoidHeld,
            payload: json!({
                "entry_id": crossing.entry_id.0.clone(),
                "seq": crossing.seq,
                "card": card.code(),
            }),
        }))
    }

    /// Put *crossing*'s dealt card back into the hold queue.
    ///
    /// A credited card leaves the hand and re-enters the hold queue. A voided
    /// card stays out of the ride, so the crossing is given a fresh deal,
    /// which needs an open shoe: RUNNING or REOPENED. Either way the card
    /// awaits [`Self::confirm_held`] or [`Self::void_held`] again.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the card is already held or a
    /// voided card needs a deal from a closed shoe, and
    /// [`RideError::NoCrossing`] when the engine never dealt *crossing*.
    pub fn return_to_held(&mut self, crossing: &Crossing) -> Result<Event, RideError> {
        if self.held.contains_key(&crossing.deal_seq) {
            return Err(illegal("crossing's card is already held"));
        }
        let Some(card) = self.card_for(crossing) else {
            return Err(RideError::NoCrossing {
                entry_id: crossing.entry_id.0.clone(),
                seq: crossing.seq,
            });
        };
        let card = self.rehold_card(crossing, card)?;
        self.held.insert(crossing.deal_seq, card.clone());
        Ok(self.append(Event {
            action: EventAction::ReturnToHeld,
            payload: json!({
                "entry_id": crossing.entry_id.0.clone(),
                "seq": crossing.seq,
                "card": card.code(),
            }),
        }))
    }

    /// Return the card that *crossing* re-holds, dealing a fresh one when the
    /// old card is voided.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when a voided card needs a deal from
    /// a closed shoe, or when the shoe refuses the deal.
    fn rehold_card(&mut self, crossing: &Crossing, card: Card) -> Result<Card, RideError> {
        if !self.voided.contains_key(&crossing.deal_seq) {
            self.discard_credited(&crossing.entry_id, &card);
            return Ok(card);
        }
        self.require_live_state("cannot re-deal a voided card")?;
        let fresh = self
            .deal_card()
            .map_err(|error| illegal(error.to_string()))?;
        self.dealt.insert(crossing.deal_seq, fresh.clone());
        // The re-dealt slot is live again, so the crossing's card is no longer
        // voided; the retired card is referenced by no crossing.
        self.voided.remove(&crossing.deal_seq);
        Ok(fresh)
    }

    /// Return the card dealt for *crossing*, held or credited.
    #[must_use]
    pub fn card_for(&self, crossing: &Crossing) -> Option<Card> {
        self.dealt.get(&crossing.deal_seq).cloned()
    }

    /// Return *crossing*'s held card, or `None` when it is not held.
    #[must_use]
    pub fn held_card_for(&self, crossing: &Crossing) -> Option<Card> {
        self.held.get(&crossing.deal_seq).cloned()
    }

    /// Return whether the card dealt for *crossing* is voided.
    ///
    /// The lookup is by the crossing's monotonic deal number, so a value-equal
    /// card on another deal stays live.
    #[must_use]
    pub fn is_card_voided(&self, crossing: &Crossing) -> bool {
        self.voided.contains_key(&crossing.deal_seq)
    }

    /// Refuse a command unless the ride is RUNNING or REOPENED.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] with *prefix* and the state name.
    pub(super) fn require_live_state(&self, prefix: &str) -> Result<(), RideError> {
        if self.state != RideStatus::Running && self.state != RideStatus::Reopened {
            return Err(illegal(format!("{prefix} from {}", self.state.as_str())));
        }
        Ok(())
    }
}

/// Refuse a blank correction reason.
///
/// # Errors
///
/// Return [`RideError::EmptyReason`] when *reason* holds no text after a trim.
pub(super) fn require_reason(reason: &str) -> Result<(), RideError> {
    if reason.trim().is_empty() {
        return Err(RideError::EmptyReason);
    }
    Ok(())
}

impl<V: RosterView, C: Clock> RideEngine<V, C> {
    /// Deal one shoe card to *plate*'s entry by hand.
    ///
    /// The operator's bonus-card correction deals one card straight into
    /// the entry's hand, marks the entry as carrying recorded data, and
    /// appends the audit event. A manual card is entry-scoped, so it
    /// carries no rider tag and a per-rider DNF never forfeits it. An
    /// empty *reason* is allowed, because the oracle records whatever the
    /// dialog supplies without a guard.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the ride is not RUNNING or
    /// REOPENED, when the shoe refuses the deal, and
    /// [`RideError::UnknownPlate`] when *plate* resolves to no entry.
    pub fn deal_manual(&mut self, plate: &str, reason: Option<&str>) -> Result<Event, RideError> {
        self.require_live_state("cannot deal manually")?;
        let entry = self.require_entry(plate)?;
        self.deal_manual_into(&entry, plate, reason.unwrap_or_default())
    }

    /// Deal one card to the already-resolved *entry*.
    ///
    /// The replay seam supplies the entry by its stable key, so a
    /// mid-ride re-plating cannot credit the wrong hand.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the shoe refuses the deal.
    #[expect(
        clippy::too_many_arguments,
        reason = "the replay seam takes the entry, the typed plate and the reason"
    )]
    pub(super) fn deal_manual_into(
        &mut self,
        entry: &EntrySnapshot,
        plate: &str,
        reason: &str,
    ) -> Result<Event, RideError> {
        let card = self
            .deal_card()
            .map_err(|error| illegal(error.to_string()))?;
        let deal_seq = self.next_deal_sequence();
        self.dealt.insert(deal_seq, card.clone());
        self.credit_untagged(&entry.id, deal_seq);
        self.roster.mark_has_data(&entry.id);
        Ok(self.append(Event {
            action: EventAction::DealManual,
            payload: json!({
                "plate": plate,
                "entry_id": entry.id.0.clone(),
                "card": card.code(),
                "reason": reason,
            }),
        }))
    }

    /// Re-time one crossing without re-dealing its card.
    ///
    /// # Errors
    ///
    /// Return [`RideError::EmptyReason`] when *reason* is blank,
    /// [`RideError::IllegalState`] when the ride is not RUNNING or
    /// REOPENED, [`RideError::NoCrossing`] when the entry has no such
    /// lap, and [`RideError::LapTimeNotPositive`] when `crossed_at` is at
    /// or before the crossing's predecessor.
    #[expect(
        clippy::too_many_arguments,
        reason = "the correction takes the entry, the lap, the new instant and the audit reason"
    )]
    pub fn edit_crossing(
        &mut self,
        entry_id: &EntryId,
        seq: u32,
        crossed_at: Timestamp,
        reason: &str,
    ) -> Result<Event, RideError> {
        require_reason(reason)?;
        self.require_live_state("cannot edit crossing")?;
        let crossing = self.require_crossing(entry_id, seq)?;
        let previous = self.preceding_instant(&crossing)?;
        let label = self.entry_label(&crossing.entry_id);
        require_lap_after(&label, crossed_at, previous)?;
        let replacement = Crossing {
            crossed_at,
            ..crossing.clone()
        };
        self.replace_crossing(&crossing, &replacement);
        Ok(self.append(Event {
            action: EventAction::EditCrossing,
            payload: json!({
                "entry_id": crossing.entry_id.0.clone(),
                "seq": crossing.seq,
                "previous_crossed_at": format_timestamp(crossing.crossed_at),
                "crossed_at": format_timestamp(crossed_at),
                "reason": reason,
            }),
        }))
    }

    /// Void one crossing and its card; the entry's later laps renumber.
    ///
    /// The card retires without restitution, because returning a card to
    /// the shoe is [`Self::undo_last`]'s job alone.
    ///
    /// # Errors
    ///
    /// Return [`RideError::EmptyReason`] when *reason* is blank,
    /// [`RideError::IllegalState`] when the ride is not RUNNING or
    /// REOPENED or the crossing has no dealt card, and
    /// [`RideError::NoCrossing`] when the entry has no such lap.
    #[expect(
        clippy::too_many_arguments,
        reason = "the correction takes the entry, the lap and the audit reason"
    )]
    pub fn void_crossing(
        &mut self,
        entry_id: &EntryId,
        seq: u32,
        reason: &str,
    ) -> Result<Event, RideError> {
        require_reason(reason)?;
        self.require_live_state("cannot void crossing")?;
        let crossing = self.require_crossing(entry_id, seq)?;
        let Some(card) = self.dealt.get(&crossing.deal_seq).cloned() else {
            return Err(illegal("crossing has no dealt card"));
        };
        self.held.remove(&crossing.deal_seq);
        self.discard_credited(&crossing.entry_id, &card);
        self.dealt.remove(&crossing.deal_seq);
        self.voided.insert(
            crossing.deal_seq,
            VoidedCard {
                card,
                crossing: Some(crossing.clone()),
            },
        );
        self.remove_crossing(&crossing);
        self.renumber_later(&crossing.entry_id, crossing.seq);
        Ok(self.append(Event {
            action: EventAction::VoidCrossing,
            payload: json!({
                "entry_id": crossing.entry_id.0.clone(),
                "seq": crossing.seq,
                "reason": reason,
            }),
        }))
    }

    /// Record one missed crossing at an explicit past instant.
    ///
    /// # Errors
    ///
    /// Return [`RideError::EmptyReason`] when *reason* is blank,
    /// [`RideError::IllegalState`] when the ride is not RUNNING or
    /// REOPENED, [`RideError::UnknownPlate`] when *plate* resolves to no
    /// entry, and [`RideError::LapTimeNotPositive`] when `crossed_at` is
    /// at or before the entry's latest crossing.
    #[expect(
        clippy::too_many_arguments,
        reason = "the correction takes the plate, the crossing instant and the audit reason"
    )]
    pub fn add_crossing_at(
        &mut self,
        plate: &str,
        crossed_at: Timestamp,
        reason: &str,
    ) -> Result<Event, RideError> {
        require_reason(reason)?;
        self.require_live_state("cannot add crossing")?;
        let entry = self.require_entry(plate)?;
        self.add_crossing_at_into(&entry, plate, crossed_at, reason)
    }

    /// Record a missed crossing on the already-resolved *entry*.
    ///
    /// The replay seam supplies the entry by its stable key. The lap-time
    /// gate still names the entry's operator-facing plate.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the ride has no start
    /// instant or the shoe refuses the deal, and
    /// [`RideError::LapTimeNotPositive`] when `crossed_at` is at or before
    /// the entry's latest crossing.
    #[expect(
        clippy::too_many_arguments,
        reason = "the replay seam takes the entry, the typed plate, the instant and the reason"
    )]
    pub(super) fn add_crossing_at_into(
        &mut self,
        entry: &EntrySnapshot,
        plate: &str,
        crossed_at: Timestamp,
        reason: &str,
    ) -> Result<Event, RideError> {
        let latest = self.latest_instant(&entry.id)?;
        require_lap_after(&entry.plate, crossed_at, latest)?;
        self.record_correction_crossing(entry, crossed_at, plate)?;
        Ok(self.append(Event {
            action: EventAction::AddCrossingAt,
            payload: json!({
                "plate": plate,
                "entry_id": entry.id.0.clone(),
                "crossed_at": format_timestamp(crossed_at),
                "reason": reason,
            }),
        }))
    }

    /// Reattribute one crossing, and its card, to another entry.
    ///
    /// The card travels in every disposition: held stays held, credited
    /// re-credits, and a voided card stays voided.
    ///
    /// # Errors
    ///
    /// Return [`RideError::EmptyReason`] when *reason* is blank,
    /// [`RideError::IllegalState`] when the ride is not RUNNING or
    /// REOPENED, when *seq* names no ride-wide crossing, or when the
    /// crossing has no dealt card, and [`RideError::UnknownPlate`] when
    /// `new_plate` resolves to no entry.
    #[expect(
        clippy::too_many_arguments,
        reason = "the correction takes the ride-wide ordinal, the destination plate and the reason"
    )]
    pub fn reassign_crossing(
        &mut self,
        seq: u32,
        new_plate: &str,
        reason: &str,
    ) -> Result<Event, RideError> {
        require_reason(reason)?;
        self.require_live_state("cannot reassign crossing")?;
        let destination = self.require_entry(new_plate)?;
        self.reassign_crossing_into(&destination, seq, new_plate, reason)
    }

    /// Reattribute crossing *seq* to the already-resolved destination.
    ///
    /// The replay seam supplies the destination by its stable key. *seq* is
    /// the ride-wide record-order ordinal, so the replayed and live
    /// crossings are the same list position.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when *seq* names no ride-wide
    /// crossing or the crossing has no dealt card.
    #[expect(
        clippy::too_many_arguments,
        reason = "the replay seam takes the destination, the ordinal, the plate and the reason"
    )]
    pub(super) fn reassign_crossing_into(
        &mut self,
        destination: &EntrySnapshot,
        seq: u32,
        new_plate: &str,
        reason: &str,
    ) -> Result<Event, RideError> {
        let Some(crossing) = self.crossing_at_ordinal(seq) else {
            return Err(illegal(format!("no crossing at ordinal {seq}")));
        };
        let Some(card) = self.dealt.get(&crossing.deal_seq).cloned() else {
            return Err(illegal("crossing has no dealt card"));
        };
        let held = self.held.remove(&crossing.deal_seq);
        self.discard_credited(&crossing.entry_id, &card);
        self.dealt.remove(&crossing.deal_seq);
        self.remove_crossing(&crossing);
        self.renumber_later(&crossing.entry_id, crossing.seq);
        let replacement = Crossing {
            entry_id: destination.id.clone(),
            seq: self.next_lap_number(&destination.id),
            crossed_at: crossing.crossed_at,
            rider_plate: Some(new_plate.to_owned()),
            deal_seq: crossing.deal_seq,
        };
        self.insert_crossing(&replacement);
        self.dealt.insert(crossing.deal_seq, card);
        if let Some(held_card) = held {
            self.held.insert(crossing.deal_seq, held_card);
        } else if !self.voided.contains_key(&crossing.deal_seq) {
            self.credit(&destination.id, crossing.deal_seq, new_plate);
        }
        Ok(self.append(Event {
            action: EventAction::Reassign,
            payload: json!({
                "seq": seq,
                "old_entry_id": crossing.entry_id.0.clone(),
                "new_entry_id": destination.id.0.clone(),
                "new_plate": new_plate,
                "reason": reason,
            }),
        }))
    }

    /// Mark one rider, or one whole entry, DNF.
    ///
    /// A pooled team member's own plate marks that rider alone, and the
    /// engine keeps the mark in its per-rider set. Every other plate marks
    /// the whole entry through the roster. Every recorded lap and card is
    /// kept either way.
    ///
    /// # Errors
    ///
    /// Return [`RideError::EmptyReason`] when *reason* is blank,
    /// [`RideError::IllegalState`] when the ride is not RUNNING or
    /// REOPENED, and [`RideError::UnknownPlate`] when *plate* resolves to
    /// no entry.
    pub fn mark_dnf(&mut self, plate: &str, reason: &str) -> Result<Event, RideError> {
        require_reason(reason)?;
        self.require_live_state("cannot mark DNF")?;
        let entry = self.require_entry(plate)?;
        let rider = entry.is_team
            && entry
                .riders
                .iter()
                .any(|member| member.plate.as_deref() == Some(plate));
        Ok(self.record_dnf(&entry, plate, rider, reason))
    }

    /// Set one DNF mark on the already-resolved *entry* and append its event.
    ///
    /// The replay seam reads the recorded scope from the payload, because a
    /// rider's DNF has no roster column to come back from and a member may
    /// have changed teams since the mark.
    #[expect(
        clippy::too_many_arguments,
        reason = "the replay seam takes the entry, the plate, the scope and the reason"
    )]
    pub(super) fn record_dnf(
        &mut self,
        entry: &EntrySnapshot,
        plate: &str,
        rider: bool,
        reason: &str,
    ) -> Event {
        if rider {
            self.dnf_riders.insert(plate.to_owned());
        } else {
            self.roster.set_entry_dnf(&entry.id, plate, false, reason);
        }
        let name = entry
            .riders
            .iter()
            .find(|member| member.plate.as_deref() == Some(plate))
            .map_or_else(
                || entry.display_name.clone(),
                |member| member.full_name.clone(),
            );
        self.append(Event {
            action: EventAction::Dnf,
            payload: json!({
                "entry_id": entry.id.0.clone(),
                "plate": plate,
                "rider": rider,
                "reason": reason,
                "display": format!("{plate} · {name}"),
            }),
        })
    }

    /// Void one credited card; its crossing and lap stay recorded.
    ///
    /// A held card is refused, because the short-lap review surface owns
    /// it. A card voided here retires at its crossing's deal slot, so a
    /// later reassignment never resurrects it.
    ///
    /// # Errors
    ///
    /// Return [`RideError::EmptyReason`] when *reason* is blank,
    /// [`RideError::IllegalState`] when the ride is not RUNNING or
    /// REOPENED, when the card is held, or when the card is not credited
    /// to the entry, and [`RideError::UnknownPlate`] when the typed
    /// `entry_id` resolves to no entry.
    #[expect(
        clippy::too_many_arguments,
        reason = "the correction takes the entry, the card and the audit reason"
    )]
    pub fn void_card(
        &mut self,
        entry_id: &str,
        card: &Card,
        reason: &str,
    ) -> Result<Event, RideError> {
        require_reason(reason)?;
        self.require_live_state("cannot void card")?;
        let entry = self.require_entry(entry_id)?;
        self.void_card_into(&entry, card, reason)
    }

    /// Void *card* from the already-resolved *entry*'s hand.
    ///
    /// The replay seam supplies the entry by its stable key, so a re-plating
    /// between the deal and the void cannot strand the card.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the card is held or is not
    /// credited to the entry.
    #[expect(
        clippy::too_many_arguments,
        reason = "the replay seam takes the entry, the card and the reason"
    )]
    pub(super) fn void_card_into(
        &mut self,
        entry: &EntrySnapshot,
        card: &Card,
        reason: &str,
    ) -> Result<Event, RideError> {
        let Some((deal_seq, retired)) = self.discard_credited(&entry.id, card) else {
            return if self.entry_holds_card(&entry.id, card) {
                Err(illegal(
                    "card is held; confirm or void it through the review panel",
                ))
            } else {
                Err(illegal(format!(
                    "no dealt card {} credited to {}",
                    card.code(),
                    entry.plate
                )))
            };
        };
        self.voided.insert(
            deal_seq,
            VoidedCard {
                card: retired,
                crossing: None,
            },
        );
        Ok(self.append(Event {
            action: EventAction::VoidCard,
            payload: json!({
                "entry_id": entry.id.0.clone(),
                "card": card.code(),
                "reason": reason,
            }),
        }))
    }

    /// Return the entry that *plate* resolves to.
    ///
    /// # Errors
    ///
    /// Return [`RideError::UnknownPlate`] when no entry matches.
    fn require_entry(&self, plate: &str) -> Result<EntrySnapshot, RideError> {
        self.roster
            .resolve_plate(plate)
            .ok_or_else(|| RideError::UnknownPlate {
                plate: plate.to_owned(),
            })
    }

    /// Return the live crossing that `entry_id`/`seq` names.
    ///
    /// # Errors
    ///
    /// Return [`RideError::NoCrossing`] when no live crossing matches.
    pub(super) fn require_crossing(
        &self,
        entry_id: &EntryId,
        seq: u32,
    ) -> Result<Crossing, RideError> {
        self.crossings
            .iter()
            .find(|crossing| &crossing.entry_id == entry_id && crossing.seq == seq)
            .cloned()
            .ok_or_else(|| RideError::NoCrossing {
                entry_id: entry_id.0.clone(),
                seq,
            })
    }

    /// Return the crossing at a 1-based ride-wide ordinal.
    fn crossing_at_ordinal(&self, ordinal: u32) -> Option<Crossing> {
        let index = usize::try_from(ordinal).ok()?.checked_sub(1)?;
        self.crossings.get(index).cloned()
    }

    /// Return the operator-facing plate of an entry, or its key.
    fn entry_label(&self, entry_id: &EntryId) -> String {
        self.roster
            .entry(entry_id)
            .map_or_else(|| entry_id.0.clone(), |entry| entry.plate)
    }

    /// Return the instant the crossing is measured from.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the crossing is the entry's
    /// first and the ride never started.
    pub(super) fn preceding_instant(&self, crossing: &Crossing) -> Result<Timestamp, RideError> {
        let laps = self.laps.get(&crossing.entry_id);
        let position = laps.and_then(|laps| laps.iter().position(|known| known == crossing));
        if let Some(previous) = position
            .and_then(|position| position.checked_sub(1))
            .and_then(|index| laps.and_then(|laps| laps.get(index)))
        {
            return Ok(previous.crossed_at);
        }
        self.require_actual_start()
    }

    /// Return the entry's latest crossing instant.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the entry has no lap and
    /// the ride never started.
    fn latest_instant(&self, entry_id: &EntryId) -> Result<Timestamp, RideError> {
        self.laps
            .get(entry_id)
            .and_then(|laps| laps.last())
            .map_or_else(
                || self.require_actual_start(),
                |crossing| Ok(crossing.crossed_at),
            )
    }

    /// Swap *old* for *new* in place, carrying its card and hold.
    fn replace_crossing(&mut self, old: &Crossing, new: &Crossing) {
        if let Some(slot) = self.crossings.iter_mut().find(|known| **known == *old) {
            *slot = new.clone();
        }
        let Some(laps) = self.laps.get_mut(&new.entry_id) else {
            return;
        };
        if let Some(slot) = laps.iter_mut().find(|known| **known == *old) {
            *slot = new.clone();
        }
        if new.crossed_at != old.crossed_at {
            laps.sort_by_key(|known| known.crossed_at);
        }
    }

    /// Decrement every later live crossing of `entry_id` by one.
    pub(super) fn renumber_later(&mut self, entry_id: &EntryId, after_seq: u32) {
        let later: Vec<Crossing> = self
            .crossings
            .iter()
            .filter(|crossing| &crossing.entry_id == entry_id && crossing.seq > after_seq)
            .cloned()
            .collect();
        for crossing in later {
            let replacement = Crossing {
                seq: crossing.seq.saturating_sub(1),
                ..crossing.clone()
            };
            self.replace_crossing(&crossing, &replacement);
        }
    }

    /// Return whether `entry_id` holds *card* for review.
    fn entry_holds_card(&self, entry_id: &EntryId, card: &Card) -> bool {
        self.held.iter().any(|(&deal_seq, held_card)| {
            held_card == card
                && self
                    .crossings
                    .iter()
                    .any(|crossing| crossing.deal_seq == deal_seq && &crossing.entry_id == entry_id)
        })
    }
}

/// Refuse a crossing instant at or before the instant its lap starts from.
///
/// # Errors
///
/// Return [`RideError::LapTimeNotPositive`] when `crossed_at` is not
/// strictly after *previous*.
fn require_lap_after(
    entry_label: &str,
    crossed_at: Timestamp,
    previous: Timestamp,
) -> Result<(), RideError> {
    if crossed_at <= previous {
        return Err(RideError::LapTimeNotPositive {
            entry_label: entry_label.to_owned(),
            crossed_at: format_timestamp(crossed_at),
            previous: format_timestamp(previous),
        });
    }
    Ok(())
}
