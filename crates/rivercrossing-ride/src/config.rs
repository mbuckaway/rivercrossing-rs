// SPDX-License-Identifier: GPL-3.0-only

//! The ride setup types.
//!
//! [`RideConfigParams`] holds the raw values that the setup dialog submits.
//! [`RideConfig::new`] checks each spec-defined bound and returns the validated
//! [`RideConfig`], or the oracle's refusal message.

use std::path::PathBuf;
use std::str::FromStr;

use rivercrossing_cards::JokersMode;
use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};

use crate::clock::Timestamp;
use crate::error::RideError;

/// The smallest team size that the setup dialog accepts (R-12).
const MIN_TEAM_SIZE: i32 = 2;
/// The largest team size that the setup dialog accepts (R-12).
const MAX_TEAM_SIZE: i32 = 10;
/// The largest joker count per deck or per shoe (spec §4).
const MAX_JOKERS_PER_DECK: i32 = 10;
/// The oracle's default deck count (spec §4).
const DEFAULT_DECK_COUNT: i32 = 8;
/// The oracle's default joker count (spec §4).
const DEFAULT_JOKERS_PER_DECK: i32 = 1;
/// The stored spelling of the jokers mode that spends one budget per ride.
const DEFAULT_JOKERS_MODE: &str = "total";
/// The oracle's default tie-break order, most important first (R-14).
const DEFAULT_TIEBREAK_ORDER: [TieBreak; 3] =
    [TieBreak::HighCard, TieBreak::Laps, TieBreak::TotalTime];

/// One tie-break criterion, most important first.
///
/// The serde spelling is the stored spelling that the Python oracle uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TieBreak {
    /// The high-card draw decides.
    HighCard,
    /// Most laps wins.
    Laps,
    /// Lowest total time wins.
    TotalTime,
}

impl TieBreak {
    /// Return the stored spelling of this tie-break.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::HighCard => "high_card",
            Self::Laps => "laps",
            Self::TotalTime => "total_time",
        }
    }
}

impl FromStr for TieBreak {
    type Err = RideError;

    /// Read a stored tie-break spelling.
    ///
    /// # Errors
    ///
    /// Return [`RideError::Config`] when the text is not a known spelling.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "high_card" => Ok(Self::HighCard),
            "laps" => Ok(Self::Laps),
            "total_time" => Ok(Self::TotalTime),
            other => Err(unknown_value("tiebreak", other)),
        }
    }
}

/// A ride's team policy (spec S1, R-11).
///
/// The serde spelling is the stored spelling that the Python oracle uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryMode {
    /// Solo entries only; the team controls stay hidden.
    Solo,
    /// Solo and team entries.
    Mixed,
}

impl EntryMode {
    /// Return the stored spelling of this entry mode.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Solo => "solo",
            Self::Mixed => "mixed",
        }
    }
}

impl FromStr for EntryMode {
    type Err = RideError;

    /// Read a stored entry-mode spelling.
    ///
    /// # Errors
    ///
    /// Return [`RideError::Config`] when the text is not a known spelling.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "solo" => Ok(Self::Solo),
            "mixed" => Ok(Self::Mixed),
            other => Err(unknown_value("entry mode", other)),
        }
    }
}

/// A mixed ride's plate policy (spec S1, R-16).
///
/// The serde spelling is the stored spelling that the Python oracle uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlateModel {
    /// Every rider carries a plate, and the team pools their cards.
    RiderPooled,
    /// One plate per entry, with one rider on course at a time.
    TeamRelay,
}

impl PlateModel {
    /// Return the stored spelling of this plate model.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::RiderPooled => "rider_pooled",
            Self::TeamRelay => "team_relay",
        }
    }
}

impl FromStr for PlateModel {
    type Err = RideError;

    /// Read a stored plate-model spelling.
    ///
    /// # Errors
    ///
    /// Return [`RideError::Config`] when the text is not a known spelling.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "rider_pooled" => Ok(Self::RiderPooled),
            "team_relay" => Ok(Self::TeamRelay),
            other => Err(unknown_value("plate model", other)),
        }
    }
}

/// The raw values that the setup dialog submits.
///
/// The bounded numbers are signed, so [`RideConfig::new`] can refuse a negative
/// value with the oracle's exact message before it stores an unsigned count.
/// The `jokers_mode` text is parsed the same way.
///
/// [`Default`] holds the oracle's defaults for the eight optional values. The
/// fields that the oracle requires have no oracle default, so their values are
/// placeholders that a caller must replace.
#[derive(Debug, Clone, PartialEq)]
pub struct RideConfigParams {
    /// The ride name.
    pub name: String,
    /// The event date.
    pub event_date: Date,
    /// The venue.
    pub venue: String,
    /// The lap length in kilometres.
    pub lap_km: f64,
    /// The organiser.
    pub organizer: String,
    /// The scorer.
    pub scorer: String,
    /// The planned start instant.
    pub planned_start: Timestamp,
    /// The planned duration in seconds; must be positive.
    pub planned_duration_s: i32,
    /// The shortest valid lap in seconds; must be positive.
    pub min_lap_s: i32,
    /// The ride's team policy.
    pub entry_mode: EntryMode,
    /// The ride's plate policy.
    pub plate_model: PlateModel,
    /// The largest team; must be in 2..10.
    pub max_team_size: i32,
    /// The shoe's deck count; must be at least 1.
    pub deck_count: i32,
    /// The joker count per deck or per shoe; must be in 0..10.
    pub jokers_per_deck: i32,
    /// The stored jokers-mode spelling.
    pub jokers_mode: String,
    /// The cap on the cards that an entry's hand scores, when one is set.
    pub max_cards: Option<u32>,
    /// The tie-break order, most important first.
    pub tiebreak_order: [TieBreak; 3],
    /// The chosen logo file, when the operator picked one.
    pub logo_path: Option<PathBuf>,
    /// True when a short lap holds its card for review.
    pub hold_short_laps: bool,
}

impl Default for RideConfigParams {
    fn default() -> Self {
        Self {
            name: String::new(),
            event_date: OffsetDateTime::UNIX_EPOCH.date(),
            venue: String::new(),
            lap_km: 0.0,
            organizer: String::new(),
            scorer: String::new(),
            planned_start: OffsetDateTime::UNIX_EPOCH,
            planned_duration_s: 1,
            min_lap_s: 1,
            entry_mode: EntryMode::Solo,
            plate_model: PlateModel::RiderPooled,
            max_team_size: 4,
            deck_count: DEFAULT_DECK_COUNT,
            jokers_per_deck: DEFAULT_JOKERS_PER_DECK,
            jokers_mode: DEFAULT_JOKERS_MODE.to_owned(),
            max_cards: None,
            tiebreak_order: DEFAULT_TIEBREAK_ORDER,
            logo_path: None,
            hold_short_laps: true,
        }
    }
}

/// One ride's setup-time settings (spec §2 `ride` row).
///
/// Build the config with [`RideConfig::new`], which checks each bound. A struct
/// literal skips that check, so production code uses the constructor.
#[derive(Debug, Clone, PartialEq)]
pub struct RideConfig {
    /// The ride name.
    pub name: String,
    /// The event date, a calendar day with no clock part.
    ///
    /// The oracle stores a `date`, so [`Date`] round-trips it exactly.
    pub event_date: Date,
    /// The venue.
    pub venue: String,
    /// The lap length in kilometres.
    pub lap_km: f64,
    /// The organiser.
    pub organizer: String,
    /// The scorer.
    pub scorer: String,
    /// The planned start instant.
    ///
    /// The oracle combines the dialog's date and time pickers into one naive
    /// datetime. The port stores instants on the [`Timestamp`] seam in UTC, so
    /// the start round-trips through the ride table's epoch seconds.
    pub planned_start: Timestamp,
    /// The planned duration in seconds.
    pub planned_duration_s: u32,
    /// The shortest valid lap in seconds.
    pub min_lap_s: u32,
    /// The ride's team policy.
    pub entry_mode: EntryMode,
    /// The ride's plate policy.
    pub plate_model: PlateModel,
    /// The largest team.
    pub max_team_size: u32,
    /// The shoe's deck count.
    pub deck_count: u32,
    /// The joker count per deck or per shoe.
    pub jokers_per_deck: u32,
    /// Whether the jokers belong to each deck or to the whole shoe.
    pub jokers_mode: JokersMode,
    /// The cap on the cards that an entry's hand scores, when one is set.
    pub max_cards: Option<u32>,
    /// The tie-break order, most important first.
    pub tiebreak_order: [TieBreak; 3],
    /// The chosen logo file, when the operator picked one.
    pub logo_path: Option<PathBuf>,
    /// True when a short lap holds its card for review.
    pub hold_short_laps: bool,
}

impl RideConfig {
    /// Build a ride config, checking each spec-defined bound.
    ///
    /// The checks run in the oracle's order.
    ///
    /// # Errors
    ///
    /// Return [`RideError::Config`] when a setup value is outside its bound.
    pub fn new(params: RideConfigParams) -> Result<Self, RideError> {
        let max_team_size = bounded_count(
            "max_team_size",
            params.max_team_size,
            MIN_TEAM_SIZE..=MAX_TEAM_SIZE,
        )?;
        let deck_count = minimum_count("deck_count", params.deck_count, 1)?;
        let jokers_per_deck = bounded_count(
            "jokers_per_deck",
            params.jokers_per_deck,
            0..=MAX_JOKERS_PER_DECK,
        )?;
        let jokers_mode = parse_jokers_mode(&params.jokers_mode)?;
        let planned_duration_s = positive_count("planned_duration_s", params.planned_duration_s)?;
        let min_lap_s = positive_count("min_lap_s", params.min_lap_s)?;
        Ok(Self {
            name: params.name,
            event_date: params.event_date,
            venue: params.venue,
            lap_km: params.lap_km,
            organizer: params.organizer,
            scorer: params.scorer,
            planned_start: params.planned_start,
            planned_duration_s,
            min_lap_s,
            entry_mode: params.entry_mode,
            plate_model: params.plate_model,
            max_team_size,
            deck_count,
            jokers_per_deck,
            jokers_mode,
            max_cards: params.max_cards,
            tiebreak_order: params.tiebreak_order,
            logo_path: params.logo_path,
            hold_short_laps: params.hold_short_laps,
        })
    }
}

/// Return why *config* fails the minimum ride-setup rule.
///
/// The rule checks that `name`, `venue`, `organizer` and `scorer` each hold
/// text after a trim, and that `lap_km` is positive. The function does not
/// re-check `planned_duration_s` or `min_lap_s`, because [`RideConfig::new`]
/// already enforces their positivity.
#[must_use]
pub fn setup_minimum_violations(config: &RideConfig) -> Vec<String> {
    let mut violations = Vec::new();
    if config.name.trim().is_empty() {
        violations.push("name is required".to_owned());
    }
    if config.venue.trim().is_empty() {
        violations.push("venue is required".to_owned());
    }
    if config.organizer.trim().is_empty() {
        violations.push("organizer is required".to_owned());
    }
    if config.scorer.trim().is_empty() {
        violations.push("scorer is required".to_owned());
    }
    if config.lap_km <= 0.0 {
        violations.push("lap length must be positive".to_owned());
    }
    violations
}

/// Return the config error for a value outside its bound.
fn config_error(message: String) -> RideError {
    RideError::Config { message }
}

/// Return the config error for an unknown enum spelling.
fn unknown_value(kind: &str, value: &str) -> RideError {
    config_error(format!("unknown {kind}: {value}"))
}

/// Check a count against an inclusive range and return it as `u32`.
fn bounded_count(
    field: &str,
    value: i32,
    bounds: std::ops::RangeInclusive<i32>,
) -> Result<u32, RideError> {
    if !bounds.contains(&value) {
        let low = bounds.start();
        let high = bounds.end();
        return Err(config_error(format!(
            "{field} must be {low}..{high}, got {value}"
        )));
    }
    Ok(value.unsigned_abs())
}

/// Check a count against a minimum and return it as `u32`.
fn minimum_count(field: &str, value: i32, minimum: i32) -> Result<u32, RideError> {
    if value < minimum {
        return Err(config_error(format!(
            "{field} must be >= {minimum}, got {value}"
        )));
    }
    Ok(value.unsigned_abs())
}

/// Check a count for positivity and return it as `u32`.
fn positive_count(field: &str, value: i32) -> Result<u32, RideError> {
    if value <= 0 {
        return Err(config_error(format!(
            "{field} must be positive, got {value}"
        )));
    }
    Ok(value.unsigned_abs())
}

/// Read a stored jokers-mode spelling.
fn parse_jokers_mode(value: &str) -> Result<JokersMode, RideError> {
    match value {
        "per_deck" => Ok(JokersMode::PerDeck),
        "total" => Ok(JokersMode::Total),
        other => Err(config_error(format!(
            "jokers_mode must be one of ('per_deck', 'total'), got '{other}'"
        ))),
    }
}

#[cfg(test)]
mod tests;
