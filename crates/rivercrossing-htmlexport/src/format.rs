// SPDX-License-Identifier: GPL-3.0-only

//! The exporter's pure formatting functions.
//!
//! Every reading is a pure function of its input: no clock, no locale and no
//! timezone conversion. English names are hard-coded, so a pinned instant
//! renders byte-identically on every machine (R-62).

use base64::Engine as _;
use time::{Date, Month, OffsetDateTime, Weekday};

use crate::types::RideInfo;

/// Render *at* as the footer's `Generated HH:MM, Mon D YYYY` stamp.
///
/// The hour is 24-hour and zero-padded, the day is unpadded, and the month
/// abbreviation is the samples' own four-letter `Sept`. The instant's own
/// wall-clock fields are used as given.
#[must_use]
pub fn format_generated(at: OffsetDateTime) -> String {
    let month = month_abbreviation(at.month());
    format!(
        "Generated {:02}:{:02}, {month} {} {}",
        at.hour(),
        at.minute(),
        at.day(),
        at.year()
    )
}

/// Render *seconds* as the golden pages' clock text.
///
/// A half second rounds to even, as Python's `round()` does. The reading is
/// `H:MM:SS` at an hour or more and `M:SS` below it.
#[must_use]
pub fn format_duration(seconds: f64) -> String {
    let total = whole_seconds(seconds);
    if total >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            total / 3600,
            total % 3600 / 60,
            total % 60
        )
    } else {
        format!("{}:{:02}", total / 60, total % 60)
    }
}

/// Round *seconds* to whole seconds, with a half to the even neighbour.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the oracle rounds to a whole second and a lap reading is far below i64::MAX"
)]
fn whole_seconds(seconds: f64) -> i64 {
    seconds.round_ties_even() as i64
}

/// Render *day* as the golden pages' `Sunday September 20, 2026`.
#[must_use]
pub fn format_event_date(day: Date) -> String {
    format!(
        "{} {} {}, {}",
        weekday_name(day.weekday()),
        month_name(day.month()),
        day.day(),
        day.year()
    )
}

/// Compose the page's meta line from *ride*.
///
/// An integral loop length prints without a fraction (`8`), a fractional one
/// keeps it (`6.5`).
#[must_use]
pub fn format_meta(ride: &RideInfo) -> String {
    let event_date = format_event_date(ride.event_date);
    let lap_km = ride.lap_km;
    format!("{event_date} · {} · {lap_km} km loop", ride.venue)
}

/// Return *bytes* as the `data:image/png;base64,` URI the page embeds.
#[must_use]
pub fn png_data_uri(bytes: &[u8]) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    format!("data:image/png;base64,{encoded}")
}

/// Return the samples' month abbreviation of *month*.
const fn month_abbreviation(month: Month) -> &'static str {
    match month {
        Month::January => "Jan",
        Month::February => "Feb",
        Month::March => "Mar",
        Month::April => "Apr",
        Month::May => "May",
        Month::June => "Jun",
        Month::July => "Jul",
        Month::August => "Aug",
        Month::September => "Sept",
        Month::October => "Oct",
        Month::November => "Nov",
        Month::December => "Dec",
    }
}

/// Return the English name of *month*.
const fn month_name(month: Month) -> &'static str {
    match month {
        Month::January => "January",
        Month::February => "February",
        Month::March => "March",
        Month::April => "April",
        Month::May => "May",
        Month::June => "June",
        Month::July => "July",
        Month::August => "August",
        Month::September => "September",
        Month::October => "October",
        Month::November => "November",
        Month::December => "December",
    }
}

/// Return the English name of *weekday*.
const fn weekday_name(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Monday => "Monday",
        Weekday::Tuesday => "Tuesday",
        Weekday::Wednesday => "Wednesday",
        Weekday::Thursday => "Thursday",
        Weekday::Friday => "Friday",
        Weekday::Saturday => "Saturday",
        Weekday::Sunday => "Sunday",
    }
}

#[cfg(test)]
mod tests;
