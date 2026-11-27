// SPDX-License-Identifier: GPL-3.0-only

//! The correction dialogs' submissions and the Crossing Detail view-model.
//!
//! The rules and the copy follow the Python oracle's `ui/views/corrections.py`,
//! `ui/views/crossing_detail.py` and the correction routes in `ui/app.py`:
//! every form requires a reason, a typed DNF plate must be digits only, and
//! each submission returns the notice to post with the event the store
//! persists. The engine owns every refusal; this module never mutates a
//! crossing on its own.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the EPIC-12c views wire these decisions in a later phase; phase 1 unit-tests them"
    )
)]

use rivercrossing_cards::Card;
use rivercrossing_ride::{
    Crossing, EntryId, Event, RideEngine, RideStatus, SystemClock, Timestamp,
};
use rivercrossing_roster::enums::EntryType;
use rivercrossing_roster::roster::Roster;
use rivercrossing_roster::types::Entry;
use time::{Date, Time};

use super::data_source::format_duration;
use super::setup::parse_date;

#[cfg(test)]
mod tests;

/// The reason gate's message for a form without its own copy.
const REASON_REQUIRED: &str = "Enter a reason.";

/// The DNF form's reason gate's message.
const DNF_REASON_REQUIRED: &str = "Enter a reason for the DNF.";

/// The DNF form's blank-plate message.
const PLATE_REQUIRED: &str = "Enter the rider plate.";

/// The DNF form's non-digit-plate message.
const PLATE_DIGITS_ONLY: &str = "The plate must be digits only.";

/// The miss form's blank-plate message.
const MISS_PLATE_REQUIRED: &str = "Enter a plate to assign to this miss.";

/// The time field's blank message.
const TIME_EMPTY: &str = "Time is empty and must be completed";

/// The Team cell that a solo entry renders (the oracle's `SOLO_TEAM_TEXT`).
const SOLO_TEAM_TEXT: &str = "solo";

/// The Status cell of a crossing that is one half of a live duplicate pair.
const DUPLICATE_STATUS: &str = "Duplicate";

/// The Status cell of a crossing whose card waits for review.
const HELD_STATUS: &str = "Held - Review";

/// The Status cell of a crossing whose card is credited.
const CREDITED_STATUS: &str = "Credited";

/// The Status cell of a crossing whose card is out of the ride.
const VOID_STATUS: &str = "Void";

/// The joker's stored code.
const JOKER_CODE: &str = "JK";

/// The joker's canvas display text.
const JOKER_DISPLAY: &str = "JK★";

/// The seconds in one hour.
const SECONDS_PER_HOUR: u64 = 3600;

/// The seconds in one minute.
const SECONDS_PER_MINUTE: u64 = 60;

/// One accepted correction: the notice to post and the event to persist.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CorrectionOutcome {
    /// The status-bar or info-bar notice.
    pub(crate) notice: String,
    /// The event the store appends.
    pub(crate) event: Event,
}

/// The Crossing Detail dialog's read-only cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CrossingDetailModel {
    /// The rider who crossed, else the entry's name.
    pub(crate) rider: String,
    /// The team's name, or `solo`.
    pub(crate) team: String,
    /// The plate the operator typed.
    pub(crate) plate: String,
    /// The per-entry lap number.
    pub(crate) lap: String,
    /// The crossing instant, local `HH:MM:SS`.
    pub(crate) time: String,
    /// The derived lap time, `M:SS` below an hour.
    pub(crate) lap_time: String,
    /// The running total from the gun.
    pub(crate) total: String,
    /// The card's glyph, or `Void`.
    pub(crate) card: String,
    /// The card's disposition.
    pub(crate) held: String,
}

/// The Crossing Detail dialog's four correction gates.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each gate is an independent button state that the view renders separately"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CorrectionActions {
    /// Whether Edit Plate is enabled.
    pub(crate) edit: bool,
    /// Whether Edit Time is enabled.
    pub(crate) edit_time: bool,
    /// Whether Void Card is enabled.
    pub(crate) void_card: bool,
    /// Whether Delete is enabled.
    pub(crate) delete: bool,
}

// --------------------------------------------------------------- submits

/// Retime one crossing at *time* on the ride's event date.
///
/// # Errors
///
/// Return the reason gate's, the time field's or the engine's refusal.
#[expect(
    clippy::too_many_arguments,
    reason = "the form's own fields: the engine, entry, lap, time and reason"
)]
pub(crate) fn edit_crossing(
    engine: &mut RideEngine<Roster, SystemClock>,
    entry: &EntryId,
    seq: u32,
    time: &str,
    reason: &str,
) -> Result<CorrectionOutcome, String> {
    require_reason(reason)?;
    let crossed_at = parse_ride_time(engine, time)?;
    let event = engine
        .edit_crossing(entry, seq, crossed_at, reason.trim())
        .map_err(|error| format!("Could not edit crossing: {}", refusal(&error)))?;
    Ok(CorrectionOutcome {
        notice: "Crossing edited".to_owned(),
        event,
    })
}

/// Record one crossing for *plate* at *time* on the ride's event date.
///
/// # Errors
///
/// Return the reason gate's, the time field's or the engine's refusal.
#[expect(
    clippy::too_many_arguments,
    reason = "the form's own fields: the engine, plate, time and reason"
)]
pub(crate) fn add_crossing_at(
    engine: &mut RideEngine<Roster, SystemClock>,
    plate: &str,
    time: &str,
    reason: &str,
) -> Result<CorrectionOutcome, String> {
    require_reason(reason)?;
    let crossed_at = parse_ride_time(engine, time)?;
    let event = engine
        .add_crossing_at(plate.trim(), crossed_at, reason.trim())
        .map_err(|error| format!("Correction refused: {}", refusal(&error)))?;
    Ok(CorrectionOutcome {
        notice: "Crossing added".to_owned(),
        event,
    })
}

/// Deal one shoe card to *plate*'s entry.
///
/// # Errors
///
/// Return the reason gate's or the engine's refusal.
pub(crate) fn deal_manual(
    engine: &mut RideEngine<Roster, SystemClock>,
    plate: &str,
    reason: &str,
) -> Result<CorrectionOutcome, String> {
    require_reason(reason)?;
    let event = engine
        .deal_manual(plate.trim(), Some(reason.trim()))
        .map_err(|error| format!("Correction refused: {}", refusal(&error)))?;
    Ok(CorrectionOutcome {
        notice: "Card dealt".to_owned(),
        event,
    })
}

/// Mark the rider or entry that *plate* names as DNF.
///
/// The notice names the target from the event's own display sentence.
///
/// # Errors
///
/// Return the plate gate's, the reason gate's or the engine's refusal.
pub(crate) fn mark_dnf(
    engine: &mut RideEngine<Roster, SystemClock>,
    plate: &str,
    reason: &str,
) -> Result<CorrectionOutcome, String> {
    require_dnf_plate(plate)?;
    require_reason_with(reason, DNF_REASON_REQUIRED)?;
    let event = engine
        .mark_dnf(plate.trim(), reason.trim())
        .map_err(|error| format!("Correction refused: {}", refusal(&error)))?;
    Ok(CorrectionOutcome {
        notice: dnf_notice(&event),
        event,
    })
}

/// Return the DNF target the engine accepts: the entry the operator typed, resolved through the
/// roster, else the typed *plate*.
///
/// A team's own name, plate or key names the whole entry; a member's name or own plate names that
/// rider alone (R-15). Text that resolves to nothing is returned unchanged, so the engine's own
/// refusal names it.
pub(crate) fn dnf_target(roster: &Roster, plate: &str, entry: &str) -> String {
    let entry = entry.trim();
    if entry.is_empty() {
        return plate.trim().to_owned();
    }
    resolve_dnf_entry(roster, entry).unwrap_or_else(|| entry.to_owned())
}

/// Return the plate that *text* names in *roster*, when it names a rider or an entry.
fn resolve_dnf_entry(roster: &Roster, text: &str) -> Option<String> {
    for entry in roster.entries() {
        let rider = entry.riders.iter().find(|rider| {
            rider.full_name().eq_ignore_ascii_case(text)
                || rider
                    .plate
                    .as_deref()
                    .is_some_and(|plate| plate.eq_ignore_ascii_case(text))
        });
        if let Some(rider) = rider {
            return rider.plate.clone().or_else(|| Some(entry.plate.clone()));
        }
        if entry.display_name.eq_ignore_ascii_case(text)
            || entry.plate.eq_ignore_ascii_case(text)
            || entry.key.eq_ignore_ascii_case(text)
        {
            return Some(entry.plate.clone());
        }
    }
    None
}

/// Void one credited *card* from *entry*'s hand.
///
/// # Errors
///
/// Return the reason gate's or the engine's refusal.
#[expect(
    clippy::too_many_arguments,
    reason = "the form's own fields: the engine, entry, card and reason"
)]
pub(crate) fn void_card(
    engine: &mut RideEngine<Roster, SystemClock>,
    entry: &str,
    card: &Card,
    reason: &str,
) -> Result<CorrectionOutcome, String> {
    require_reason(reason)?;
    let event = engine
        .void_card(entry.trim(), card, reason.trim())
        .map_err(|error| format!("Could not void card: {}", refusal(&error)))?;
    Ok(CorrectionOutcome {
        notice: "Card voided".to_owned(),
        event,
    })
}

/// Back-date the ride's gun to *date* and *time*.
///
/// # Errors
///
/// Return the date field's, the time field's or the engine's refusal.
pub(crate) fn set_start(
    engine: &mut RideEngine<Roster, SystemClock>,
    date: &str,
    time: &str,
) -> Result<CorrectionOutcome, String> {
    let date = parse_date(date)?;
    let at = combine(date, parse_clock(time)?);
    let event = engine
        .set_start_time(at)
        .map_err(|error| format!("Correction refused: {}", refusal(&error)))?;
    Ok(CorrectionOutcome {
        notice: "Start time set".to_owned(),
        event,
    })
}

/// Resolve pending miss *seq* to *plate*.
///
/// # Errors
///
/// Return the plate gate's, the reason gate's or the engine's refusal.
#[expect(
    clippy::too_many_arguments,
    reason = "the form's own fields: the engine, miss number, plate and reason"
)]
pub(crate) fn assign_plate_to_miss(
    engine: &mut RideEngine<Roster, SystemClock>,
    seq: u32,
    plate: &str,
    reason: &str,
) -> Result<CorrectionOutcome, String> {
    if plate.trim().is_empty() {
        return Err(MISS_PLATE_REQUIRED.to_owned());
    }
    require_reason(reason)?;
    let event = engine
        .assign_plate_to_miss(seq, plate.trim(), reason.trim())
        .map_err(|error| format!("Could not assign: {}", refusal(&error)))?;
    Ok(CorrectionOutcome {
        notice: "Miss scored".to_owned(),
        event,
    })
}

// ------------------------------------------------------------ view-model

/// Return the Crossing Detail dialog's cells for *crossing*.
#[must_use]
pub(crate) fn crossing_detail_fields(
    crossing: &Crossing,
    engine: &RideEngine<Roster, SystemClock>,
    roster: &Roster,
) -> CrossingDetailModel {
    let entry = roster.entry_by_key(&crossing.entry_id.0);
    let entry_plate = entry.map_or_else(
        || crossing.rider_plate.clone().unwrap_or_default(),
        |entry| entry.plate.clone(),
    );
    let entry_name = entry.map_or_else(|| entry_plate.clone(), |entry| entry.display_name.clone());
    let held = engine.held_card_for(crossing);
    let card = held.clone().or_else(|| engine.card_for(crossing));
    let status = held_status(engine, crossing, held.is_some());
    let (lap_seconds, total_seconds) = lap_and_total(engine, crossing);
    CrossingDetailModel {
        rider: rider_name(entry, crossing.rider_plate.as_deref())
            .filter(|name| !name.is_empty())
            .unwrap_or(entry_name),
        team: team_name(entry, &entry_plate),
        plate: plate_text(crossing, entry),
        lap: crossing.seq.to_string(),
        time: clock_text(crossing.crossed_at),
        lap_time: lap_time_text(lap_seconds),
        total: format_duration(total_seconds),
        card: card_text_for(status, card.as_ref()),
        held: status.to_owned(),
    }
}

/// Return the Crossing Detail dialog's four correction gates.
#[must_use]
pub(crate) fn can_edit(
    engine: &RideEngine<Roster, SystemClock>,
    crossing: &Crossing,
) -> CorrectionActions {
    let live = matches!(engine.state(), RideStatus::Running | RideStatus::Reopened);
    CorrectionActions {
        edit: true,
        edit_time: true,
        void_card: void_card_available(engine, crossing),
        delete: live,
    }
}

// ------------------------------------------------------------- internals

/// Return whether the engine allows Void Card for *crossing*.
///
/// A credited card is voidable. A held card is the review surface's, except
/// on one half of a duplicate pair, where the operator's next step is to
/// delete a twin.
fn void_card_available(engine: &RideEngine<Roster, SystemClock>, crossing: &Crossing) -> bool {
    if is_duplicate(engine, crossing) {
        return true;
    }
    let credited = engine
        .card_for(crossing)
        .is_some_and(|card| engine.credited_cards(&crossing.entry_id).contains(&card));
    engine.held_card_for(crossing).is_none() && credited
}

/// Return whether *crossing* is one half of a live duplicate pair.
fn is_duplicate(engine: &RideEngine<Roster, SystemClock>, crossing: &Crossing) -> bool {
    engine
        .duplicate_crossings()
        .iter()
        .any(|(older, newer)| older == crossing || newer == crossing)
}

/// Return the Status cell for *crossing*.
fn held_status(
    engine: &RideEngine<Roster, SystemClock>,
    crossing: &Crossing,
    held: bool,
) -> &'static str {
    if is_duplicate(engine, crossing) {
        return DUPLICATE_STATUS;
    }
    if held {
        return HELD_STATUS;
    }
    let credited = engine
        .card_for(crossing)
        .is_some_and(|card| engine.credited_cards(&crossing.entry_id).contains(&card));
    if engine.is_card_voided(crossing) || !credited {
        return VOID_STATUS;
    }
    CREDITED_STATUS
}

/// Return the Card cell: the status word for a voided card, else the glyph.
fn card_text_for(status: &str, card: Option<&Card>) -> String {
    if status == VOID_STATUS {
        return VOID_STATUS.to_owned();
    }
    card.map_or_else(String::new, |card| card_text(&card.code()))
}

/// Return the rider who owns *`rider_plate`*, when the entry records one.
fn rider_name(entry: Option<&Entry>, rider_plate: Option<&str>) -> Option<String> {
    let entry = entry?;
    let plate = rider_plate?;
    entry
        .riders
        .iter()
        .find(|rider| rider.plate.as_deref() == Some(plate))
        .map(rivercrossing_roster::types::Rider::full_name)
}

/// Return the Team cell: the team's name, `solo`, or the entry's plate.
fn team_name(entry: Option<&Entry>, entry_plate: &str) -> String {
    match entry {
        None => entry_plate.to_owned(),
        Some(entry) if entry.kind == EntryType::Solo => SOLO_TEAM_TEXT.to_owned(),
        Some(entry) => entry.display_name.clone(),
    }
}

/// Return the Plate cell: the typed plate, else the entry's own.
fn plate_text(crossing: &Crossing, entry: Option<&Entry>) -> String {
    crossing
        .rider_plate
        .clone()
        .filter(|plate| !plate.is_empty())
        .or_else(|| entry.map(|entry| entry.plate.clone()))
        .unwrap_or_default()
}

/// Return the crossing's derived lap time and running total, in seconds.
fn lap_and_total(engine: &RideEngine<Roster, SystemClock>, crossing: &Crossing) -> (f64, f64) {
    let times = engine.lap_times(&crossing.entry_id);
    let Ok(seq) = usize::try_from(crossing.seq) else {
        return (0.0, 0.0);
    };
    if seq == 0 || seq > times.len() {
        return (0.0, 0.0);
    }
    let lap = times.get(seq - 1).copied().unwrap_or(0.0);
    (lap, times.iter().take(seq).sum())
}

/// Return a lap time as `M:SS`, or `H:MM:SS` at an hour or more.
fn lap_time_text(seconds: f64) -> String {
    let total = whole_seconds(seconds);
    if total >= SECONDS_PER_HOUR {
        return format_duration(seconds);
    }
    format!(
        "{}:{:02}",
        total / SECONDS_PER_MINUTE,
        total % SECONDS_PER_MINUTE
    )
}

/// Return *seconds* truncated to a whole count, with a negative clamped to zero.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is clamped to zero first, and a lap time is far below u64::MAX seconds"
)]
fn whole_seconds(seconds: f64) -> u64 {
    seconds.max(0.0).trunc() as u64
}

/// Return *at* rendered as local `HH:MM:SS`.
fn clock_text(at: Timestamp) -> String {
    format!("{:02}:{:02}:{:02}", at.hour(), at.minute(), at.second())
}

/// Return one card code's canvas display text.
///
/// A code whose last character names no suit renders unchanged, so a corrupt
/// stored code never panics in a view.
fn card_text(code: &str) -> String {
    if code == JOKER_CODE {
        return JOKER_DISPLAY.to_owned();
    }
    let mut chars = code.chars();
    let Some(suit) = chars.next_back() else {
        return code.to_owned();
    };
    let rank = chars.as_str();
    match suit {
        'S' => format!("{rank}♠"),
        'H' => format!("{rank}♥"),
        'D' => format!("{rank}♦"),
        'C' => format!("{rank}♣"),
        _ => code.to_owned(),
    }
}

/// Return the DNF notice, naming the target from the event's display line.
fn dnf_notice(event: &Event) -> String {
    let display = event
        .payload
        .get("display")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    format!("{display} — DNF")
}

/// Refuse a blank correction reason with the generic message.
fn require_reason(reason: &str) -> Result<(), String> {
    require_reason_with(reason, REASON_REQUIRED)
}

/// Refuse a blank correction reason with *message*.
fn require_reason_with(reason: &str, message: &str) -> Result<(), String> {
    if reason.trim().is_empty() {
        return Err(message.to_owned());
    }
    Ok(())
}

/// Refuse a DNF plate that is blank or is not ASCII digits.
fn require_dnf_plate(plate: &str) -> Result<(), String> {
    let plate = plate.trim();
    if plate.is_empty() {
        return Err(PLATE_REQUIRED.to_owned());
    }
    if !plate.is_ascii() || !plate.chars().all(|character| character.is_ascii_digit()) {
        return Err(PLATE_DIGITS_ONLY.to_owned());
    }
    Ok(())
}

/// Return the refusal text that the ride error carries.
///
/// The oracle's status copy quotes the message alone, not the error's kind
/// prefix.
fn refusal(error: &rivercrossing_ride::RideError) -> String {
    super::data_source::refusal_message(error)
}

/// Parse *text* as a clock time on the ride's event date.
fn parse_ride_time(
    engine: &RideEngine<Roster, SystemClock>,
    text: &str,
) -> Result<Timestamp, String> {
    Ok(combine(engine.config().event_date, parse_clock(text)?))
}

/// Parse `H:MM` or `H:MM:SS` text into a time of day.
///
/// # Errors
///
/// Return the blank message for a stripped-empty field, and the format
/// message for text that is not two or three colon-separated whole numbers.
fn parse_clock(text: &str) -> Result<Time, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(TIME_EMPTY.to_owned());
    }
    let malformed = || format!("Time must be H:MM:SS, got '{text}'");
    let mut parts = trimmed.split(':');
    let hours = parts.next().ok_or_else(malformed)?;
    let minutes = parts.next().ok_or_else(malformed)?;
    let seconds = parts.next().unwrap_or("0");
    if parts.next().is_some() {
        return Err(malformed());
    }
    Time::from_hms(
        hours.parse::<u8>().map_err(|_| malformed())?,
        minutes.parse::<u8>().map_err(|_| malformed())?,
        seconds.parse::<u8>().map_err(|_| malformed())?,
    )
    .map_err(|_| malformed())
}

/// Combine a calendar *date* and a *time* into one instant.
fn combine(date: Date, time: Time) -> Timestamp {
    time::OffsetDateTime::new_utc(date, time)
}
