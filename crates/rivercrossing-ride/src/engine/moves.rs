// SPDX-License-Identifier: GPL-3.0-only

//! The two pooled rider moves and the re-attribution core they share.
//!
//! The roster owns membership; the engine owns the rider's data. A move
//! asks the roster to change the membership, then re-keys the rider's
//! credited cards, live laps and restored voided laps to the destination.

use serde_json::json;

use super::corrections::require_reason;
use super::{Crossing, RideEngine, illegal};
use crate::clock::Clock;
use crate::config::PlateModel;
use crate::error::RideError;
use crate::event::{Event, EventAction};
use crate::roster_view::{EntryId, EntrySnapshot, RosterView};
use crate::status::RideStatus;

impl<V: RosterView, C: Clock> RideEngine<V, C> {
    /// Move one pooled rider onto another team mid-ride.
    ///
    /// The rider's live laps, held card and credited cards travel with
    /// them, and the source entry's later laps renumber.
    ///
    /// # Errors
    ///
    /// Return [`RideError::EmptyReason`] when *reason* is blank,
    /// [`RideError::IllegalState`] when the ride is RUNNING and not
    /// stopped, is DRAFT or FINISHED, is not `rider_pooled`, or names the
    /// rider's own team, [`RideError::UnknownPlate`] when `rider_plate`
    /// names no rider, [`RideError::UnknownTeam`] when `to_team` names no
    /// team, and [`RideError::Roster`] when the roster refuses the move.
    #[expect(
        clippy::too_many_arguments,
        reason = "the move takes the rider, the destination team and the audit reason"
    )]
    pub fn move_rider(
        &mut self,
        rider_plate: &str,
        to_team: &str,
        reason: &str,
    ) -> Result<Event, RideError> {
        self.require_move_allowed(reason)?;
        let source = self.find_rider(rider_plate)?;
        let destination = self.require_team(to_team)?;
        if destination.id == source.id {
            return Err(illegal(format!(
                "rider {rider_plate} is already on team {to_team}"
            )));
        }
        let source_key = source.id;
        let destination_key = destination.id;
        self.roster
            .move_rider(rider_plate, &destination_key, reason)?;
        self.roster.mark_has_data(&destination_key);
        self.reattribute_rider(rider_plate, &source_key, &destination_key)?;
        Ok(self.append(Event {
            action: EventAction::MoveRider,
            payload: json!({
                "rider_plate": rider_plate,
                "from_key": source_key.0.clone(),
                "to_team": to_team,
                "to_key": destination_key.0.clone(),
                "reason": reason,
            }),
        }))
    }

    /// Move one pooled team member into their own solo entry.
    ///
    /// The extraction is gated exactly like [`Self::move_rider`], and the
    /// rider's data follows them onto the new entry's key.
    ///
    /// # Errors
    ///
    /// Return [`RideError::EmptyReason`] when *reason* is blank,
    /// [`RideError::IllegalState`] when the ride is RUNNING and not
    /// stopped, is DRAFT or FINISHED, or is not `rider_pooled`,
    /// [`RideError::UnknownPlate`] when `rider_plate` names no rider,
    /// [`RideError::NotPooledTeamMember`] when the rider is not on a
    /// pooled team, and [`RideError::Roster`] when the roster refuses the
    /// extraction.
    pub fn extract_rider_to_solo(
        &mut self,
        rider_plate: &str,
        reason: &str,
    ) -> Result<Event, RideError> {
        self.require_move_allowed(reason)?;
        let source = self.find_rider(rider_plate)?;
        if !source.is_team {
            return Err(RideError::NotPooledTeamMember {
                plate: rider_plate.to_owned(),
            });
        }
        let source_key = source.id;
        let destination_key = self.roster.extract_rider_to_solo(rider_plate, reason)?;
        self.roster.mark_has_data(&destination_key);
        self.reattribute_rider(rider_plate, &source_key, &destination_key)?;
        Ok(self.append(Event {
            action: EventAction::ExtractRiderToSolo,
            payload: json!({
                "rider_plate": rider_plate,
                "from_key": source_key.0.clone(),
                "to_key": destination_key.0.clone(),
                "reason": reason,
            }),
        }))
    }

    /// Refuse a rider move that the ride's state or model forbids.
    ///
    /// # Errors
    ///
    /// Return [`RideError::EmptyReason`] when *reason* is blank,
    /// [`RideError::IllegalState`] when the ride is RUNNING and not
    /// stopped, is DRAFT or FINISHED, or is not `rider_pooled`.
    fn require_move_allowed(&self, reason: &str) -> Result<(), RideError> {
        require_reason(reason)?;
        if self.state == RideStatus::Running && !self.stopped {
            return Err(illegal("Stop the ride first"));
        }
        if self.state != RideStatus::Running && self.state != RideStatus::Reopened {
            return Err(illegal(format!(
                "cannot move a rider from {}",
                self.state.as_str()
            )));
        }
        if self.config.plate_model != PlateModel::RiderPooled {
            return Err(illegal(format!(
                "rider moves need a rider_pooled ride, not {}",
                self.config.plate_model.as_str()
            )));
        }
        Ok(())
    }

    /// Return the entry that holds the rider `rider_plate` names.
    ///
    /// # Errors
    ///
    /// Return [`RideError::UnknownPlate`] when no rider carries the plate.
    fn find_rider(&self, rider_plate: &str) -> Result<EntrySnapshot, RideError> {
        self.roster
            .entries()
            .into_iter()
            .find(|entry| {
                entry
                    .riders
                    .iter()
                    .any(|rider| rider.plate.as_deref() == Some(rider_plate))
            })
            .ok_or_else(|| RideError::UnknownPlate {
                plate: rider_plate.to_owned(),
            })
    }

    /// Return the team entry that *name* names.
    ///
    /// # Errors
    ///
    /// Return [`RideError::UnknownTeam`] when no team carries the name.
    fn require_team(&self, name: &str) -> Result<EntrySnapshot, RideError> {
        self.roster
            .entries()
            .into_iter()
            .find(|entry| entry.is_team && entry.display_name == name)
            .ok_or_else(|| RideError::UnknownTeam {
                name: name.to_owned(),
            })
    }

    /// Re-key one rider's data from *source* to *destination*.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when a restored voided lap needs
    /// the ride's start instant and the ride never started.
    #[expect(
        clippy::too_many_arguments,
        reason = "the core takes the rider plate and the two stable entry keys"
    )]
    pub(super) fn reattribute_rider(
        &mut self,
        rider_plate: &str,
        source: &EntryId,
        destination: &EntryId,
    ) -> Result<(), RideError> {
        self.reattribute_hand(rider_plate, source, destination);
        self.reattribute_live_laps(rider_plate, source, destination);
        self.reattribute_voided_laps(rider_plate, destination)
    }

    /// Move the rider's tagged credited cards to the destination hand.
    ///
    /// A card with no rider tag, and a teammate's tagged card, stay put.
    #[expect(
        clippy::too_many_arguments,
        reason = "the pass takes the rider plate and the two stable entry keys"
    )]
    fn reattribute_hand(&mut self, rider_plate: &str, source: &EntryId, destination: &EntryId) {
        let Some(hand) = self.hand.remove(source) else {
            return;
        };
        let mut staying = Vec::new();
        let mut travelling = Vec::new();
        for slot in hand {
            if slot.1.as_deref() == Some(rider_plate) {
                travelling.push(slot);
            } else {
                staying.push(slot);
            }
        }
        if travelling.is_empty() {
            self.hand.insert(source.clone(), staying);
            return;
        }
        self.hand.insert(source.clone(), staying);
        self.hand
            .entry(destination.clone())
            .or_default()
            .extend(travelling);
    }

    /// Move the rider's live laps, their cards and holds, to the destination.
    ///
    /// The source's later laps renumber last, in descending order, so each
    /// call closes a single gap and the survivors stay contiguous.
    #[expect(
        clippy::too_many_arguments,
        reason = "the pass takes the rider plate and the two stable entry keys"
    )]
    fn reattribute_live_laps(
        &mut self,
        rider_plate: &str,
        source: &EntryId,
        destination: &EntryId,
    ) {
        let moved: Vec<Crossing> = self
            .crossings
            .iter()
            .filter(|crossing| {
                &crossing.entry_id == source && crossing.rider_plate.as_deref() == Some(rider_plate)
            })
            .cloned()
            .collect();
        let mut removed = Vec::new();
        for crossing in moved {
            let card = self.dealt.remove(&crossing.deal_seq);
            let held = self.held.remove(&crossing.deal_seq);
            self.remove_crossing(&crossing);
            removed.push(crossing.seq);
            let replacement = Crossing {
                entry_id: destination.clone(),
                seq: self.next_lap_number(destination),
                crossed_at: crossing.crossed_at,
                rider_plate: Some(rider_plate.to_owned()),
                deal_seq: crossing.deal_seq,
            };
            self.insert_crossing(&replacement);
            if let Some(card) = card {
                self.dealt.insert(replacement.deal_seq, card);
            }
            if let Some(held_card) = held {
                self.held.insert(replacement.deal_seq, held_card);
            }
        }
        for seq in removed.iter().rev() {
            self.renumber_later(source, *seq);
        }
    }

    /// Restore the rider's own voided laps into the destination.
    ///
    /// A `void_crossing` void of this rider is reversed: the card is
    /// un-voided and the lap re-appends, with its short-lap disposition
    /// derived again against its new predecessor.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when a restored lap is the
    /// destination's first and the ride never started.
    fn reattribute_voided_laps(
        &mut self,
        rider_plate: &str,
        destination: &EntryId,
    ) -> Result<(), RideError> {
        let restored: Vec<(u64, Crossing)> = self
            .voided
            .iter()
            .filter_map(|(&deal_seq, voided)| {
                let crossing = voided.crossing.as_ref()?;
                (crossing.rider_plate.as_deref() == Some(rider_plate))
                    .then(|| (deal_seq, crossing.clone()))
            })
            .collect();
        for (deal_seq, crossing) in restored {
            let Some(voided) = self.voided.remove(&deal_seq) else {
                continue;
            };
            let replacement = Crossing {
                entry_id: destination.clone(),
                seq: self.next_lap_number(destination),
                crossed_at: crossing.crossed_at,
                rider_plate: Some(rider_plate.to_owned()),
                deal_seq,
            };
            self.insert_crossing(&replacement);
            self.dealt.insert(deal_seq, voided.card.clone());
            let previous = self.preceding_instant(&replacement)?;
            let lap_time = (replacement.crossed_at - previous).as_seconds_f64();
            if self.config.hold_short_laps && lap_time < f64::from(self.config.min_lap_s) {
                self.held.insert(deal_seq, voided.card);
            } else {
                self.credit(destination, deal_seq, rider_plate);
            }
        }
        Ok(())
    }
}
