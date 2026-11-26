// SPDX-License-Identifier: GPL-3.0-only

//! The seeded "GORBA Test Ride #N" setup.
//!
//! One source of truth for the ride an operator gets when a throwaway or
//! demonstration ride has to be up quickly: the generated name
//! ([`next_test_ride_name`]), the GORBA reference ride's own venue, organizer,
//! scorer and timing, and the shoe and scoring shape that ride runs with
//! ([`build_test_ride_config`]).
//!
//! The rules follow the Python oracle's `ui/presenters/ride_defaults.py`. The
//! shoe, lap-length and tie-break defaults are not duplicated here: the ride
//! crate's own defaults and the standings crate's frozen order are passed
//! straight through, so the test ride and a hand-opened ride setup cannot
//! drift apart.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the EPIC-12d views wire these defaults in a later phase; phase 1 unit-tests them"
    )
)]

use rivercrossing_ride::{EntryMode, PlateModel, RideConfig, RideConfigParams, Timestamp};
use rivercrossing_standings::DEFAULT_TIEBREAK_ORDER;
use time::Date;

#[cfg(test)]
mod tests;

/// Every generated name starts with this exact prefix; the numbering keys off
/// it and nothing else.
pub(crate) const TEST_RIDE_NAME_PREFIX: &str = "GORBA Test Ride #";

/// The GORBA reference ride's venue (spec §6's 8 km Sea to Sky loop).
pub(crate) const DEFAULT_VENUE: &str = "Sea to Sky Gondola";

/// The GORBA reference ride's organizer.
pub(crate) const DEFAULT_ORGANIZER: &str = "GORBA";

/// The GORBA reference ride's scorer.
pub(crate) const DEFAULT_SCORER: &str = "K. Singh";

/// The planned duration: six hours.
pub(crate) const DEFAULT_PLANNED_DURATION_S: i32 = 21_600;

/// The shortest valid lap: eighteen minutes.
pub(crate) const DEFAULT_MIN_LAP_S: i32 = 1_080;

/// The largest team that the test ride seats (R-12's 2..10 bound leaves room
/// for five).
pub(crate) const DEFAULT_MAX_TEAM_SIZE: i32 = 5;

/// The closed-loop length in kilometres (the ride crate's own default).
const DEFAULT_LAP_KM: f64 = 8.0;

/// The stored spelling of the jokers mode that spends one budget per ride.
const JOKERS_TOTAL: &str = "total";

/// The oracle's default shoe: eight decks holding one joker each.
const DEFAULT_DECK_COUNT: i32 = 8;

/// The oracle's default joker count per deck or per shoe.
const DEFAULT_JOKERS_PER_DECK: i32 = 1;

/// Return the next free `"GORBA Test Ride #N"` name.
///
/// The highest number already in use among `existing` decides the next one, so
/// a deleted test ride's number is reused rather than skipped. A name that does
/// not carry [`TEST_RIDE_NAME_PREFIX`] exactly, or whose tail is not one or more
/// ASCII digits, is ignored. With nothing used, or nothing recognizable, the
/// count starts at `#1`.
#[must_use]
pub(crate) fn next_test_ride_name(existing: &[String]) -> String {
    let highest = existing
        .iter()
        .filter_map(|name| suffix_number(name))
        .max()
        .unwrap_or(0);
    format!("{TEST_RIDE_NAME_PREFIX}{}", highest.saturating_add(1))
}

/// Return the test-ride number that `name` carries, or `None`.
///
/// Only an exact prefix followed by one or more ASCII decimal digits counts,
/// so a tail in another script or carrying sign or space padding reads as
/// unnumbered. A tail too large for a 64-bit count reads as unnumbered too.
fn suffix_number(name: &str) -> Option<u64> {
    let suffix = name.strip_prefix(TEST_RIDE_NAME_PREFIX)?;
    if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    suffix.parse().ok()
}

/// Build the fixed test-ride config under the given identity.
///
/// Only the identity varies: the ride's own venue, organizer, scorer, loop,
/// timings, shoe and scoring shape are this module's defaults, so two test
/// rides built a week apart differ in name and date alone.
///
/// # Panics
///
/// Panics if the module's own fixed constants stop satisfying the ride
/// config's bounds. They are compile-time literals inside every bound
/// (`max_team_size` 5 in 2..=10, `jokers_per_deck` 1 in 0..=10, a positive
/// duration and a positive lap floor), so the build cannot fail.
#[must_use]
#[expect(
    clippy::expect_used,
    reason = "the fixed constants satisfy every ride-config bound, so the build cannot fail"
)]
pub(crate) fn build_test_ride_config(
    name: &str,
    event_date: Date,
    planned_start: Timestamp,
) -> RideConfig {
    RideConfig::new(RideConfigParams {
        name: name.to_owned(),
        event_date,
        venue: DEFAULT_VENUE.to_owned(),
        lap_km: DEFAULT_LAP_KM,
        organizer: DEFAULT_ORGANIZER.to_owned(),
        scorer: DEFAULT_SCORER.to_owned(),
        planned_start,
        planned_duration_s: DEFAULT_PLANNED_DURATION_S,
        min_lap_s: DEFAULT_MIN_LAP_S,
        entry_mode: EntryMode::Mixed,
        plate_model: PlateModel::RiderPooled,
        max_team_size: DEFAULT_MAX_TEAM_SIZE,
        deck_count: DEFAULT_DECK_COUNT,
        jokers_per_deck: DEFAULT_JOKERS_PER_DECK,
        jokers_mode: JOKERS_TOTAL.to_owned(),
        max_cards: None,
        tiebreak_order: DEFAULT_TIEBREAK_ORDER,
        logo_path: None,
        hold_short_laps: true,
    })
    .expect("the fixed test-ride constants satisfy every config bound")
}
