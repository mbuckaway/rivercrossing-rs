// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the team-logo card pick.

use pretty_assertions::assert_eq;
use rivercrossing_cards::Card;

use super::{available_cards, choose_card, claimed_cards, next_logo_card};
use crate::enums::{EntryMode, EntryType, PlateModel};
use crate::roster::Roster;
use crate::types::{AuditEvent, Entry, EntryId};

fn deck(codes: &[&str]) -> Vec<String> {
    codes.iter().map(|code| (*code).to_owned()).collect()
}

fn team(plate: &str, logo_card: Option<&str>) -> Entry {
    let mut entry = Entry::restored(
        format!("key-{plate}"),
        plate,
        "Team",
        EntryType::Team,
        Vec::new(),
    );
    entry.logo_card = logo_card.map(str::to_owned);
    entry
}

fn seeded() -> Roster {
    Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(8843))
        .expect("the settings are valid")
}

fn unseeded() -> Roster {
    Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, None).expect("the settings are valid")
}

fn seeded_codes() -> Vec<String> {
    rivercrossing_cards::seeded_card_codes(8843, 1)
        .iter()
        .map(Card::code)
        .collect()
}

fn carded_team(code: &str) -> Roster {
    let mut roster = seeded();
    let mut entry = Entry::restored("key-a", "77", "Team A", EntryType::Team, Vec::new());
    entry.logo_card = Some(code.to_owned());
    roster.load_entries(vec![entry]);
    roster
}

fn all_claimed() -> Roster {
    let entries = seeded_codes()
        .iter()
        .enumerate()
        .map(|(index, code)| {
            let mut entry = Entry::restored(
                format!("key-{index}"),
                (index + 1).to_string(),
                "Team",
                EntryType::Team,
                Vec::new(),
            );
            entry.logo_card = Some(code.clone());
            entry
        })
        .collect();
    let mut roster = seeded();
    roster.load_entries(entries);
    roster
}

// ---------------------------------------------------------- claimed_cards

#[test]
fn test_claimed_cards_given_no_team_collects_nothing() {
    let entries = vec![team("1", None)];

    assert_eq!(
        claimed_cards(&entries).iter().copied().collect::<Vec<_>>(),
        Vec::<&str>::new()
    );
}

#[test]
fn test_claimed_cards_collects_every_assigned_logo() {
    let entries = vec![
        team("1", Some("AS")),
        team("2", None),
        team("3", Some("KH")),
    ];

    assert_eq!(
        claimed_cards(&entries).iter().copied().collect::<Vec<_>>(),
        ["AS", "KH"]
    );
}

// --------------------------------------------------------- next_logo_card

#[test]
fn test_next_logo_card_given_an_empty_deck_returns_none() {
    let codes = deck(&[]);

    assert_eq!(next_logo_card(&codes, &claimed_cards(&[]), None), None);
}

#[test]
fn test_next_logo_card_given_no_after_returns_the_first_free_code() {
    let codes = deck(&["AS", "KH", "QD"]);

    assert_eq!(
        next_logo_card(&codes, &claimed_cards(&[]), None),
        Some("AS")
    );
}

#[test]
fn test_next_logo_card_given_an_after_code_returns_the_next_code() {
    let codes = deck(&["AS", "KH", "QD"]);

    assert_eq!(
        next_logo_card(&codes, &claimed_cards(&[]), Some("AS")),
        Some("KH")
    );
}

#[test]
fn test_next_logo_card_given_the_last_code_wraps_to_the_first() {
    let codes = deck(&["AS", "KH", "QD"]);

    assert_eq!(
        next_logo_card(&codes, &claimed_cards(&[]), Some("QD")),
        Some("AS")
    );
}

#[test]
fn test_next_logo_card_given_an_unknown_after_code_starts_at_the_first() {
    let codes = deck(&["AS", "KH"]);

    assert_eq!(
        next_logo_card(&codes, &claimed_cards(&[]), Some("ZZ")),
        Some("AS")
    );
}

#[test]
fn test_next_logo_card_given_a_claimed_code_skips_it() {
    let codes = deck(&["AS", "KH", "QD"]);
    let entries = vec![team("1", Some("AS"))];

    assert_eq!(
        next_logo_card(&codes, &claimed_cards(&entries), None),
        Some("KH")
    );
}

#[test]
fn test_next_logo_card_given_the_next_code_claimed_wraps_past_it() {
    let codes = deck(&["AS", "KH"]);
    let entries = vec![team("1", Some("KH"))];

    assert_eq!(
        next_logo_card(&codes, &claimed_cards(&entries), Some("AS")),
        Some("AS")
    );
}

#[test]
fn test_next_logo_card_given_every_code_claimed_returns_none() {
    let codes = deck(&["AS", "KH"]);
    let entries = vec![team("1", Some("AS")), team("2", Some("KH"))];

    assert_eq!(next_logo_card(&codes, &claimed_cards(&entries), None), None);
}

// --------------------------------------------------------- available_cards

#[test]
fn test_available_cards_given_no_claims_and_no_exclude_keeps_the_deck() {
    let codes = deck(&["AS", "KH", "QD"]);

    assert_eq!(
        available_cards(&codes, &claimed_cards(&[]), None),
        ["AS", "KH", "QD"]
    );
}

#[test]
fn test_available_cards_drops_a_claimed_code() {
    let codes = deck(&["AS", "KH", "QD"]);
    let entries = vec![team("1", Some("KH"))];

    assert_eq!(
        available_cards(&codes, &claimed_cards(&entries), None),
        ["AS", "QD"]
    );
}

#[test]
fn test_available_cards_drops_the_excluded_code() {
    let codes = deck(&["AS", "KH", "QD"]);

    assert_eq!(
        available_cards(&codes, &claimed_cards(&[]), Some("AS")),
        ["KH", "QD"]
    );
}

#[test]
fn test_available_cards_given_every_code_claimed_returns_none_free() {
    let codes = deck(&["AS", "KH"]);
    let entries = vec![team("1", Some("AS")), team("2", Some("KH"))];

    assert_eq!(
        available_cards(&codes, &claimed_cards(&entries), None),
        Vec::<&str>::new()
    );
}

// ------------------------------------------------------------- choose_card

#[test]
fn test_choose_card_given_no_available_code_returns_none() {
    assert_eq!(choose_card(&[], |_| 0), None);
}

#[test]
fn test_choose_card_given_the_first_index_returns_the_first_code() {
    // One chooser instance drives both the empty and the non-empty pool, so the
    // pick's two outcomes live in one monomorphisation for branch coverage.
    let mut pick = |_| 0;
    assert_eq!(choose_card(&[], &mut pick), None);
    assert_eq!(choose_card(&["AS", "KH"], &mut pick), Some("AS"));
}

#[test]
fn test_choose_card_given_the_last_index_returns_the_last_code() {
    assert_eq!(choose_card(&["AS", "KH"], |_| 1), Some("KH"));
}

#[test]
fn test_choose_card_given_an_index_past_the_end_wraps() {
    assert_eq!(choose_card(&["AS", "KH"], |len| len + 1), Some("KH"));
}

// -------------------------------------------------------- random_team_card

#[test]
fn test_random_team_card_given_a_fixed_chooser_returns_the_decks_first_code() {
    let roster = seeded();
    let cards = seeded_codes();

    assert_eq!(roster.random_team_card(None, |_| 0), Some(cards[0].clone()));
}

#[test]
fn test_random_team_card_given_an_exclude_never_returns_it() {
    let roster = seeded();
    let cards = seeded_codes();

    assert_eq!(
        roster.random_team_card(Some(&cards[0]), |_| 0),
        Some(cards[1].clone())
    );
}

#[test]
fn test_random_team_card_skips_a_claimed_card() {
    let cards = seeded_codes();
    let roster = carded_team(&cards[0]);

    assert_eq!(roster.random_team_card(None, |_| 0), Some(cards[1].clone()));
}

#[test]
fn test_random_team_card_given_no_seed_returns_none() {
    assert_eq!(unseeded().random_team_card(None, |_| 0), None);
}

#[test]
fn test_random_team_card_given_every_card_claimed_returns_none() {
    assert_eq!(all_claimed().random_team_card(None, |_| 0), None);
}

// ----------------------------------------------------- set_team_logo_card

#[test]
fn test_set_team_logo_card_replaces_the_card_and_audits() {
    let mut roster = seeded();
    let id = roster
        .create_team_entry(crate::roster::TeamFields {
            display_name: "Team A",
            riders: vec![
                crate::types::Rider {
                    first_name: "Alex".to_owned(),
                    plate: Some("77".to_owned()),
                    ..crate::types::Rider::default()
                },
                crate::types::Rider {
                    first_name: "Bo".to_owned(),
                    plate: Some("78".to_owned()),
                    ..crate::types::Rider::default()
                },
            ],
            ..crate::roster::TeamFields::default()
        })
        .expect("a valid team is accepted");
    roster.take_audit_log();

    roster
        .set_team_logo_card(&id, "AS")
        .expect("the entry is a member");

    assert_eq!(
        (
            roster
                .entry_by_key(&id.0)
                .and_then(|entry| entry.logo_card.clone()),
            roster.audit_log()
        ),
        (
            Some("AS".to_owned()),
            &[AuditEvent {
                action: "set_team_logo_card".to_owned(),
                payload: serde_json::json!({"plate": "77", "code": "AS"}),
            }][..]
        )
    );
}

#[test]
fn test_set_team_logo_card_accepts_a_retired_entry() {
    let mut roster = seeded();
    roster.load_retired_entries(vec![Entry::restored(
        "retired1",
        "7",
        "Old Team",
        EntryType::Team,
        Vec::new(),
    )]);

    roster
        .set_team_logo_card(&EntryId("retired1".to_owned()), "KH")
        .expect("a retired entry is a member");

    assert_eq!(
        roster
            .entry_by_key("retired1")
            .and_then(|entry| entry.logo_card.clone()),
        Some("KH".to_owned())
    );
}

#[test]
fn test_set_team_logo_card_given_an_unknown_entry_refuses() {
    let mut roster = seeded();

    let error = roster
        .set_team_logo_card(&EntryId("ghost".to_owned()), "AS")
        .expect_err("a foreign entry is refused");

    assert_eq!(
        error,
        crate::error::RosterError::EntryNotFound {
            key: "ghost".to_owned(),
        }
    );
}
