// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the rider, the entry and the audit event.

use pretty_assertions::assert_eq;

use super::{AuditEvent, Entry, EntryId, Rider};
use crate::enums::{EntryStatus, EntryType};

fn rider(first_name: &str, last_name: &str) -> Rider {
    Rider {
        first_name: first_name.to_owned(),
        last_name: last_name.to_owned(),
        ..Rider::default()
    }
}

fn fixture_entry() -> Entry {
    Entry::new(
        "77A",
        "Sam Ellis",
        EntryType::Solo,
        vec![rider("Sam", "Ellis")],
    )
}

// ------------------------------------------------------------ full_name

#[test]
fn test_rider_full_name_given_two_parts_joins_them_with_one_space() {
    let rider = Rider {
        first_name: "Sam".to_owned(),
        last_name: "Ellis".to_owned(),
        ..Rider::default()
    };

    assert_eq!(rider.full_name(), "Sam Ellis");
}

#[test]
fn test_rider_full_name_given_one_word_has_no_trailing_space() {
    let rider = rider("Alex", "");

    assert_eq!(rider.full_name(), "Alex");
}

#[test]
fn test_rider_full_name_given_padded_parts_strips_the_outer_blanks() {
    let rider = rider(" Sam ", " Ellis ");

    assert_eq!(rider.full_name(), "Sam Ellis");
}

#[test]
fn test_rider_full_name_given_internal_whitespace_keeps_the_run() {
    let rider = rider("Mary  Anne", "Knibbe");

    assert_eq!(rider.full_name(), "Mary  Anne Knibbe");
}

#[test]
fn test_rider_full_name_given_a_blank_last_name_renders_the_first_name_only() {
    let rider = rider("Alex", "   ");

    assert_eq!(rider.full_name(), "Alex");
}

#[test]
fn test_rider_full_name_given_a_blank_first_name_renders_the_last_name_only() {
    let rider = rider("  ", "Ellis");

    assert_eq!(rider.full_name(), "Ellis");
}

#[test]
fn test_rider_full_name_given_two_blank_parts_renders_empty() {
    let rider = rider("   ", " ");

    assert_eq!(rider.full_name(), "");
}

// ------------------------------------------------------------- defaults

#[test]
fn test_rider_default_leaves_plate_and_sex_unset() {
    let rider = Rider::default();

    assert_eq!((rider.plate, rider.sex), (None, None));
}

#[test]
fn test_rider_default_sets_sort_order_to_zero() {
    assert_eq!(Rider::default().sort_order, 0);
}

// --------------------------------------------------------------- entry

#[test]
fn test_entry_new_mints_a_thirty_two_character_lowercase_hex_key() {
    let key = fixture_entry().key;

    assert_eq!(key.len(), 32);
    assert!(
        key.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    );
}

#[test]
fn test_entry_new_mints_a_distinct_key_for_each_entry() {
    let first = fixture_entry().key;
    let second = fixture_entry().key;

    assert_ne!(first, second);
}

#[test]
fn test_entry_restored_keeps_the_supplied_key() {
    let entry = Entry::restored("a1b2c3", "77A", "Sam Ellis", EntryType::Solo, Vec::new());

    assert_eq!(entry.key, "a1b2c3");
}

#[test]
fn test_entry_id_wraps_the_entry_stable_key() {
    let entry = Entry::restored("a1b2c3", "77A", "Sam Ellis", EntryType::Solo, Vec::new());

    assert_eq!(entry.id(), EntryId("a1b2c3".to_owned()));
}

#[test]
fn test_entry_new_defaults_status_to_active() {
    assert_eq!(fixture_entry().status, EntryStatus::Active);
}

#[test]
fn test_entry_new_defaults_notes_to_empty() {
    assert_eq!(fixture_entry().notes, "");
}

#[test]
fn test_entry_new_defaults_has_data_to_false() {
    assert!(!fixture_entry().has_data);
}

#[test]
fn test_entry_new_defaults_logo_card_to_none() {
    assert_eq!(fixture_entry().logo_card, None);
}

#[test]
fn test_entry_new_keeps_the_supplied_plate_display_name_kind_and_riders() {
    let entry = fixture_entry();

    assert_eq!(
        (entry.plate, entry.display_name, entry.kind, entry.riders),
        (
            "77A".to_owned(),
            "Sam Ellis".to_owned(),
            EntryType::Solo,
            vec![rider("Sam", "Ellis")],
        )
    );
}

#[test]
fn test_entry_team_size_counts_the_riders() {
    let entry = Entry::new(
        "9",
        "Sam Ellis and Alex Roy",
        EntryType::Team,
        vec![rider("Sam", "Ellis"), rider("Alex", "Roy")],
    );

    assert_eq!(entry.team_size(), 2);
}

#[test]
fn test_entry_team_size_given_no_riders_is_zero() {
    let entry = Entry::new("9", "Empty Team", EntryType::Team, Vec::new());

    assert_eq!(entry.team_size(), 0);
}

#[test]
fn test_entry_equality_given_the_same_key_and_different_fields_is_equal() {
    let left = Entry::restored("a1b2c3", "77A", "Sam Ellis", EntryType::Solo, Vec::new());
    let mut right = Entry::restored(
        "a1b2c3",
        "9",
        "Alex Roy",
        EntryType::Team,
        vec![rider("Alex", "Roy")],
    );
    right.status = EntryStatus::Dnf;
    right.notes = "dnf".to_owned();
    right.has_data = true;
    right.logo_card = Some("AS".to_owned());

    assert_eq!(left, right);
}

#[test]
fn test_entry_equality_given_a_different_key_is_not_equal() {
    let left = Entry::restored("a1b2c3", "77A", "Sam Ellis", EntryType::Solo, Vec::new());
    let right = Entry::restored("d4e5f6", "77A", "Sam Ellis", EntryType::Solo, Vec::new());

    assert_ne!(left, right);
}

// ---------------------------------------------------------- audit event

#[test]
fn test_audit_event_equality_given_equal_action_and_payload_is_equal() {
    let left = AuditEvent {
        action: "create_solo_entry".to_owned(),
        payload: serde_json::json!({"plate": "77A", "count": 1}),
    };
    let right = AuditEvent {
        action: "create_solo_entry".to_owned(),
        payload: serde_json::json!({"plate": "77A", "count": 1}),
    };

    assert_eq!(left, right);
}

#[test]
fn test_audit_event_equality_given_different_actions_is_not_equal() {
    let left = AuditEvent {
        action: "create_solo_entry".to_owned(),
        payload: serde_json::Value::Null,
    };
    let right = AuditEvent {
        action: "create_team_entry".to_owned(),
        payload: serde_json::Value::Null,
    };

    assert_ne!(left, right);
}

#[test]
fn test_audit_event_equality_given_different_payloads_is_not_equal() {
    let left = AuditEvent {
        action: "move_rider".to_owned(),
        payload: serde_json::json!({"count": 1}),
    };
    let right = AuditEvent {
        action: "move_rider".to_owned(),
        payload: serde_json::json!({"count": 2}),
    };

    assert_ne!(left, right);
}

#[test]
fn test_audit_event_holds_an_integer_payload() {
    let event = AuditEvent {
        action: "create_team_entry".to_owned(),
        payload: serde_json::json!({"sort_order": 3}),
    };

    assert_eq!(event.payload["sort_order"], serde_json::json!(3));
}
