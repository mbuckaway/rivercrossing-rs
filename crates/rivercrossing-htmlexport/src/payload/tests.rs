// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the shared payload builder.

use std::collections::BTreeMap;

use pretty_assertions::assert_eq;
use rivercrossing_cards::Card;
use rivercrossing_hands::best_hand;
use rivercrossing_ride::EntryId;
use rivercrossing_standings::{EntryResult, Placed, SnapshotKind};
use rstest::rstest;
use time::{Date, Month};

use super::build_payload;
use crate::error::ExportError;
use crate::types::{CardPair, ExportInput, ExportOptions, KICKER, RideInfo, SELF_TEST_NOTE};

const GENERATED: &str = "Generated 16:07, Sept 20 2026";

fn card(code: &str) -> Card {
    Card::parse(code).expect("the fixture card code is valid")
}

fn pair(rank: &str, suit: &str) -> CardPair {
    CardPair {
        rank: rank.to_owned(),
        suit: suit.to_owned(),
    }
}

/// Return the fixture's five-card draw: three nines, a king and a two.
fn fixture_cards() -> Vec<Card> {
    vec![card("9S"), card("9D"), card("9C"), card("KH"), card("2S")]
}

/// Build one finished-ride entry whose total time follows its lap count.
fn sample(plate: &str, name: &str, kind: SnapshotKind, laps: u32) -> EntryResult {
    let cards = fixture_cards();
    EntryResult {
        entry_id: EntryId(plate.to_owned()),
        plate: plate.to_owned(),
        name: name.to_owned(),
        kind,
        laps,
        total_time: f64::from(laps) * 1800.0 + 120.0,
        best_lap: 1800.0,
        hand: best_hand(&cards),
        cards,
        dnf: false,
        sex: (kind == SnapshotKind::Solo).then(|| "F".to_owned()),
        tiebreak_card: None,
    }
}

fn placed(place: Option<u32>, result: EntryResult) -> Placed {
    Placed {
        place,
        result,
        tie_note: None,
        draw_required: false,
    }
}

/// Build the oracle's two-entry sample field: 11 laps, then 10.
fn pair_field() -> Vec<Placed> {
    vec![
        placed(
            Some(1),
            sample("88", "Moss Ridge Riders", SnapshotKind::Solo, 11),
        ),
        placed(Some(2), sample("7", "Luca Ferrari", SnapshotKind::Solo, 10)),
    ]
}

/// Build a per-kind field: teams take places 1..N, then solo restarts at 1.
///
/// Lap counts fall down each kind's list, so the first entry of each kind
/// leads its board.
fn field(teams: u32, solo: u32) -> Vec<Placed> {
    let mut rows = Vec::new();
    for index in 0..teams {
        let number = index + 1;
        rows.push(placed(
            Some(number),
            sample(
                &(500 + index).to_string(),
                &format!("Team {number}"),
                SnapshotKind::Team,
                30 - index,
            ),
        ));
    }
    for index in 0..solo {
        let number = index + 1;
        rows.push(placed(
            Some(number),
            sample(
                &(10 + index).to_string(),
                &format!("Solo {number}"),
                SnapshotKind::Solo,
                20 - index,
            ),
        ));
    }
    rows
}

fn ride() -> RideInfo {
    RideInfo {
        name: "Test Poker Run 2026".to_owned(),
        event_date: Date::from_calendar_date(2026, Month::June, 6)
            .expect("the fixture date is valid"),
        venue: "Test Venue".to_owned(),
        lap_km: 8.0,
        organizer: "Test Org".to_owned(),
        scorer: "T. Ester".to_owned(),
    }
}

fn input_with(placed: Vec<Placed>, options: ExportOptions) -> ExportInput {
    ExportInput {
        ride: ride(),
        placed,
        options,
        generated: GENERATED.to_owned(),
        logo_src: None,
        team_logos: BTreeMap::new(),
        riders: 0,
        self_test_unverified: false,
        tie_note: None,
    }
}

fn input(placed: Vec<Placed>) -> ExportInput {
    input_with(placed, ExportOptions::default())
}

fn options(show_times: bool, laps_board: bool, time_board: bool) -> ExportOptions {
    ExportOptions {
        show_times,
        laps_board,
        time_board,
        ..ExportOptions::default()
    }
}

// ------------------------------------------------------------- the event

#[test]
fn test_build_payload_maps_the_ride_to_the_event_block() {
    // Arrange
    let input = input(pair_field());

    // Act
    let event = build_payload(&input)
        .expect("the fixture plates are numeric")
        .event;

    // Assert
    assert_eq!(
        (
            event.kicker.as_str(),
            event.title.as_str(),
            event.meta.as_str(),
            event.organizer.as_str(),
            event.scorer.as_str(),
        ),
        (
            KICKER,
            "Test Poker Run 2026",
            "Saturday June 6, 2026 · Test Venue · 8 km loop",
            "Organizer: Test Org",
            "Scorer: T. Ester",
        )
    );
}

#[test]
fn test_build_payload_carries_the_caller_stamp() {
    // Arrange
    let input = input(pair_field());

    // Act
    let generated = build_payload(&input)
        .expect("the fixture plates are numeric")
        .event
        .generated;

    // Assert
    assert_eq!(generated, GENERATED);
}

#[test]
fn test_build_payload_counts_entries_laps_cards_and_riders() {
    // Arrange
    let mut input = input(pair_field());
    input.riders = 7;

    // Act
    let event = build_payload(&input)
        .expect("the fixture plates are numeric")
        .event;

    // Assert
    assert_eq!(
        (event.entries, event.laps, event.cards, event.riders),
        (2, 21, 10, 7)
    );
}

#[test]
fn test_build_payload_carries_the_options_through() {
    // Arrange
    let chosen = options(true, false, true);
    let input = input_with(pair_field(), chosen);

    // Act
    let payload = build_payload(&input).expect("the fixture plates are numeric");

    // Assert
    assert_eq!(payload.options, chosen);
}

#[test]
fn test_build_payload_carries_the_callers_tie_note() {
    // Arrange
    let mut input = input(pair_field());
    input.tie_note = Some("draw required".to_owned());

    // Act
    let payload = build_payload(&input).expect("the fixture plates are numeric");

    // Assert
    assert_eq!(payload.tie_note, Some("draw required".to_owned()));
}

// ------------------------------------------------------------ the rows

#[test]
fn test_build_payload_keeps_the_placed_order() {
    // Arrange
    let input = input(pair_field());

    // Act
    let entries: Vec<String> = build_payload(&input)
        .expect("the fixture plates are numeric")
        .results
        .iter()
        .map(|row| row.entry.clone())
        .collect();

    // Assert
    assert_eq!(entries, vec!["Moss Ridge Riders", "Luca Ferrari"]);
}

#[test]
fn test_build_payload_maps_a_result_row() {
    // Arrange
    let mut result = sample("88", "Moss Ridge Riders", SnapshotKind::Solo, 11);
    result.tiebreak_card = Some(card("AD"));
    let mut row = placed(Some(1), result);
    row.draw_required = true;
    let input = input(vec![row]);

    // Act
    let mapped = build_payload(&input)
        .expect("the fixture plates are numeric")
        .results
        .remove(0);

    // Assert
    assert_eq!(
        (
            mapped.place,
            mapped.plate,
            mapped.entry.as_str(),
            mapped.entry_type.as_str(),
            mapped.laps,
            mapped.hand.as_str(),
            mapped.total.as_deref(),
            mapped.best_lap.as_deref(),
            mapped.sex.as_deref(),
            mapped.tie,
            mapped.dnf,
            mapped.dns,
        ),
        (
            1,
            88,
            "Moss Ridge Riders",
            "SOLO",
            11,
            "Three of a Kind — Nines",
            Some("5:32:00"),
            Some("30:00"),
            Some("F"),
            true,
            false,
            false,
        )
    );
    assert_eq!(
        (mapped.cards.len(), mapped.draw, mapped.logo),
        (5, Some(pair("A", "d")), None)
    );
}

#[test]
fn test_build_payload_maps_the_drawn_cards_in_deal_order() {
    // Arrange
    let input = input(vec![placed(
        Some(1),
        sample("88", "Moss Ridge Riders", SnapshotKind::Solo, 11),
    )]);

    // Act
    let drawn = build_payload(&input)
        .expect("the fixture plates are numeric")
        .results
        .remove(0)
        .drawn;

    // Assert
    assert_eq!(
        drawn,
        vec![
            pair("9", "s"),
            pair("9", "d"),
            pair("9", "c"),
            pair("K", "h"),
            pair("2", "s"),
        ]
    );
}

#[test]
fn test_build_payload_spells_the_team_kind_uppercase() {
    // Arrange
    let input = input(vec![placed(
        Some(1),
        sample("7", "Dirt Dynamos", SnapshotKind::Team, 10),
    )]);

    // Act
    let mapped = build_payload(&input)
        .expect("the fixture plates are numeric")
        .results
        .remove(0);

    // Assert
    assert_eq!((mapped.entry_type.as_str(), mapped.sex), ("TEAM", None));
}

#[test]
fn test_build_payload_maps_a_ten_and_a_joker_to_record_pairs() {
    // Arrange
    let cards = vec![card("10D"), Card::Joker];
    let mut result = sample("4", "Wild Bunch", SnapshotKind::Team, 2);
    result.hand = best_hand(&cards);
    result.cards = cards;
    let input = input(vec![placed(Some(1), result)]);

    // Act
    let mapped = build_payload(&input)
        .expect("the fixture plates are numeric")
        .results
        .remove(0);

    // Assert
    assert_eq!(
        (mapped.cards, mapped.drawn),
        (
            vec![pair("10", "d"), pair("JK", "j")],
            vec![pair("10", "d"), pair("JK", "j")],
        )
    );
}

#[test]
fn test_build_payload_given_a_blank_hand_leaves_the_hand_blank() {
    // Arrange
    let mut result = sample("9", "Rita Slow", SnapshotKind::Solo, 0);
    result.cards = Vec::new();
    result.hand = best_hand(&[]);
    let input = input(vec![placed(Some(1), result)]);

    // Act
    let hand = build_payload(&input)
        .expect("the fixture plates are numeric")
        .results
        .remove(0)
        .hand;

    // Assert
    assert_eq!(hand, String::new());
}

// ------------------------------------------------------- plates and logos

#[test]
fn test_build_payload_given_a_non_numeric_plate_returns_plate_not_numeric() {
    // Arrange
    let input = input(vec![placed(
        Some(1),
        sample("x7", "Moss Ridge Riders", SnapshotKind::Solo, 3),
    )]);

    // Act
    let result = build_payload(&input);

    // Assert
    assert!(matches!(
        result,
        Err(ExportError::PlateNotNumeric { plate }) if plate == "x7"
    ));
}

#[test]
fn test_build_payload_trims_a_whitespace_plate() {
    // Arrange
    let input = input(vec![placed(
        Some(1),
        sample(" 88 ", "Moss Ridge Riders", SnapshotKind::Solo, 3),
    )]);

    // Act
    let plate = build_payload(&input)
        .expect("a surrounded plate still parses")
        .results
        .remove(0)
        .plate;

    // Assert
    assert_eq!(plate, 88);
}

#[test]
fn test_build_payload_applies_the_team_logo_map_by_plate() {
    // Arrange
    let mut input = input(pair_field());
    input
        .team_logos
        .insert("88".to_owned(), "data:image/png;base64,AA==".to_owned());

    // Act
    let logos: Vec<Option<String>> = build_payload(&input)
        .expect("the fixture plates are numeric")
        .results
        .iter()
        .map(|row| row.logo.clone())
        .collect();

    // Assert
    assert_eq!(
        logos,
        vec![Some("data:image/png;base64,AA==".to_owned()), None]
    );
}

// ------------------------------------------------------------- the units

#[test]
fn test_build_payload_given_an_unverified_ride_carries_the_self_test_note() {
    // Arrange
    let mut input = input(pair_field());
    input.self_test_unverified = true;

    // Act
    let note = build_payload(&input)
        .expect("the fixture plates are numeric")
        .self_test_note;

    // Assert
    assert_eq!(note, Some(SELF_TEST_NOTE.to_owned()));
}

#[test]
fn test_build_payload_given_a_verified_ride_leaves_the_note_unset() {
    // Arrange
    let input = input(pair_field());

    // Act
    let note = build_payload(&input)
        .expect("the fixture plates are numeric")
        .self_test_note;

    // Assert
    assert_eq!(note, None);
}

// ------------------------------------------------------- the laps board

#[test]
fn test_build_payload_mixed_field_laps_board_lists_teams_then_solo_with_types() {
    // Arrange
    let input = input(field(6, 6));

    // Act
    let board = build_payload(&input)
        .expect("the fixture plates are numeric")
        .laps_board;

    // Assert
    assert_eq!(
        (
            board
                .iter()
                .map(|row| row.entry_type.clone())
                .collect::<Vec<_>>(),
            board
                .iter()
                .map(|row| row.entry.clone())
                .collect::<Vec<_>>(),
        ),
        (
            vec![
                "TEAM", "TEAM", "TEAM", "TEAM", "TEAM", "SOLO", "SOLO", "SOLO", "SOLO", "SOLO",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>(),
            vec![
                "Team 1", "Team 2", "Team 3", "Team 4", "Team 5", "Solo 1", "Solo 2", "Solo 3",
                "Solo 4", "Solo 5",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>(),
        )
    );
}

#[test]
fn test_build_payload_solo_event_laps_board_keeps_the_top_ten() {
    // Arrange
    let input = input(field(0, 11));

    // Act
    let board = build_payload(&input)
        .expect("the fixture plates are numeric")
        .laps_board;

    // Assert
    assert_eq!(
        (
            board.len(),
            board
                .iter()
                .map(|row| row.entry_type.as_str())
                .collect::<Vec<_>>(),
            board.first().map(|row| row.entry.as_str()),
            board.last().map(|row| row.entry.as_str()),
        ),
        (10, vec!["SOLO"; 10], Some("Solo 1"), Some("Solo 10"),)
    );
}

#[test]
fn test_build_payload_laps_board_rows_carry_the_plate_and_laps() {
    // Arrange
    let input = input(field(1, 0));

    // Act
    let row = build_payload(&input)
        .expect("the fixture plates are numeric")
        .laps_board
        .remove(0);

    // Assert
    assert_eq!(
        (
            row.plate,
            row.entry.as_str(),
            row.entry_type.as_str(),
            row.laps,
            row.total
        ),
        (500, "Team 1", "TEAM", 30, None)
    );
}

#[test]
fn test_build_payload_given_show_times_off_laps_rows_carry_no_total() {
    // Arrange
    let input = input_with(field(1, 0), options(false, true, false));

    // Act
    let totals: Vec<Option<String>> = build_payload(&input)
        .expect("the fixture plates are numeric")
        .laps_board
        .into_iter()
        .map(|row| row.total)
        .collect();

    // Assert
    assert_eq!(totals, vec![None]);
}

#[test]
fn test_build_payload_given_show_times_on_laps_rows_carry_the_total() {
    // Arrange
    let input = input_with(field(1, 0), options(true, true, false));

    // Act
    let total = build_payload(&input)
        .expect("the fixture plates are numeric")
        .laps_board
        .remove(0)
        .total;

    // Assert
    assert_eq!(total, Some("15:02:00".to_owned()));
}

#[test]
fn test_build_payload_given_laps_board_off_returns_no_laps_board() {
    // Arrange
    let input = input_with(field(6, 6), options(false, false, false));

    // Act
    let board = build_payload(&input)
        .expect("the fixture plates are numeric")
        .laps_board;

    // Assert
    assert!(matches!(board.as_slice(), []));
}

#[test]
fn test_build_payload_given_a_dns_row_keeps_it_off_the_boards() {
    // Arrange
    let mut rows = field(2, 2);
    rows.push(placed(
        None,
        sample("31", "Rita Slow", SnapshotKind::Solo, 0),
    ));
    let input = input_with(rows, options(true, true, true));

    // Act
    let payload = build_payload(&input).expect("the fixture plates are numeric");

    // Assert
    assert_eq!(
        (
            payload
                .laps_board
                .iter()
                .map(|row| row.entry.as_str())
                .collect::<Vec<_>>(),
            payload
                .time_board
                .iter()
                .map(|row| row.entry.as_str())
                .collect::<Vec<_>>(),
        ),
        (
            vec!["Team 1", "Team 2", "Solo 1", "Solo 2"],
            vec!["Team 1", "Team 2", "Solo 1", "Solo 2"],
        )
    );
}

#[test]
fn test_build_payload_given_a_dns_team_makes_the_field_mixed() {
    // Arrange
    let mut rows = field(0, 7);
    rows.insert(
        0,
        placed(None, sample("77", "Late Starters", SnapshotKind::Team, 0)),
    );
    let input = input(rows);

    // Act
    let board = build_payload(&input)
        .expect("the fixture plates are numeric")
        .laps_board;

    // Assert
    assert_eq!(
        (
            board.iter().filter(|row| row.entry_type == "TEAM").count(),
            board.len(),
        ),
        (0, 5)
    );
}

// ------------------------------------------------------- the time board

#[rstest]
#[case(false, false, Vec::new())]
#[case(false, true, Vec::new())]
#[case(true, false, Vec::new())]
#[case(
    true,
    true,
    vec!["Moss Ridge Riders", "Luca Ferrari"]
)]
fn test_build_payload_time_board_follows_show_times_and_time_board(
    #[case] show_times: bool,
    #[case] time_board: bool,
    #[case] expected: Vec<&str>,
) {
    // Arrange
    let input = input_with(pair_field(), options(show_times, true, time_board));

    // Act
    let entries: Vec<String> = build_payload(&input)
        .expect("the fixture plates are numeric")
        .time_board
        .into_iter()
        .map(|row| row.entry)
        .collect();

    // Assert
    assert_eq!(entries, expected);
}

#[test]
fn test_build_payload_time_board_stays_flat_across_kinds() {
    // Arrange
    let input = input_with(field(6, 6), options(true, true, true));

    // Act
    let entries: Vec<String> = build_payload(&input)
        .expect("the fixture plates are numeric")
        .time_board
        .into_iter()
        .map(|row| row.entry)
        .collect();

    // Assert
    assert_eq!(
        entries,
        vec![
            "Team 1", "Team 2", "Team 3", "Team 4", "Team 5", "Team 6", "Solo 1", "Solo 2",
            "Solo 3", "Solo 4",
        ]
    );
}

#[test]
fn test_build_payload_time_board_rows_carry_total_and_avg() {
    // Arrange
    let input = input_with(field(1, 0), options(true, true, true));

    // Act
    let row = build_payload(&input)
        .expect("the fixture plates are numeric")
        .time_board
        .remove(0);

    // Assert
    assert_eq!(
        (row.plate, row.entry.as_str(), row.laps, row.total, row.avg),
        (500, "Team 1", 30, "15:02:00".to_owned(), "30:04".to_owned())
    );
}

#[test]
fn test_build_payload_time_board_given_zero_laps_reads_an_em_dash_avg() {
    // Arrange
    let mut result = sample("9", "Zero Laps", SnapshotKind::Solo, 0);
    result.total_time = 0.0;
    let input = input_with(vec![placed(Some(1), result)], options(true, true, true));

    // Act
    let row = build_payload(&input)
        .expect("the fixture plates are numeric")
        .time_board
        .remove(0);

    // Assert
    assert_eq!((row.total.as_str(), row.avg.as_str()), ("0:00", "—"));
}

// ------------------------------------------------------- empty and DNS

#[test]
fn test_build_payload_given_an_empty_field_returns_empty_boards_and_zero_counts() {
    // Arrange
    let input = input(Vec::new());

    // Act
    let payload = build_payload(&input).expect("an empty field has no plate to reject");

    // Assert
    assert_eq!(
        (
            payload.event.entries,
            payload.event.laps,
            payload.event.cards,
            payload.results.len(),
            payload.laps_board.len(),
            payload.time_board.len(),
        ),
        (0, 0, 0, 0, 0, 0)
    );
}

#[test]
fn test_build_payload_given_a_dns_row_records_place_zero_and_null_times() {
    // Arrange
    let mut result = sample("31", "Rita Slow", SnapshotKind::Solo, 0);
    result.total_time = 1200.0;
    result.best_lap = 900.0;
    let input = input_with(vec![placed(None, result)], options(true, true, false));

    // Act
    let row = build_payload(&input)
        .expect("the fixture plates are numeric")
        .results
        .remove(0);

    // Assert
    assert_eq!(
        (row.place, row.dns, row.total, row.best_lap),
        (0, true, None, None)
    );
}

#[test]
fn test_build_payload_given_a_drawn_dns_row_records_no_draw_card() {
    // Arrange
    let mut result = sample("31", "Rita Slow", SnapshotKind::Solo, 0);
    result.tiebreak_card = Some(card("7D"));
    let input = input(vec![placed(None, result)]);

    // Act
    let draw = build_payload(&input)
        .expect("the fixture plates are numeric")
        .results
        .remove(0)
        .draw;

    // Assert
    assert_eq!(draw, None);
}

#[test]
fn test_build_payload_counts_a_dns_row_in_entries_with_zero_laps() {
    // Arrange
    let mut rows = field(2, 2);
    rows.push(placed(
        None,
        sample("31", "Rita Slow", SnapshotKind::Solo, 0),
    ));
    let input = input(rows);

    // Act
    let event = build_payload(&input)
        .expect("the fixture plates are numeric")
        .event;

    // Assert
    assert_eq!((event.entries, event.laps), (5, 98));
}
