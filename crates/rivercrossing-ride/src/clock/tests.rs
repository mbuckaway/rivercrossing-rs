// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the wall-clock seam.

use time::OffsetDateTime;

use super::{Clock, SystemClock};

/// A fixed instant well before any test run, so `now` can only be later.
fn a_past_instant() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_600_000_000).expect("2020-09-13 is a valid instant")
}

#[test]
fn system_clock_reads_an_instant_after_the_past_reference() {
    let now = SystemClock.now();

    assert!(
        now > a_past_instant(),
        "the system clock must read the present, got {now}"
    );
}

#[test]
fn system_clock_is_a_zero_sized_unit_type() {
    assert_eq!(
        std::mem::size_of::<SystemClock>(),
        0,
        "the clock carries no state"
    );
}
