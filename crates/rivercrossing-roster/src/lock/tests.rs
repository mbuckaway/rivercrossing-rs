// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the lock matrix.

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::{can_add_entry, can_delete_entry, can_edit_structure, can_fix_name, can_move_rider};
use crate::enums::{PlateModel, RideStatus};

// ----------------------------------------------------- can_edit_structure

#[rstest]
#[case::draft(RideStatus::Draft, true)]
#[case::running(RideStatus::Running, false)]
#[case::finished(RideStatus::Finished, false)]
#[case::reopened(RideStatus::Reopened, false)]
fn test_can_edit_structure_given_a_status_matches_the_draft_only_rule(
    #[case] status: RideStatus,
    #[case] expected: bool,
) {
    assert_eq!(can_edit_structure(status), expected);
}

// ------------------------------------------------------ can_delete_entry

#[rstest]
#[case::draft_without_data(RideStatus::Draft, false, true)]
#[case::draft_with_data(RideStatus::Draft, true, false)]
#[case::running_without_data(RideStatus::Running, false, false)]
#[case::running_with_data(RideStatus::Running, true, false)]
#[case::finished_without_data(RideStatus::Finished, false, false)]
#[case::finished_with_data(RideStatus::Finished, true, false)]
#[case::reopened_without_data(RideStatus::Reopened, false, false)]
#[case::reopened_with_data(RideStatus::Reopened, true, false)]
fn test_can_delete_entry_given_a_status_and_has_data_matches_r15(
    #[case] status: RideStatus,
    #[case] has_data: bool,
    #[case] expected: bool,
) {
    assert_eq!(can_delete_entry(status, has_data), expected);
}

// -------------------------------------------------------- can_move_rider

#[rstest]
#[case::draft_pooled(RideStatus::Draft, PlateModel::RiderPooled, true)]
#[case::draft_relay(RideStatus::Draft, PlateModel::TeamRelay, true)]
#[case::running_pooled(RideStatus::Running, PlateModel::RiderPooled, true)]
#[case::running_relay(RideStatus::Running, PlateModel::TeamRelay, false)]
#[case::finished_pooled(RideStatus::Finished, PlateModel::RiderPooled, false)]
#[case::finished_relay(RideStatus::Finished, PlateModel::TeamRelay, false)]
#[case::reopened_pooled(RideStatus::Reopened, PlateModel::RiderPooled, true)]
#[case::reopened_relay(RideStatus::Reopened, PlateModel::TeamRelay, false)]
fn test_can_move_rider_given_a_status_and_a_plate_model_matches_r17(
    #[case] status: RideStatus,
    #[case] plate_model: PlateModel,
    #[case] expected: bool,
) {
    assert_eq!(can_move_rider(status, plate_model), expected);
}

// ---------------------------------------------------------- always open

#[test]
fn test_can_add_entry_is_always_true() {
    assert!(can_add_entry());
}

#[test]
fn test_can_fix_name_is_always_true() {
    assert!(can_fix_name());
}
