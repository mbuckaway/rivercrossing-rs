// SPDX-License-Identifier: GPL-3.0-only

//! The plate namespace: the plate allocation and the model-correct plate-change
//! dispatch that the editors and the fixes share.
//!
//! Every plate in a ride shares one namespace: an entry's own plate and, under
//! `rider_pooled`, each rider's own plate. So a duplicate check spans both, and
//! the next free plate is one past the highest numeric plate of either.
//!
//! A plate is a whole-number string. The Python oracle accepts any Unicode
//! decimal digit through `str.isdigit`; the port accepts the ASCII digits only,
//! because the namespace feeds a CSV round trip and a numeric comparison.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use crate::enums::PlateModel;
use crate::error::RosterError;
use crate::types::{Entry, Rider};

/// The plate that a create path will store, and every plate it claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryPlates {
    /// The plate that the entry carries.
    pub(crate) entry_plate: String,
    /// Every plate that the entry claims in the ride's namespace.
    pub(crate) claims: Vec<String>,
}

/// Return an error unless *plate* is one or more ASCII digits (spec S7).
pub(crate) fn require_whole_plate(plate: &str) -> Result<(), RosterError> {
    if is_whole_number(plate) {
        return Ok(());
    }
    Err(RosterError::PlateShape {
        plate: plate.to_owned(),
        reason: "must be a whole number".to_owned(),
    })
}

/// Return an error unless *plate* has a non-blank character.
///
/// Every caller checks this before [`require_whole_plate`], so a blank plate
/// reads as blank rather than as a malformed number.
pub(crate) fn require_plate_nonempty(plate: &str) -> Result<(), RosterError> {
    if !plate.trim().is_empty() {
        return Ok(());
    }
    Err(RosterError::PlateShape {
        plate: plate.to_owned(),
        reason: "must not be empty".to_owned(),
    })
}

/// Return `true` when *plate* is one or more ASCII digits.
pub(crate) fn is_whole_number(plate: &str) -> bool {
    !plate.is_empty() && plate.chars().all(|character| character.is_ascii_digit())
}

/// Compare two whole-number plates by their numeric value.
///
/// The comparison trims leading zeros and then compares the digit count before
/// the digits, so `"10"` sorts above `"9"` and `"007"` equals `"7"`.
pub(crate) fn compare_numeric(left: &str, right: &str) -> Ordering {
    let left = left.trim_start_matches('0');
    let right = right.trim_start_matches('0');
    (left.len(), left).cmp(&(right.len(), right))
}

/// Return the canonical decimal successor of a whole-number string.
///
/// Leading zeros are dropped, so `"0099"` becomes `"100"`. The increment never
/// parses the value into an integer, so a plate of any length is safe.
pub(crate) fn increment_decimal(digits: &str) -> String {
    let trimmed = digits.trim_start_matches('0');
    let mut incremented: Vec<char> = if trimmed.is_empty() {
        vec!['0']
    } else {
        trimmed.chars().collect()
    };
    let mut carry = true;
    for digit in incremented.iter_mut().rev() {
        if !carry {
            break;
        }
        match digit.to_digit(10) {
            Some(value) if value < 9 => {
                *digit = char::from_digit(value + 1, 10).unwrap_or(*digit);
                carry = false;
            }
            _ => *digit = '0',
        }
    }
    if carry {
        incremented.insert(0, '1');
    }
    incremented.into_iter().collect()
}

/// Return every plate that the live entries claim.
pub(crate) fn plates_in_use(entries: &[Entry]) -> BTreeSet<&str> {
    let mut in_use = BTreeSet::new();
    for entry in entries {
        in_use.insert(entry.plate.as_str());
        for rider in &entry.riders {
            if let Some(plate) = rider.plate.as_deref() {
                in_use.insert(plate);
            }
        }
    }
    in_use
}

/// Return every plate that the live entries claim, as owned strings.
///
/// A mutator takes this snapshot before it borrows an entry mutably.
pub(crate) fn plates_in_use_owned(entries: &[Entry]) -> BTreeSet<String> {
    plates_in_use(entries)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

/// Return one past the highest numeric plate in use.
///
/// A non-numeric plate is ignored; a roster with no numeric plate returns
/// `"1"`.
pub(crate) fn next_free_plate(entries: &[Entry]) -> String {
    next_free_plate_skipping(entries, &BTreeSet::new())
}

/// Return one past the highest numeric plate in use or in *reserved*.
///
/// The namespace is the entries' own plates and their riders' plates, plus
/// every reserved plate. A reserved plate counts even when no entry carries
/// it, so an import can keep a plate for a staged entry. A non-numeric plate
/// is ignored; an empty namespace returns `"1"`.
pub(crate) fn next_free_plate_skipping(entries: &[Entry], reserved: &BTreeSet<String>) -> String {
    let in_use = plates_in_use(entries);
    in_use
        .iter()
        .copied()
        .chain(reserved.iter().map(String::as_str))
        .filter(|plate| is_whole_number(plate))
        .max_by(|left, right| compare_numeric(left, right))
        .map_or_else(|| "1".to_owned(), increment_decimal)
}

/// Return an error when any of *plates* is claimed or repeated.
pub(crate) fn require_plates_free(entries: &[Entry], plates: &[&str]) -> Result<(), RosterError> {
    let in_use = plates_in_use(entries);
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for plate in plates {
        if in_use.contains(plate) || !seen.insert(plate) {
            return Err(RosterError::DuplicatePlate {
                plate: (*plate).to_owned(),
            });
        }
    }
    Ok(())
}

/// Return an error when *plate* is already claimed.
pub(crate) fn require_plate_free(
    in_use: &BTreeSet<String>,
    plate: &str,
) -> Result<(), RosterError> {
    if in_use.contains(plate) {
        return Err(RosterError::DuplicatePlate {
            plate: plate.to_owned(),
        });
    }
    Ok(())
}

/// Return an error unless *plate* is free or equal to the plate *current*.
///
/// Setting a plate back to itself is a harmless no-op, not a collision with the
/// plate's own prior claim.
pub(crate) fn require_plate_change_free(
    in_use: &BTreeSet<String>,
    plate: &str,
    current: &str,
) -> Result<(), RosterError> {
    if plate == current {
        return Ok(());
    }
    require_plate_free(in_use, plate)
}

/// Return the entry plate that a create path stores, shaped by the model.
///
/// A `team_relay` entry carries the caller's explicit whole-number plate and
/// its riders carry none. A `rider_pooled` entry requires every rider's own
/// plate and adopts the numerically lowest of them.
pub(crate) fn shape_entry_plate(
    model: PlateModel,
    riders: &mut [Rider],
    plate: Option<&str>,
) -> Result<EntryPlates, RosterError> {
    match model {
        PlateModel::TeamRelay => {
            let Some(plate) = plate else {
                return Err(RosterError::InvalidMove {
                    message: "team_relay entries require an explicit plate".to_owned(),
                });
            };
            require_plate_nonempty(plate)?;
            require_whole_plate(plate)?;
            for rider in riders.iter_mut() {
                rider.plate = None;
            }
            Ok(EntryPlates {
                entry_plate: plate.to_owned(),
                claims: vec![plate.to_owned()],
            })
        }
        PlateModel::RiderPooled => {
            if riders.is_empty() {
                return Err(RosterError::InvalidMove {
                    message: "rider_pooled entries need at least one rider".to_owned(),
                });
            }
            if riders.iter().any(|rider| rider.plate.is_none()) {
                return Err(RosterError::InvalidMove {
                    message: "rider_pooled riders must each carry a plate".to_owned(),
                });
            }
            let claims: Vec<String> = riders
                .iter()
                .filter_map(|rider| rider.plate.clone())
                .collect();
            for claim in &claims {
                require_plate_nonempty(claim)?;
                require_whole_plate(claim)?;
            }
            let entry_plate = claims
                .iter()
                .min_by(|left, right| compare_numeric(left, right))
                .cloned()
                .unwrap_or_default();
            Ok(EntryPlates {
                entry_plate,
                claims,
            })
        }
    }
}

/// Return the plate that a zero-rider team carries (W8).
///
/// A `team_relay` empty team carries its explicit whole-number plate, or the
/// next free plate when none is given. A `rider_pooled` empty team has no rider
/// to derive a plate from, so it claims the next free plate provisionally; a
/// caller-supplied plate is refused.
pub(crate) fn empty_team_plate(
    entries: &[Entry],
    model: PlateModel,
    plate: Option<&str>,
) -> Result<String, RosterError> {
    match model {
        PlateModel::TeamRelay => {
            let Some(plate) = plate else {
                return Ok(next_free_plate(entries));
            };
            require_plate_nonempty(plate)?;
            require_whole_plate(plate)?;
            require_plates_free(entries, &[plate])?;
            Ok(plate.to_owned())
        }
        PlateModel::RiderPooled => {
            if plate.is_some() {
                return Err(RosterError::InvalidMove {
                    message: "a rider_pooled team's plate is derived from its riders; an empty team claims the next free plate instead"
                        .to_owned(),
                });
            }
            Ok(next_free_plate(entries))
        }
    }
}

/// Return the entry that *plate* names, under the ride's plate model.
///
/// An entry's own plate always matches; under `rider_pooled` a rider's own
/// plate resolves to the entry that carries the rider.
pub(crate) fn resolve_plate<'a>(
    entries: &'a [Entry],
    model: PlateModel,
    plate: &str,
) -> Option<&'a Entry> {
    entries.iter().find(|entry| {
        entry.plate == plate
            || (model == PlateModel::RiderPooled && holds_rider_plate(entry, plate))
    })
}

/// Return `true` when a rider on *entry* carries *plate*.
fn holds_rider_plate(entry: &Entry, plate: &str) -> bool {
    entry
        .riders
        .iter()
        .any(|rider| rider.plate.as_deref() == Some(plate))
}

/// Re-derive a pooled team's plate from its riders' own plates.
///
/// An entry with no plate-bearing rider keeps the plate it carries.
pub(crate) fn recompute_pooled_plate(entry: &mut Entry) {
    let rider_plates: Vec<&str> = entry
        .riders
        .iter()
        .filter_map(|rider| rider.plate.as_deref())
        .collect();
    let lowest = rider_plates
        .iter()
        .copied()
        .min_by(|left, right| compare_numeric(left, right))
        .map_or_else(|| entry.plate.clone(), str::to_owned);
    entry.plate = lowest;
}

#[cfg(test)]
mod tests;
