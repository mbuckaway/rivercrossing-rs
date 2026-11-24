// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the plate namespace: the shape rules, the in-use set, the pooled
//! derivation and the next-free pick.

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::{
    compare_numeric, empty_team_plate, increment_decimal, is_whole_number, next_free_plate,
    next_free_plate_skipping, plates_in_use, plates_in_use_owned, recompute_pooled_plate,
    require_plate_change_free, require_plate_free, require_plate_nonempty, require_plates_free,
    require_whole_plate, resolve_plate, shape_entry_plate,
};
use crate::enums::{EntryType, PlateModel};
use crate::error::RosterError;
use crate::types::{Entry, Rider};

fn rider(plate: Option<&str>) -> Rider {
    Rider {
        first_name: "Sam".to_owned(),
        plate: plate.map(str::to_owned),
        ..Rider::default()
    }
}

fn team(plate: &str, plates: &[Option<&str>]) -> Entry {
    let riders = plates.iter().map(|plate| rider(*plate)).collect();
    Entry::restored(
        format!("key-{plate}"),
        plate,
        "Team",
        EntryType::Team,
        riders,
    )
}

fn solo(plate: &str) -> Entry {
    Entry::restored(
        format!("key-{plate}"),
        plate,
        "Sam",
        EntryType::Solo,
        vec![rider(Some(plate))],
    )
}

// ------------------------------------------------------- require_whole_plate

#[test]
fn test_require_whole_plate_given_digits_accepts() {
    assert_eq!(require_whole_plate("77"), Ok(()));
}

#[rstest]
#[case::a_letter("77A", "77A")]
#[case::a_sign("-1", "-1")]
#[case::a_space("7 7", "7 7")]
fn test_require_whole_plate_given_a_non_digit_plate_refuses(
    #[case] plate: &str,
    #[case] reported: &str,
) {
    assert_eq!(
        require_whole_plate(plate),
        Err(RosterError::PlateShape {
            plate: reported.to_owned(),
            reason: "must be a whole number".to_owned(),
        })
    );
}

#[test]
fn test_require_whole_plate_given_an_empty_plate_refuses() {
    assert_eq!(
        require_whole_plate(""),
        Err(RosterError::PlateShape {
            plate: String::new(),
            reason: "must be a whole number".to_owned(),
        })
    );
}

// ---------------------------------------------------- require_plate_nonempty

#[test]
fn test_require_plate_nonempty_given_a_plate_accepts() {
    assert_eq!(require_plate_nonempty("7"), Ok(()));
}

#[test]
fn test_require_plate_nonempty_given_an_empty_plate_refuses() {
    assert_eq!(
        require_plate_nonempty(""),
        Err(RosterError::PlateShape {
            plate: String::new(),
            reason: "must not be empty".to_owned(),
        })
    );
}

#[test]
fn test_require_plate_nonempty_given_whitespace_refuses() {
    assert_eq!(
        require_plate_nonempty("   "),
        Err(RosterError::PlateShape {
            plate: "   ".to_owned(),
            reason: "must not be empty".to_owned(),
        })
    );
}

// ---------------------------------------------------------- is_whole_number

#[test]
fn test_is_whole_number_given_digits_is_true() {
    assert!(is_whole_number("007"));
}

#[test]
fn test_is_whole_number_given_an_empty_plate_is_false() {
    assert!(!is_whole_number(""));
}

#[test]
fn test_is_whole_number_given_a_letter_is_false() {
    assert!(!is_whole_number("77A"));
}

// ----------------------------------------------------------- compare_numeric

#[test]
fn test_compare_numeric_given_more_digits_is_greater() {
    assert!(compare_numeric("10", "9").is_gt());
}

#[test]
fn test_compare_numeric_given_zero_padding_is_equal() {
    assert!(compare_numeric("007", "7").is_eq());
}

#[test]
fn test_compare_numeric_given_a_smaller_plate_is_less() {
    assert!(compare_numeric("9", "45").is_lt());
}

// -------------------------------------------------------- increment_decimal

#[test]
fn test_increment_decimal_given_nine_carries_to_the_next_place() {
    assert_eq!(increment_decimal("9"), "10");
}

#[test]
fn test_increment_decimal_given_trailing_nines_carries_each_one() {
    assert_eq!(increment_decimal("1299"), "1300");
}

#[test]
fn test_increment_decimal_given_leading_zeros_drops_them() {
    assert_eq!(increment_decimal("0007"), "8");
}

#[test]
fn test_increment_decimal_given_only_zeros_is_one() {
    assert_eq!(increment_decimal("000"), "1");
}

// ------------------------------------------------------------ plates_in_use

#[test]
fn test_plates_in_use_given_no_entries_is_empty() {
    assert_eq!(
        plates_in_use(&[]).into_iter().collect::<Vec<_>>(),
        Vec::<&str>::new()
    );
}

#[test]
fn test_plates_in_use_collects_entry_and_rider_plates() {
    let entries = vec![solo("1"), team("2", &[Some("2"), Some("9")])];

    assert_eq!(
        plates_in_use(&entries).into_iter().collect::<Vec<_>>(),
        ["1", "2", "9"]
    );
}

#[test]
fn test_plates_in_use_skips_a_plate_less_rider() {
    let entries = vec![team("7", &[None, None])];

    assert_eq!(
        plates_in_use(&entries).into_iter().collect::<Vec<_>>(),
        ["7"]
    );
}

#[test]
fn test_plates_in_use_owned_returns_owned_plates() {
    let entries = vec![solo("4")];

    assert_eq!(
        plates_in_use_owned(&entries)
            .into_iter()
            .collect::<Vec<_>>(),
        ["4"]
    );
}

// ---------------------------------------------------------- next_free_plate

#[test]
fn test_next_free_plate_given_no_entries_is_one() {
    assert_eq!(next_free_plate(&[]), "1");
}

#[test]
fn test_next_free_plate_given_plates_is_one_past_the_highest() {
    let entries = vec![solo("77"), solo("78"), solo("123"), solo("212")];

    assert_eq!(next_free_plate(&entries), "213");
}

#[test]
fn test_next_free_plate_ignores_a_non_numeric_entry_plate() {
    let entries = vec![solo("paceCAR")];

    assert_eq!(next_free_plate(&entries), "1");
}

#[test]
fn test_next_free_plate_given_zero_padded_plates_returns_the_canonical_next() {
    let entries = vec![solo("0099")];

    assert_eq!(next_free_plate(&entries), "100");
}

#[test]
fn test_next_free_plate_given_only_zero_is_one() {
    let entries = vec![solo("0")];

    assert_eq!(next_free_plate(&entries), "1");
}

#[test]
fn test_next_free_plate_given_a_very_long_plate_does_not_overflow() {
    let entries = vec![solo(&"9".repeat(40))];

    assert_eq!(next_free_plate(&entries), format!("1{}", "0".repeat(40)));
}

#[test]
fn test_next_free_plate_counts_a_rider_plate() {
    let entries = vec![team("3", &[Some("3"), Some("50")])];

    assert_eq!(next_free_plate(&entries), "51");
}

// ------------------------------------------------- next_free_plate_skipping

use std::collections::BTreeSet;

#[test]
fn test_next_free_plate_skipping_given_no_reserved_equals_next_free_plate() {
    let entries = vec![solo("3"), team("7", &[Some("7"), Some("50")])];

    assert_eq!(
        next_free_plate_skipping(&entries, &BTreeSet::new()),
        next_free_plate(&entries)
    );
}

#[test]
fn test_next_free_plate_skipping_given_a_reserved_higher_plate_skips_it() {
    let entries = vec![solo("3")];
    let reserved = BTreeSet::from(["4".to_owned()]);

    assert_eq!(next_free_plate_skipping(&entries, &reserved), "5");
}

#[test]
fn test_next_free_plate_skipping_ignores_a_non_numeric_reserved_plate() {
    let reserved = BTreeSet::from(["A1".to_owned()]);

    assert_eq!(next_free_plate_skipping(&[], &reserved), "1");
}

// ----------------------------------------------------------- resolve_plate

#[test]
fn test_resolve_plate_given_an_entry_plate_returns_the_entry() {
    let entries = vec![solo("12")];

    let found = resolve_plate(&entries, PlateModel::RiderPooled, "12");

    assert_eq!(found.map(|entry| entry.key.as_str()), Some("key-12"));
}

#[test]
fn test_resolve_plate_given_a_pooled_rider_plate_returns_the_entry() {
    let entries = vec![team("9", &[Some("45"), Some("9")])];

    let found = resolve_plate(&entries, PlateModel::RiderPooled, "45");

    assert_eq!(found.map(|entry| entry.key.as_str()), Some("key-9"));
}

#[test]
fn test_resolve_plate_given_a_relay_rider_plate_returns_none() {
    let entries = vec![team("7", &[Some("5")])];

    assert_eq!(resolve_plate(&entries, PlateModel::TeamRelay, "5"), None);
}

#[test]
fn test_resolve_plate_given_an_unknown_plate_returns_none() {
    let entries = vec![solo("12")];

    assert_eq!(
        resolve_plate(&entries, PlateModel::RiderPooled, "999"),
        None
    );
}

#[test]
fn test_resolve_plate_given_a_relay_entry_plate_returns_the_entry() {
    let entries = vec![team("7", &[Some("5")])];

    let found = resolve_plate(&entries, PlateModel::TeamRelay, "7");

    assert_eq!(found.map(|entry| entry.key.as_str()), Some("key-7"));
}

// ------------------------------------------------------ shape_entry_plate

#[test]
fn test_shape_entry_plate_relay_given_a_plate_clears_rider_plates() {
    let mut riders = vec![rider(Some("5")), rider(Some("6"))];

    let shaped = shape_entry_plate(PlateModel::TeamRelay, &mut riders, Some("7"));

    assert_eq!(
        shaped.map(|shaped| (shaped.entry_plate, shaped.claims)),
        Ok(("7".to_owned(), vec!["7".to_owned()]))
    );
    assert_eq!(
        riders
            .iter()
            .map(|rider| rider.plate.clone())
            .collect::<Vec<_>>(),
        [None, None]
    );
}

#[test]
fn test_shape_entry_plate_relay_given_no_plate_refuses() {
    let mut riders = vec![rider(None), rider(None)];

    assert_eq!(
        shape_entry_plate(PlateModel::TeamRelay, &mut riders, None),
        Err(RosterError::InvalidMove {
            message: "team_relay entries require an explicit plate".to_owned(),
        })
    );
}

#[test]
fn test_shape_entry_plate_relay_given_a_blank_plate_refuses() {
    let mut riders = vec![rider(None)];

    assert_eq!(
        shape_entry_plate(PlateModel::TeamRelay, &mut riders, Some("   ")),
        Err(RosterError::PlateShape {
            plate: "   ".to_owned(),
            reason: "must not be empty".to_owned(),
        })
    );
}

#[test]
fn test_shape_entry_plate_relay_given_a_letter_plate_refuses() {
    let mut riders = vec![rider(None)];

    assert_eq!(
        shape_entry_plate(PlateModel::TeamRelay, &mut riders, Some("A1")),
        Err(RosterError::PlateShape {
            plate: "A1".to_owned(),
            reason: "must be a whole number".to_owned(),
        })
    );
}

#[test]
fn test_shape_entry_plate_pooled_adopts_the_lowest_rider_plate() {
    let mut riders = vec![rider(Some("45")), rider(Some("9"))];

    let shaped = shape_entry_plate(PlateModel::RiderPooled, &mut riders, None);

    assert_eq!(shaped.map(|shaped| shaped.entry_plate), Ok("9".to_owned()));
}

#[test]
fn test_shape_entry_plate_pooled_claims_every_rider_plate() {
    let mut riders = vec![rider(Some("45")), rider(Some("9"))];

    let shaped = shape_entry_plate(PlateModel::RiderPooled, &mut riders, None);

    assert_eq!(
        shaped.map(|shaped| shaped.claims),
        Ok(vec!["45".to_owned(), "9".to_owned()])
    );
}

#[test]
fn test_shape_entry_plate_pooled_given_no_riders_refuses() {
    let mut riders: Vec<Rider> = Vec::new();

    assert_eq!(
        shape_entry_plate(PlateModel::RiderPooled, &mut riders, None),
        Err(RosterError::InvalidMove {
            message: "rider_pooled entries need at least one rider".to_owned(),
        })
    );
}

#[test]
fn test_shape_entry_plate_pooled_given_a_plate_less_rider_refuses() {
    let mut riders = vec![rider(Some("1")), rider(None)];

    assert_eq!(
        shape_entry_plate(PlateModel::RiderPooled, &mut riders, None),
        Err(RosterError::InvalidMove {
            message: "rider_pooled riders must each carry a plate".to_owned(),
        })
    );
}

#[test]
fn test_shape_entry_plate_pooled_given_a_blank_rider_plate_refuses() {
    let mut riders = vec![rider(Some("  "))];

    assert_eq!(
        shape_entry_plate(PlateModel::RiderPooled, &mut riders, None),
        Err(RosterError::PlateShape {
            plate: "  ".to_owned(),
            reason: "must not be empty".to_owned(),
        })
    );
}

#[test]
fn test_shape_entry_plate_pooled_given_a_letter_rider_plate_refuses() {
    let mut riders = vec![rider(Some("77A"))];

    assert_eq!(
        shape_entry_plate(PlateModel::RiderPooled, &mut riders, None),
        Err(RosterError::PlateShape {
            plate: "77A".to_owned(),
            reason: "must be a whole number".to_owned(),
        })
    );
}

// ------------------------------------------------------ require_plates_free

#[test]
fn test_require_plates_free_given_a_claimed_plate_refuses() {
    let entries = vec![solo("12")];

    assert_eq!(
        require_plates_free(&entries, &["12"]),
        Err(RosterError::DuplicatePlate {
            plate: "12".to_owned(),
        })
    );
}

#[test]
fn test_require_plates_free_given_a_repeated_plate_refuses() {
    assert_eq!(
        require_plates_free(&[], &["5", "5"]),
        Err(RosterError::DuplicatePlate {
            plate: "5".to_owned(),
        })
    );
}

#[test]
fn test_require_plates_free_given_free_plates_accepts() {
    let entries = vec![solo("12")];

    assert_eq!(require_plates_free(&entries, &["5", "6"]), Ok(()));
}

#[test]
fn test_require_plates_free_given_a_claimed_rider_plate_refuses() {
    let entries = vec![team("3", &[Some("3"), Some("9")])];

    assert_eq!(
        require_plates_free(&entries, &["9"]),
        Err(RosterError::DuplicatePlate {
            plate: "9".to_owned(),
        })
    );
}

#[test]
fn test_require_plate_free_given_a_claimed_plate_refuses() {
    let in_use = plates_in_use_owned(&[solo("4")]);

    assert_eq!(
        require_plate_free(&in_use, "4"),
        Err(RosterError::DuplicatePlate {
            plate: "4".to_owned(),
        })
    );
}

#[test]
fn test_require_plate_free_given_a_free_plate_accepts() {
    let in_use = plates_in_use_owned(&[solo("4")]);

    assert_eq!(require_plate_free(&in_use, "5"), Ok(()));
}

#[test]
fn test_require_plate_change_free_given_the_current_plate_accepts() {
    let in_use = plates_in_use_owned(&[solo("4")]);

    assert_eq!(require_plate_change_free(&in_use, "4", "4"), Ok(()));
}

#[test]
fn test_require_plate_change_free_given_a_claimed_plate_refuses() {
    let in_use = plates_in_use_owned(&[solo("4"), solo("5")]);

    assert_eq!(
        require_plate_change_free(&in_use, "5", "4"),
        Err(RosterError::DuplicatePlate {
            plate: "5".to_owned(),
        })
    );
}

#[test]
fn test_require_plate_change_free_given_a_free_plate_accepts() {
    let in_use = plates_in_use_owned(&[solo("4")]);

    assert_eq!(require_plate_change_free(&in_use, "8", "4"), Ok(()));
}

// -------------------------------------------------------- empty_team_plate

#[test]
fn test_empty_team_plate_relay_given_a_plate_returns_it() {
    assert_eq!(
        empty_team_plate(&[], PlateModel::TeamRelay, Some("120")),
        Ok("120".to_owned())
    );
}

#[test]
fn test_empty_team_plate_relay_given_no_plate_takes_the_next_free() {
    let entries = vec![team("88", &[None, None])];

    assert_eq!(
        empty_team_plate(&entries, PlateModel::TeamRelay, None),
        Ok("89".to_owned())
    );
}

#[test]
fn test_empty_team_plate_relay_given_a_blank_plate_refuses() {
    assert_eq!(
        empty_team_plate(&[], PlateModel::TeamRelay, Some("  ")),
        Err(RosterError::PlateShape {
            plate: "  ".to_owned(),
            reason: "must not be empty".to_owned(),
        })
    );
}

#[test]
fn test_empty_team_plate_relay_given_a_letter_plate_refuses() {
    assert_eq!(
        empty_team_plate(&[], PlateModel::TeamRelay, Some("A1")),
        Err(RosterError::PlateShape {
            plate: "A1".to_owned(),
            reason: "must be a whole number".to_owned(),
        })
    );
}

#[test]
fn test_empty_team_plate_relay_given_a_duplicate_plate_refuses() {
    let entries = vec![team("88", &[None, None])];

    assert_eq!(
        empty_team_plate(&entries, PlateModel::TeamRelay, Some("88")),
        Err(RosterError::DuplicatePlate {
            plate: "88".to_owned(),
        })
    );
}

#[test]
fn test_empty_team_plate_pooled_given_no_plate_takes_the_next_free() {
    let entries = vec![solo("3")];

    assert_eq!(
        empty_team_plate(&entries, PlateModel::RiderPooled, None),
        Ok("4".to_owned())
    );
}

#[test]
fn test_empty_team_plate_pooled_given_an_explicit_plate_refuses() {
    assert_eq!(
        empty_team_plate(&[], PlateModel::RiderPooled, Some("99")),
        Err(RosterError::InvalidMove {
            message: "a rider_pooled team's plate is derived from its riders; an empty team claims the next free plate instead"
                .to_owned(),
        })
    );
}

// ---------------------------------------------------- recompute_pooled_plate

#[test]
fn test_recompute_pooled_plate_adopts_the_lowest_rider_plate() {
    let mut entry = team("45", &[Some("45"), Some("9")]);

    recompute_pooled_plate(&mut entry);

    assert_eq!(entry.plate, "9");
}

#[test]
fn test_recompute_pooled_plate_given_no_plate_bearing_rider_keeps_the_plate() {
    let mut entry = team("7", &[None, None]);

    recompute_pooled_plate(&mut entry);

    assert_eq!(entry.plate, "7");
}
