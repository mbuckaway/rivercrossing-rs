// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the standings row types and the snapshot conversion.

#![expect(
    clippy::float_cmp,
    reason = "the fixtures hold exact whole and half seconds, so every copy is exact"
)]

use pretty_assertions::assert_eq;
use rivercrossing_cards::Card;
use rivercrossing_hands::best_hand;
use rivercrossing_ride::{EntryId, SnapshotEntry, SnapshotKind};

use super::{EntryResult, Placed};

fn fixture_cards() -> Vec<Card> {
    vec![
        Card::parse("10C").expect("10C is a valid card code"),
        Card::parse("JC").expect("JC is a valid card code"),
        Card::parse("QC").expect("QC is a valid card code"),
        Card::parse("KC").expect("KC is a valid card code"),
        Card::parse("AC").expect("AC is a valid card code"),
    ]
}

fn fixture_snapshot() -> SnapshotEntry {
    let cards = fixture_cards();
    let hand = best_hand(&cards);
    SnapshotEntry {
        entry_id: EntryId("entry-1".to_owned()),
        plate: "101".to_owned(),
        name: "Ada Lovelace".to_owned(),
        kind: SnapshotKind::Solo,
        laps: 3,
        total_time: 4210.5,
        best_lap: 1200.25,
        cards,
        hand,
        dnf: false,
        sex: Some("F".to_owned()),
        tiebreak_card: Some(Card::parse("KD").expect("KD is a valid card code")),
    }
}

fn fixture_team_snapshot() -> SnapshotEntry {
    let cards = fixture_cards();
    let hand = best_hand(&cards);
    SnapshotEntry {
        entry_id: EntryId("entry-2".to_owned()),
        plate: "201".to_owned(),
        name: "The Drafters".to_owned(),
        kind: SnapshotKind::Team,
        laps: 5,
        total_time: 7210.0,
        best_lap: 980.5,
        cards,
        hand,
        dnf: true,
        sex: None,
        tiebreak_card: None,
    }
}

fn fixture_result() -> EntryResult {
    EntryResult::from(&fixture_snapshot())
}

#[test]
fn test_entry_result_from_snapshot_preserves_every_field() {
    let snapshot = fixture_snapshot();
    let result = EntryResult::from(&snapshot);
    assert_eq!(result.entry_id, snapshot.entry_id);
    assert_eq!(result.plate, snapshot.plate);
    assert_eq!(result.name, snapshot.name);
    assert_eq!(result.kind, snapshot.kind);
    assert_eq!(result.laps, snapshot.laps);
    assert_eq!(result.total_time, snapshot.total_time);
    assert_eq!(result.best_lap, snapshot.best_lap);
    assert_eq!(result.cards, snapshot.cards);
    assert_eq!(result.hand, snapshot.hand);
    assert_eq!(result.dnf, snapshot.dnf);
    assert_eq!(result.sex, snapshot.sex);
    assert_eq!(result.tiebreak_card, snapshot.tiebreak_card);
}

#[test]
fn test_entry_result_from_snapshot_keeps_the_solo_sex() {
    let result = EntryResult::from(&fixture_snapshot());
    assert_eq!(result.sex, Some("F".to_owned()));
}

#[test]
fn test_entry_result_from_snapshot_keeps_the_drawn_tiebreak_card() {
    let result = EntryResult::from(&fixture_snapshot());
    assert_eq!(
        result.tiebreak_card,
        Some(Card::parse("KD").expect("KD is a valid card code"))
    );
}

#[test]
fn test_entry_result_from_snapshot_keeps_a_team_without_sex_or_draw() {
    let result = EntryResult::from(&fixture_team_snapshot());
    assert_eq!(result.kind, SnapshotKind::Team);
    assert_eq!(result.sex, None);
    assert_eq!(result.tiebreak_card, None);
    assert!(result.dnf);
}

#[test]
fn test_placed_is_dns_without_a_place() {
    let placed = Placed {
        place: None,
        result: fixture_result(),
        tie_note: None,
        draw_required: false,
    };
    assert_eq!(placed.place, None);
    assert!(placed.is_dns());
}

#[test]
fn test_placed_is_not_dns_with_a_place() {
    let placed = Placed {
        place: Some(1),
        result: fixture_result(),
        tie_note: None,
        draw_required: false,
    };
    assert_eq!(placed.place, Some(1));
    assert!(!placed.is_dns());
}

#[test]
fn test_placed_keeps_its_tie_markings() {
    let placed = Placed {
        place: Some(2),
        result: fixture_result(),
        tie_note: Some("draw required".to_owned()),
        draw_required: true,
    };
    assert_eq!(placed.tie_note.as_deref(), Some("draw required"));
    assert!(placed.draw_required);
}
