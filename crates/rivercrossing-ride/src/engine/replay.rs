// SPDX-License-Identifier: GPL-3.0-only

//! The replay seam: re-apply a persisted audit event onto the engine.
//!
//! Dispatch calls the matching mutation with the payload's own values. Every
//! subject that names an entry is located by its stable `entry_id`, never by
//! re-resolving a plate: a ride saved after a mid-ride rider move has
//! re-derived both teams' plates, so the plate typed on the night may no
//! longer name the entry the crossing belongs to.

use std::collections::HashMap;

use rivercrossing_cards::Card;
use serde_json::Value;
use time::PrimitiveDateTime;
use time::format_description::well_known::Iso8601;

use super::{Crossing, RideEngine, illegal};
use crate::clock::{Clock, Timestamp};
use crate::error::RideError;
use crate::event::{Event, EventAction};
use crate::roster_view::{EntryId, EntrySnapshot, RosterView};

impl<V: RosterView, C: Clock> RideEngine<V, C> {
    /// Replay one previously recorded event onto this engine.
    ///
    /// The store's replay seam: build a fresh DRAFT engine and apply every
    /// persisted event, oldest first, to reach the exact live state. A
    /// `start` row is re-applied through the gate-bypassing start, because a
    /// live start already cleared the readiness gates. The shoe's open and
    /// closed state, the held queue, the credited hands and the recorded
    /// tie-break draws all rebuild from their own rows.
    ///
    /// # Errors
    ///
    /// Return [`RideError::UnknownEntry`] when a payload key names no entry,
    /// and the matching mutation's error for an inconsistent event stream.
    pub fn apply(&mut self, event: &Event) -> Result<(), RideError> {
        self.dispatch(event)
    }

    /// Replay one event named by its stored action spelling.
    ///
    /// The audit table stores the action as text, so this seam is what a
    /// reader with a foreign or corrupted row meets.
    ///
    /// # Errors
    ///
    /// Return [`RideError::UnknownEventAction`] when *action* is not one of
    /// the ride's actions, and the same errors as [`Self::apply`].
    pub fn apply_action(&mut self, action: &str, payload: &Value) -> Result<(), RideError> {
        let parsed: EventAction = serde_json::from_value(Value::String(action.to_owned()))
            .map_err(|_| RideError::UnknownEventAction {
                action: action.to_owned(),
            })?;
        let event = Event {
            action: parsed,
            payload: payload.clone(),
        };
        self.dispatch(&event)
    }

    /// Dispatch one typed event to its matching mutation.
    #[expect(
        clippy::too_many_lines,
        reason = "the dispatch is the event vocabulary's own size, one arm per action"
    )]
    fn dispatch(&mut self, event: &Event) -> Result<(), RideError> {
        match event.action {
            EventAction::Start => {
                self.begin_start(payload_timestamp(&event.payload, "actual_start")?);
                Ok(())
            }
            EventAction::Continue => {
                self.start(None)?;
                Ok(())
            }
            EventAction::SetStartTime => {
                self.set_start_time(payload_timestamp(&event.payload, "actual_start")?)?;
                Ok(())
            }
            EventAction::RecordCrossing => {
                let entry =
                    self.require_entry_by_key(&payload_text(&event.payload, "entry_id")?)?;
                let plate = payload_text(&event.payload, "plate")?;
                let crossed_at = payload_timestamp(&event.payload, "crossed_at")?;
                self.record_crossing_into(&entry, &plate, crossed_at)
            }
            EventAction::ConfirmHeld => {
                let crossing = self.crossing_from(&event.payload)?;
                self.confirm_held(&crossing)?;
                Ok(())
            }
            EventAction::VoidHeld => {
                let crossing = self.crossing_from(&event.payload)?;
                self.void_held(&crossing)?;
                Ok(())
            }
            EventAction::ReturnToHeld => {
                let crossing = self.crossing_from(&event.payload)?;
                self.return_to_held(&crossing)?;
                Ok(())
            }
            EventAction::Undo => {
                self.undo_last()?;
                Ok(())
            }
            EventAction::DealManual => {
                let entry =
                    self.require_entry_by_key(&payload_text(&event.payload, "entry_id")?)?;
                let plate = payload_text(&event.payload, "plate")?;
                let reason = payload_text(&event.payload, "reason")?;
                self.deal_manual_into(&entry, &plate, &reason)?;
                Ok(())
            }
            EventAction::EditCrossing => {
                let entry_id = EntryId(payload_text(&event.payload, "entry_id")?);
                let seq = payload_u32(&event.payload, "seq")?;
                let crossed_at = payload_timestamp(&event.payload, "crossed_at")?;
                let reason = payload_text(&event.payload, "reason")?;
                self.edit_crossing(&entry_id, seq, crossed_at, &reason)?;
                Ok(())
            }
            EventAction::VoidCrossing => {
                let entry_id = EntryId(payload_text(&event.payload, "entry_id")?);
                let seq = payload_u32(&event.payload, "seq")?;
                let reason = payload_text(&event.payload, "reason")?;
                self.void_crossing(&entry_id, seq, &reason)?;
                Ok(())
            }
            EventAction::AddCrossingAt => {
                let entry =
                    self.require_entry_by_key(&payload_text(&event.payload, "entry_id")?)?;
                let plate = payload_text(&event.payload, "plate")?;
                let crossed_at = payload_timestamp(&event.payload, "crossed_at")?;
                let reason = payload_text(&event.payload, "reason")?;
                self.add_crossing_at_into(&entry, &plate, crossed_at, &reason)?;
                Ok(())
            }
            EventAction::RecordMiss => {
                let crossed_at = payload_timestamp(&event.payload, "crossed_at")?;
                let reason = payload_text(&event.payload, "reason")?;
                self.record_miss(crossed_at, &reason)?;
                Ok(())
            }
            EventAction::AssignPlateToMiss => {
                let entry =
                    self.require_entry_by_key(&payload_text(&event.payload, "entry_id")?)?;
                let miss = self.pending_miss(payload_u32(&event.payload, "miss_seq")?)?;
                let new_plate = payload_text(&event.payload, "new_plate")?;
                let reason = payload_text(&event.payload, "reason")?;
                self.assign_plate_to_miss_into(&entry, &miss, &new_plate, &reason)?;
                Ok(())
            }
            EventAction::Reassign => {
                let destination =
                    self.require_entry_by_key(&payload_text(&event.payload, "new_entry_id")?)?;
                let seq = payload_u32(&event.payload, "seq")?;
                let new_plate = payload_text(&event.payload, "new_plate")?;
                let reason = payload_text(&event.payload, "reason")?;
                self.reassign_crossing_into(&destination, seq, &new_plate, &reason)?;
                Ok(())
            }
            EventAction::Dnf => {
                let entry =
                    self.require_entry_by_key(&payload_text(&event.payload, "entry_id")?)?;
                let plate = payload_text(&event.payload, "plate")?;
                let rider = event
                    .payload
                    .get("rider")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let reason = payload_text(&event.payload, "reason")?;
                self.record_dnf(&entry, &plate, rider, &reason);
                Ok(())
            }
            EventAction::VoidCard => {
                let entry =
                    self.require_entry_by_key(&payload_text(&event.payload, "entry_id")?)?;
                let card = Card::parse(&payload_text(&event.payload, "card")?)
                    .map_err(|error| illegal(error.to_string()))?;
                let reason = payload_text(&event.payload, "reason")?;
                self.void_card_into(&entry, &card, &reason)?;
                Ok(())
            }
            EventAction::MoveRider | EventAction::ExtractRiderToSolo => {
                let rider_plate = payload_text(&event.payload, "rider_plate")?;
                let source = EntryId(payload_text(&event.payload, "from_key")?);
                let destination = EntryId(payload_text(&event.payload, "to_key")?);
                self.reattribute_rider(&rider_plate, &source, &destination)?;
                self.append(event.clone());
                Ok(())
            }
            EventAction::Stop => {
                self.stop()?;
                Ok(())
            }
            EventAction::Finish => {
                let finished_at = payload_timestamp(&event.payload, "finished_at")?;
                let checks = payload_strings(&event.payload, "self_test_failed_checks");
                self.finish_at(finished_at, &checks)?;
                Ok(())
            }
            EventAction::Reopen => {
                self.reopen_at(payload_timestamp(&event.payload, "reopened_at")?)?;
                Ok(())
            }
            EventAction::TiebreakDraw => {
                self.tiebreak = payload_draws(&event.payload)?;
                self.append(event.clone());
                Ok(())
            }
            EventAction::ShoeReshuffle => Ok(()),
        }
    }

    /// Return the recorded crossing that an event's entry and seq name.
    ///
    /// # Errors
    ///
    /// Return [`RideError::NoCrossing`] when the engine recorded no such
    /// crossing.
    fn crossing_from(&self, payload: &Value) -> Result<Crossing, RideError> {
        let entry_id = EntryId(payload_text(payload, "entry_id")?);
        let seq = payload_u32(payload, "seq")?;
        self.require_crossing(&entry_id, seq)
    }

    /// Return the entry that *key* names.
    ///
    /// # Errors
    ///
    /// Return [`RideError::UnknownEntry`] when no entry carries the key.
    fn require_entry_by_key(&self, key: &str) -> Result<EntrySnapshot, RideError> {
        self.roster
            .entry(&EntryId(key.to_owned()))
            .ok_or_else(|| RideError::UnknownEntry {
                key: key.to_owned(),
            })
    }
}

/// Read one payload value as text.
///
/// A JSON string reads as itself; any other value reads as its JSON text, so
/// a stored number round-trips as the digits the audit row showed.
///
/// # Errors
///
/// Return [`RideError::IllegalState`] when the payload holds no such key.
fn payload_text(payload: &Value, key: &str) -> Result<String, RideError> {
    match payload.get(key) {
        Some(Value::String(text)) => Ok(text.clone()),
        Some(other) => Ok(other.to_string()),
        None => Err(illegal(format!("payload has no {key}"))),
    }
}

/// Read one payload value as an ISO-8601 instant.
///
/// # Errors
///
/// Return [`RideError::IllegalState`] when the key is absent or does not
/// parse as a naive ISO-8601 date-time.
fn payload_timestamp(payload: &Value, key: &str) -> Result<Timestamp, RideError> {
    let text = payload_text(payload, key)?;
    PrimitiveDateTime::parse(&text, &Iso8601::DEFAULT)
        .map(PrimitiveDateTime::assume_utc)
        .map_err(|_| illegal(format!("invalid {key}: {text}")))
}

/// Read one payload value as an unsigned count.
///
/// The corrections' `seq` and `miss_seq` keys are numbers live but may be
/// strings on disk, so both spellings read back.
///
/// # Errors
///
/// Return [`RideError::IllegalState`] when the key is absent, is not a
/// number or a numeric string, or exceeds `u32`.
fn payload_u32(payload: &Value, key: &str) -> Result<u32, RideError> {
    let Some(value) = payload.get(key) else {
        return Err(illegal(format!("payload has no {key}")));
    };
    if let Some(number) = value.as_u64() {
        return u32::try_from(number).map_err(|_| illegal(format!("{key} is out of range")));
    }
    if let Some(text) = value.as_str() {
        return text
            .parse::<u32>()
            .map_err(|_| illegal(format!("invalid {key}: {text}")));
    }
    Err(illegal(format!("invalid {key}")))
}

/// Read one optional string-list payload value.
///
/// A missing key and any value that is not a list read as an empty list.
/// Each list member reads as text, so a stored number becomes the string the
/// confirm dialog showed.
fn payload_strings(payload: &Value, key: &str) -> Vec<String> {
    let Some(Value::Array(items)) = payload.get(key) else {
        return Vec::new();
    };
    items.iter().map(payload_scalar).collect()
}

/// Read one payload scalar as text.
fn payload_scalar(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Read a `tiebreak_draw` payload's entry and card rows.
///
/// A missing key, or any value that is not a list of rows, reads as no draws
/// at all.
///
/// # Errors
///
/// Return [`RideError::IllegalState`] when a row's card code does not parse.
fn payload_draws(payload: &Value) -> Result<HashMap<EntryId, Card>, RideError> {
    let Some(Value::Array(rows)) = payload.get("draws") else {
        return Ok(HashMap::new());
    };
    let mut draws = HashMap::new();
    for row in rows {
        let Some(entry_id) = row.get("entry_id") else {
            continue;
        };
        let Some(code) = row.get("card") else {
            continue;
        };
        let card =
            Card::parse(&payload_scalar(code)).map_err(|error| illegal(error.to_string()))?;
        draws.insert(EntryId(payload_scalar(entry_id)), card);
    }
    Ok(draws)
}
