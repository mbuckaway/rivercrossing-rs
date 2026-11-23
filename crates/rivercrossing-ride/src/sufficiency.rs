// SPDX-License-Identifier: GPL-3.0-only

//! The card-sufficiency advisory.
//!
//! The estimate and the check take `&[EntrySnapshot]` rather than a roster,
//! because both functions read only the entry and rider counts. This signature
//! keeps them pure, so the plan's roster stub is unnecessary.

use rivercrossing_cards::JokersMode;

use crate::config::{PlateModel, RideConfig};
use crate::roster_view::EntrySnapshot;

/// The number of natural cards in one deck.
const DECK_SIZE: u64 = 52;

/// The card-sufficiency verdict for one ride.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sufficiency {
    /// The shoe runs dry before the field stops drawing.
    NotEnough,
    /// The shoe holds enough cards for the field.
    Ok,
    /// The shoe holds more than twice the demand.
    FarTooMany,
}

impl Sufficiency {
    /// Return the machine-readable spelling of this verdict.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::NotEnough => "not_enough",
            Self::Ok => "ok",
            Self::FarTooMany => "far_too_many",
        }
    }
}

/// One ride's shoe size against its estimated card demand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardCheck {
    /// The shoe's capacity at the ride's jokers mode.
    pub shoe_cards: u32,
    /// The crossing count that the estimate predicts.
    pub expected: u32,
    /// The verdict for this shoe and demand.
    pub verdict: Sufficiency,
}

/// Estimate how many cards the field will draw.
///
/// One card per accepted crossing, so the estimate is the crossing count that
/// an average-speed field of this size records. The count is the planned
/// duration divided by the seconds of one lap at `avg_speed_kmh`, rounded up,
/// then multiplied by the draw units: every rider on a rider-pooled ride, or
/// every entry on a team-relay ride.
///
/// The function takes `&[EntrySnapshot]` rather than a roster, because it reads
/// only the entries and their riders. That keeps it pure, so the plan's roster
/// stub is unnecessary.
///
/// The function returns [`None`] when `lap_km`, `avg_speed_kmh` or
/// `planned_duration_s` is not positive, when *entries* is empty, or when the
/// demand is above `u32`.
#[must_use]
pub fn estimate_cards_needed(
    config: &RideConfig,
    entries: &[EntrySnapshot],
    avg_speed_kmh: f64,
) -> Option<u32> {
    if config.lap_km <= 0.0
        || avg_speed_kmh <= 0.0
        || config.planned_duration_s == 0
        || entries.is_empty()
    {
        return None;
    }
    let lap_seconds = config.lap_km / avg_speed_kmh * 3600.0;
    let laps_per_entry = ceil_to_u32(f64::from(config.planned_duration_s) / lap_seconds)?;
    let draw_units = match config.plate_model {
        PlateModel::RiderPooled => entries.iter().map(|entry| entry.riders.len()).sum(),
        PlateModel::TeamRelay => entries.len(),
    };
    let draw_units = u32::try_from(draw_units).ok()?;
    draw_units.checked_mul(laps_per_entry)
}

/// Judge the ride's shoe against its estimated demand.
///
/// The shoe's capacity is `deck_count * 52 + jokers_per_deck` when the jokers
/// mode spends one budget, or `deck_count * (52 + jokers_per_deck)` when each
/// deck carries its own. A demand above the capacity is
/// [`Sufficiency::NotEnough`]; a capacity above twice the demand is
/// [`Sufficiency::FarTooMany`]; every value between is [`Sufficiency::Ok`].
///
/// The function returns [`None`] when [`estimate_cards_needed`] returns
/// [`None`], or when the capacity is above `u32`.
#[must_use]
pub fn check_card_sufficiency(
    config: &RideConfig,
    entries: &[EntrySnapshot],
    avg_speed_kmh: f64,
) -> Option<CardCheck> {
    let expected = estimate_cards_needed(config, entries, avg_speed_kmh)?;
    let shoe_cards = shoe_cards(config)?;
    let verdict = if expected > shoe_cards {
        Sufficiency::NotEnough
    } else if u64::from(shoe_cards) > 2 * u64::from(expected) {
        Sufficiency::FarTooMany
    } else {
        Sufficiency::Ok
    };
    Some(CardCheck {
        shoe_cards,
        expected,
        verdict,
    })
}

/// Return the shoe's capacity at the ride's jokers mode.
fn shoe_cards(config: &RideConfig) -> Option<u32> {
    let decks = u64::from(config.deck_count);
    let jokers = u64::from(config.jokers_per_deck);
    let cards = match config.jokers_mode {
        JokersMode::Total => decks.checked_mul(DECK_SIZE)?.checked_add(jokers)?,
        JokersMode::PerDeck => decks.checked_mul(DECK_SIZE + jokers)?,
    };
    u32::try_from(cards).ok()
}

/// Round a positive lap count up to `u32`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the finite, non-negative range check proves the ceiled value fits u32"
)]
fn ceil_to_u32(value: f64) -> Option<u32> {
    let ceiled = value.ceil();
    if !ceiled.is_finite() || ceiled > f64::from(u32::MAX) {
        return None;
    }
    Some(ceiled as u32)
}

#[cfg(test)]
mod tests;
