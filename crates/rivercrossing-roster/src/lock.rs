// SPDX-License-Identifier: GPL-3.0-only

//! The lock matrix: the guard that decides which roster change each ride state
//! and plate model allows.
//!
//! The ride engine owns which status transitions are legal; this module only
//! answers whether a given mutation is currently allowed. Every mutator asks
//! one function here instead of repeating the rule.

use crate::enums::{PlateModel, RideStatus};

/// Return `true` when structure edits may proceed (R-15).
///
/// Membership, plate and kind edits, and delete, are DRAFT-only, for either
/// plate model (spec S3).
#[must_use]
pub fn can_edit_structure(status: RideStatus) -> bool {
    status == RideStatus::Draft
}

/// Return `true` when a delete may remove the entry (R-15).
///
/// An entry that carries recorded data is never deletable, in any ride state;
/// otherwise deletion follows the same DRAFT-only rule as any other structural
/// edit.
#[must_use]
pub fn can_delete_entry(status: RideStatus, has_data: bool) -> bool {
    if has_data {
        return false;
    }
    can_edit_structure(status)
}

/// Return `true` when a rider may be relocated (R-17).
///
/// DRAFT allows every move, for either plate model. Once started, `team_relay`
/// keeps the start lock permanently -- the plate *is* the team's identity --
/// while `rider_pooled` stays open for audited moves while running and, as a
/// correction, once reopened; finished closes that door until reopened.
#[must_use]
pub fn can_move_rider(status: RideStatus, plate_model: PlateModel) -> bool {
    if status == RideStatus::Draft {
        return true;
    }
    if plate_model == PlateModel::TeamRelay {
        return false;
    }
    status == RideStatus::Running || status == RideStatus::Reopened
}

/// Return `true`: a new plate may be entered in any ride state.
///
/// The window inventory lists the ride as open for new plates at any time.
#[must_use]
pub fn can_add_entry() -> bool {
    true
}

/// Return `true`: a name-spelling fix is allowed in any ride state.
///
/// Spec S3 keeps name fixes open regardless of the ride status.
#[must_use]
pub fn can_fix_name() -> bool {
    true
}

#[cfg(test)]
mod tests;
