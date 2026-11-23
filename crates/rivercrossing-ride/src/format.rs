// SPDX-License-Identifier: GPL-3.0-only

//! The ride clock's `h:mm:ss` reading.

/// Render *seconds* as the ride clock's `h:mm:ss` reading.
///
/// The function truncates a fractional second toward zero, so a recording
/// instant that is a whole second renders unchanged. It keeps the sign, so a
/// back-dated start reads `"-0:05:00"`. Hours are unbounded and not padded,
/// while minutes and seconds are two digits.
#[must_use]
pub fn format_elapsed(seconds: f64) -> String {
    let total = whole_seconds(seconds);
    let sign = if total < 0 { "-" } else { "" };
    let magnitude = total.unsigned_abs();
    let hours = magnitude / 3600;
    let minutes = magnitude % 3600 / 60;
    let secs = magnitude % 60;
    format!("{sign}{hours}:{minutes:02}:{secs:02}")
}

/// Truncate *seconds* toward zero.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the oracle truncates toward zero and an audit instant is a whole second"
)]
fn whole_seconds(seconds: f64) -> i64 {
    seconds.trunc() as i64
}

#[cfg(test)]
mod tests;
