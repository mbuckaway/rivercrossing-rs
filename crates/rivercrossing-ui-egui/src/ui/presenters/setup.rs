// SPDX-License-Identifier: GPL-3.0-only

//! The ride-setup dialog's decisions: the two time-field parsers, their
//! inverse formatters, the New Ride defaults, and the submit validation.
//!
//! The rules follow the Python oracle's `ui/presenters/setup.py`, and every
//! message reproduces its copy.

use std::collections::BTreeMap;

use rivercrossing_ride::{RideConfig, RideConfigParams, setup_minimum_violations};
use time::{Date, Month, OffsetDateTime, Time};

use super::data_source::refusal_message;

#[cfg(test)]
mod tests;

/// The seconds in the duration field's hour.
const SECONDS_PER_HOUR: i64 = 3600;

/// The seconds in one minute.
const SECONDS_PER_MINUTE: i64 = 60;

/// The oracle's default deck count (spec §4).
const DEFAULT_DECKS: u32 = 8;

/// The oracle's default lap length in kilometres (the setup canvas).
const DEFAULT_LAP_KM: f64 = 8.0;

/// The oracle's default joker count per deck or per shoe.
const DEFAULT_JOKERS_PER_DECK: u32 = 1;

/// The oracle's default largest team (R-12).
const DEFAULT_TEAM_SIZE: u32 = 4;

/// The `cap_choice` item that stores no cap.
const CAP_DISABLED: &str = "Disabled";

/// The lowest stored card cap.
const CAP_MIN: u32 = 5;

/// The highest stored card cap.
const CAP_MAX: u32 = 20;

/// The jokers mode that spends a joker per deck.
const JOKERS_PER_DECK: &str = "per_deck";

/// The jokers mode that spends the shoe's jokers once.
const JOKERS_TOTAL: &str = "total";

/// The New Ride dialog's preloaded values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SetupDefaults {
    /// The shoe's deck count.
    pub(crate) decks: u32,
    /// The lap length in kilometres.
    pub(crate) lap_km: f64,
    /// The joker count per deck or per shoe.
    pub(crate) jokers_per_deck: u32,
    /// The largest team.
    pub(crate) team_size: u32,
}

/// Return the New Ride dialog's preloaded values.
#[must_use]
pub(crate) fn defaults() -> SetupDefaults {
    SetupDefaults {
        decks: DEFAULT_DECKS,
        lap_km: DEFAULT_LAP_KM,
        jokers_per_deck: DEFAULT_JOKERS_PER_DECK,
        team_size: DEFAULT_TEAM_SIZE,
    }
}

/// Parse the duration field's `H:MM` text into whole seconds.
///
/// # Errors
///
/// Return the blank message for a stripped-empty text, and the format
/// message for a text that is not exactly one `H:MM` pair of integers.
pub(crate) fn parse_duration(text: &str) -> Result<i64, String> {
    if text.trim().is_empty() {
        return Err("Duration is empty and must be completed".to_owned());
    }
    let malformed = || format!("Duration must be H:MM, got '{text}'");
    let (hours, minutes) = split_pair(text).ok_or_else(malformed)?;
    let hours = parse_number(hours).ok_or_else(malformed)?;
    let minutes = parse_number(minutes).ok_or_else(malformed)?;
    hours
        .checked_mul(SECONDS_PER_HOUR)
        .and_then(|hours| {
            minutes
                .checked_mul(SECONDS_PER_MINUTE)
                .and_then(|minutes| hours.checked_add(minutes))
        })
        .ok_or_else(malformed)
}

/// Render whole *seconds* as the duration field's `H:MM` text.
///
/// The field has no seconds column, so any remainder is dropped.
#[must_use]
pub(crate) fn format_duration(seconds: i64) -> String {
    let hours = seconds.div_euclid(SECONDS_PER_HOUR);
    let minutes = seconds.rem_euclid(SECONDS_PER_HOUR) / SECONDS_PER_MINUTE;
    format!("{hours}:{minutes:02}")
}

/// Parse the minimum-lap field's `M:SS` text into whole seconds.
///
/// # Errors
///
/// Return the blank message for a stripped-empty text, and the format
/// message for a text that is not exactly one `M:SS` pair of integers.
pub(crate) fn parse_min_lap(text: &str) -> Result<i64, String> {
    if text.trim().is_empty() {
        return Err("Min lap is blank and must be completed".to_owned());
    }
    let malformed = || format!("Min lap must be M:SS, got '{text}'");
    let (minutes, seconds) = split_pair(text).ok_or_else(malformed)?;
    let minutes = parse_number(minutes).ok_or_else(malformed)?;
    let seconds = parse_number(seconds).ok_or_else(malformed)?;
    minutes
        .checked_mul(SECONDS_PER_MINUTE)
        .and_then(|minutes| minutes.checked_add(seconds))
        .ok_or_else(malformed)
}

/// Render whole *seconds* as the minimum-lap field's `M:SS` text.
///
/// Minutes never fold into hours: the field is a lap's own duration.
#[must_use]
pub(crate) fn format_min_lap(seconds: i64) -> String {
    let minutes = seconds.div_euclid(SECONDS_PER_MINUTE);
    let remainder = seconds.rem_euclid(SECONDS_PER_MINUTE);
    format!("{minutes}:{remainder:02}")
}

/// Build the ride-setup parameters from the dialog's field values.
///
/// Every field is optional: an absent text field reads blank, an absent numeric field keeps its
/// [`defaults`] value, and an absent radio reads as its unchecked state. A malformed date, time,
/// duration, min lap, number or card cap is reported, and every message is collected so the
/// infobar names them together.
///
/// # Errors
///
/// Return the collected messages when any field does not parse.
pub(crate) fn params_from_fields(
    fields: &BTreeMap<String, String>,
) -> Result<RideConfigParams, Vec<String>> {
    let mut errors = Vec::new();
    let parsed = ParsedForm {
        date: date_field(fields, &mut errors),
        start: time_field(fields, &mut errors),
        duration: duration_field(fields, &mut errors),
        min_lap: min_lap_field(fields, &mut errors),
        lap_km: float_field(
            &NumberField {
                text: text(fields, "lap_km_spin"),
                label: "Lap length",
            },
            DEFAULT_LAP_KM,
            &mut errors,
        ),
        team_size: whole_field(
            &NumberField {
                text: text(fields, "team_size_spin"),
                label: "Team size",
            },
            whole_default(defaults().team_size),
            &mut errors,
        ),
        decks: whole_field(
            &NumberField {
                text: text(fields, "decks_spin"),
                label: "Decks",
            },
            whole_default(defaults().decks),
            &mut errors,
        ),
        jokers: whole_field(
            &NumberField {
                text: text(fields, "jokers_spin"),
                label: "Jokers",
            },
            whole_default(defaults().jokers_per_deck),
            &mut errors,
        ),
        cap: cap_field(fields, &mut errors),
    };
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(parsed.into_params(fields))
}

/// The field values that parsed, before the radios and the text fields join them.
struct ParsedForm {
    /// The event date, when it parsed.
    date: Option<Date>,
    /// The planned start time, when it parsed.
    start: Option<Time>,
    /// The planned duration in seconds, when it parsed.
    duration: Option<i32>,
    /// The minimum lap in seconds, when it parsed.
    min_lap: Option<i32>,
    /// The lap length in kilometres.
    lap_km: f64,
    /// The largest team.
    team_size: i32,
    /// The shoe's deck count.
    decks: i32,
    /// The joker count per deck or per shoe.
    jokers: i32,
    /// The card cap, when one is set.
    cap: Option<u32>,
}

impl ParsedForm {
    /// Return the ride parameters that the parsed values and the radios build.
    fn into_params(self, fields: &BTreeMap<String, String>) -> RideConfigParams {
        let date = self
            .date
            .unwrap_or_else(|| OffsetDateTime::UNIX_EPOCH.date());
        let start = self.start.unwrap_or(Time::MIDNIGHT);
        RideConfigParams {
            name: text(fields, "name_input").to_owned(),
            event_date: date,
            venue: text(fields, "venue_input").to_owned(),
            lap_km: self.lap_km,
            organizer: text(fields, "organizer_input").to_owned(),
            scorer: text(fields, "scorer_input").to_owned(),
            planned_start: OffsetDateTime::new_utc(date, start),
            planned_duration_s: self.duration.unwrap_or(0),
            min_lap_s: self.min_lap.unwrap_or(0),
            entry_mode: if truthy(fields, "mixed_radio") {
                rivercrossing_ride::EntryMode::Mixed
            } else {
                rivercrossing_ride::EntryMode::Solo
            },
            plate_model: if truthy(fields, "relay_radio") {
                rivercrossing_ride::PlateModel::TeamRelay
            } else {
                rivercrossing_ride::PlateModel::RiderPooled
            },
            max_team_size: self.team_size,
            deck_count: self.decks,
            jokers_per_deck: self.jokers,
            jokers_mode: if truthy(fields, "jokers_per_deck_radio") {
                JOKERS_PER_DECK.to_owned()
            } else {
                JOKERS_TOTAL.to_owned()
            },
            max_cards: self.cap,
            hold_short_laps: !truthy(fields, "always_deal_radio"),
            ..RideConfigParams::default()
        }
    }
}

/// Return the dialog field's text, or a blank string when it is unset.
fn text<'a>(fields: &'a BTreeMap<String, String>, id: &str) -> &'a str {
    fields.get(id).map_or("", String::as_str)
}

/// Parse the date field, reporting its message.
fn date_field(fields: &BTreeMap<String, String>, errors: &mut Vec<String>) -> Option<Date> {
    parse_date(text(fields, "date_picker")).map_or_else(
        |message| {
            errors.push(message);
            None
        },
        Some,
    )
}

/// Parse the start-time field, reporting its message.
fn time_field(fields: &BTreeMap<String, String>, errors: &mut Vec<String>) -> Option<Time> {
    parse_time(text(fields, "start_time_picker")).map_or_else(
        |message| {
            errors.push(message);
            None
        },
        Some,
    )
}

/// Parse the duration field, reporting its message.
fn duration_field(fields: &BTreeMap<String, String>, errors: &mut Vec<String>) -> Option<i32> {
    match parse_duration(text(fields, "duration_input")) {
        Ok(seconds) => whole_seconds(seconds, "Duration", errors),
        Err(message) => {
            errors.push(message);
            None
        }
    }
}

/// Parse the minimum-lap field, reporting its message.
fn min_lap_field(fields: &BTreeMap<String, String>, errors: &mut Vec<String>) -> Option<i32> {
    match parse_min_lap(text(fields, "min_lap_input")) {
        Ok(seconds) => whole_seconds(seconds, "Min lap", errors),
        Err(message) => {
            errors.push(message);
            None
        }
    }
}

/// Read the card-cap choice, reporting its message; `None` is the disabled choice.
fn cap_field(fields: &BTreeMap<String, String>, errors: &mut Vec<String>) -> Option<u32> {
    match parse_cap(text(fields, "cap_choice")) {
        Ok(cap) => cap,
        Err(message) => {
            errors.push(message);
            None
        }
    }
}

/// Validate the form and return the config the store should persist.
///
/// # Errors
///
/// Return the bound violation, or the joined minimum-setup reasons, when the form is refused.
pub(crate) fn validated_config(params: &RideConfigParams) -> Result<RideConfig, Vec<String>> {
    let config = RideConfig::new(params.clone()).map_err(|error| vec![refusal_message(&error)])?;
    let violations = setup_minimum_violations(&config);
    if violations.is_empty() {
        Ok(config)
    } else {
        Err(vec![violations.join("; ")])
    }
}

/// Parse a date field's `YYYY-MM-DD` text.
///
/// # Errors
///
/// Return the blank message for a stripped-empty text, and the format message otherwise.
pub(crate) fn parse_date(text: &str) -> Result<Date, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("Date is empty and must be completed".to_owned());
    }
    let malformed = || format!("Date must be YYYY-MM-DD, got '{text}'");
    let (year, month, day) = split_date(trimmed).ok_or_else(malformed)?;
    let year = year.parse::<i32>().map_err(|_| malformed())?;
    let month = month
        .parse::<u8>()
        .ok()
        .and_then(|m| Month::try_from(m).ok());
    let day = day.parse::<u8>().map_err(|_| malformed())?;
    let month = month.ok_or_else(malformed)?;
    Date::from_calendar_date(year, month, day).map_err(|_| malformed())
}

/// Parse a start-time field's `HH:MM` text.
///
/// # Errors
///
/// Return the blank message for a stripped-empty text, and the format message otherwise.
pub(crate) fn parse_time(text: &str) -> Result<Time, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("Start time is empty and must be completed".to_owned());
    }
    let malformed = || format!("Start time must be HH:MM, got '{text}'");
    let (hours, minutes) = split_pair(trimmed).ok_or_else(malformed)?;
    let hours = hours.parse::<u8>().map_err(|_| malformed())?;
    let minutes = minutes.parse::<u8>().map_err(|_| malformed())?;
    Time::from_hms(hours, minutes, 0).map_err(|_| malformed())
}

/// Return the messages that refuse one setup submission.
///
/// An empty list accepts the submission. A bound violation from
/// [`RideConfig::new`] is the whole refusal. Otherwise the minimum-setup
/// reasons join into one message with `"; "`, the same join the Python
/// presenter shows.
#[must_use]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the refusal-list API the setup presenter tests pin; the dialog stores validated_config's error"
    )
)]
pub(crate) fn submit_errors(params: &RideConfigParams) -> Vec<String> {
    validated_config(params).err().unwrap_or_default()
}

/// Return whether the dialog field holds the radio's checked value.
fn truthy(fields: &BTreeMap<String, String>, id: &str) -> bool {
    fields.get(id).is_some_and(|value| value == "true")
}

/// One numeric dialog field: its text and the label its message names.
struct NumberField<'a> {
    /// The field's text.
    text: &'a str,
    /// The label the refusal names.
    label: &'a str,
}

/// Return a whole field's value, or *default* when the field is blank.
///
/// A value that does not parse is reported and leaves *default*.
fn whole_field(field: &NumberField<'_>, default: i32, errors: &mut Vec<String>) -> i32 {
    let trimmed = field.text.trim();
    if trimmed.is_empty() {
        return default;
    }
    if let Ok(value) = trimmed.parse::<i32>() {
        value
    } else {
        errors.push(format!(
            "{} must be a number, got '{}'",
            field.label, field.text
        ));
        default
    }
}

/// Return a decimal field's value, or *default* when the field is blank.
///
/// A value that does not parse is reported and leaves *default*.
fn float_field(field: &NumberField<'_>, default: f64, errors: &mut Vec<String>) -> f64 {
    let trimmed = field.text.trim();
    if trimmed.is_empty() {
        return default;
    }
    if let Ok(value) = trimmed.parse::<f64>() {
        value
    } else {
        errors.push(format!(
            "{} must be a number, got '{}'",
            field.label, field.text
        ));
        default
    }
}

/// Convert parsed seconds to the config's whole count, or report a value past the bound.
fn whole_seconds(seconds: i64, label: &str, errors: &mut Vec<String>) -> Option<i32> {
    if let Ok(value) = i32::try_from(seconds) {
        Some(value)
    } else {
        errors.push(format!("{label} is too long"));
        None
    }
}

/// Convert a default whole count to the config's signed field.
fn whole_default(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

/// Read the card-cap choice: `Disabled` is no cap, else a number in 5..=20.
fn parse_cap(text: &str) -> Result<Option<u32>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed == CAP_DISABLED {
        return Ok(None);
    }
    trimmed
        .parse::<u32>()
        .ok()
        .filter(|value| (CAP_MIN..=CAP_MAX).contains(value))
        .map(Some)
        .ok_or_else(|| {
            format!(
                "Card cap must be '{CAP_DISABLED}' or a number {CAP_MIN}..{CAP_MAX}, got '{text}'"
            )
        })
}

/// Split *text* into exactly three hyphen-separated parts.
fn split_date(text: &str) -> Option<(&str, &str, &str)> {
    let mut parts = text.split('-');
    let year = parts.next()?;
    let month = parts.next()?;
    let day = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some((year, month, day))
}

/// Split *text* into exactly two colon-separated parts.
fn split_pair(text: &str) -> Option<(&str, &str)> {
    let mut parts = text.split(':');
    let first = parts.next()?;
    let second = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some((first, second))
}

/// Parse one time-field number, tolerating the whitespace that `int` accepts.
fn parse_number(text: &str) -> Option<i64> {
    text.trim().parse::<i64>().ok()
}
