// SPDX-License-Identifier: GPL-3.0-only

//! The ride-setup dialog's decisions: the two time-field parsers, their
//! inverse formatters, the New Ride defaults, and the submit validation.
//!
//! The rules follow the Python oracle's `ui/presenters/setup.py`, and every
//! message reproduces its copy.

use rivercrossing_ride::{RideConfig, RideConfigParams, setup_minimum_violations};

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

/// Return the messages that refuse one setup submission.
///
/// An empty list accepts the submission. A bound violation from
/// [`RideConfig::new`] is the whole refusal. Otherwise the minimum-setup
/// reasons join into one message with `"; "`, the same join the Python
/// presenter shows.
#[must_use]
pub(crate) fn submit_errors(params: &RideConfigParams) -> Vec<String> {
    let config = match RideConfig::new(params.clone()) {
        Ok(config) => config,
        Err(error) => return vec![refusal_message(&error)],
    };
    let violations = setup_minimum_violations(&config);
    if violations.is_empty() {
        return Vec::new();
    }
    vec![violations.join("; ")]
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
