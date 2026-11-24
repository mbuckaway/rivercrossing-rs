// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the page's per-kind section plan and the poster's plan.

#![expect(
    clippy::too_many_arguments,
    reason = "the fixture builders name each field of an entry separately"
)]

use std::collections::{BTreeMap, BTreeSet};

use pretty_assertions::assert_eq;
use rivercrossing_cards::Card;
use rivercrossing_hands::best_hand;
use rivercrossing_ride::EntryId;
use rivercrossing_standings::{EntryResult, Placed, SnapshotKind};
use rstest::rstest;
use time::{Date, Month};

use super::{poster_sections, sections};
use crate::payload::build_payload;
use crate::types::{
    EventInfo, ExportInput, ExportOptions, LapsBoardRow, RacePayload, ResultRow, RideInfo,
    TimeBoardRow,
};

fn card(code: &str) -> Card {
    Card::parse(code).expect("the fixture card code is valid")
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

/// Build a per-kind field: teams take places 1..N, then solo restarts at 1.
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

/// Build one DID-NOT-START row: unplaced, zero laps.
fn dns(plate: &str, name: &str, kind: SnapshotKind) -> Placed {
    placed(None, sample(plate, name, kind, 0))
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
        generated: "Generated 16:07, Sept 20 2026".to_owned(),
        logo_src: None,
        team_logos: BTreeMap::new(),
        riders: 0,
        self_test_unverified: false,
        tie_note: None,
    }
}

fn payload(placed: Vec<Placed>, options: ExportOptions) -> RacePayload {
    build_payload(&input_with(placed, options)).expect("the fixture plates are numeric")
}

fn options(show_times: bool, laps_board: bool, time_board: bool) -> ExportOptions {
    ExportOptions {
        show_times,
        laps_board,
        time_board,
        ..ExportOptions::default()
    }
}

fn event() -> EventInfo {
    EventInfo {
        kicker: "K".to_owned(),
        title: "T".to_owned(),
        meta: "M".to_owned(),
        organizer: "O".to_owned(),
        scorer: "S".to_owned(),
        generated: "G".to_owned(),
        entries: 1,
        laps: 1,
        cards: 0,
        riders: 0,
    }
}

fn row(plate: i64, entry: &str, entry_type: &str) -> ResultRow {
    ResultRow {
        place: 1,
        plate,
        entry: entry.to_owned(),
        entry_type: entry_type.to_owned(),
        laps: 1,
        hand: String::new(),
        total: None,
        best_lap: None,
        sex: None,
        tie: false,
        dnf: false,
        dns: false,
        cards: Vec::new(),
        drawn: Vec::new(),
        draw: None,
        logo: None,
    }
}

fn laps_row(plate: i64, entry: &str, entry_type: &str) -> LapsBoardRow {
    LapsBoardRow {
        plate,
        entry: entry.to_owned(),
        entry_type: entry_type.to_owned(),
        laps: 1,
        total: None,
    }
}

fn time_row(entry: &str) -> TimeBoardRow {
    TimeBoardRow {
        plate: 1,
        entry: entry.to_owned(),
        laps: 1,
        total: "0:01".to_owned(),
        avg: "0:01".to_owned(),
    }
}

fn assembled(
    results: Vec<ResultRow>,
    laps_board: Vec<LapsBoardRow>,
    time_board: Vec<TimeBoardRow>,
    options: ExportOptions,
) -> RacePayload {
    RacePayload {
        event: event(),
        options,
        tie_note: None,
        results,
        laps_board,
        time_board,
        self_test_note: None,
    }
}

/// Return the entry names of a result-row slice.
fn entries(rows: &[ResultRow]) -> Vec<&str> {
    rows.iter().map(|row| row.entry.as_str()).collect()
}

/// Return the entry names of a laps-board row slice.
fn board_entries(rows: &[LapsBoardRow]) -> Vec<&str> {
    rows.iter().map(|row| row.entry.as_str()).collect()
}

// -------------------------------------------------------- the mixed field

#[test]
fn test_sections_mixed_field_plans_per_kind_podiums_and_top_fives() {
    // Arrange
    let placed = field(6, 6);
    let payload = payload(placed.clone(), ExportOptions::default());

    // Act
    let plan = sections(&payload, &placed);

    // Assert
    assert_eq!(
        (
            entries(&plan.podium_teams),
            entries(&plan.podium_solo),
            entries(&plan.top_teams),
            entries(&plan.top_solo),
        ),
        (
            vec!["Team 1", "Team 2", "Team 3"],
            vec!["Solo 1", "Solo 2", "Solo 3"],
            vec!["Team 1", "Team 2", "Team 3", "Team 4", "Team 5"],
            vec!["Solo 1", "Solo 2", "Solo 3", "Solo 4", "Solo 5"],
        )
    );
}

#[test]
fn test_sections_mixed_field_keeps_every_row_in_the_full_field_lists() {
    // Arrange
    let placed = field(7, 7);
    let payload = payload(placed.clone(), ExportOptions::default());

    // Act
    let plan = sections(&payload, &placed);

    // Assert
    assert_eq!(
        (
            plan.teams.len(),
            plan.solo.len(),
            plan.teams.iter().all(|row| row.entry_type == "TEAM"),
            plan.solo.iter().all(|row| row.entry_type == "SOLO"),
        ),
        (7, 7, true, true)
    );
}

#[test]
fn test_sections_laps_boards_are_per_kind_not_a_crowded_combined_top_ten() {
    // Arrange
    let placed = field(8, 6);
    let mut payload = payload(placed.clone(), ExportOptions::default());
    payload.laps_board.clear();

    // Act
    let plan = sections(&payload, &placed);

    // Assert
    assert_eq!(
        (
            board_entries(&plan.laps_teams),
            board_entries(&plan.laps_solo),
            plan.laps_teams.first().map(|row| row.entry_type.as_str()),
            plan.laps_solo.first().map(|row| row.entry_type.as_str()),
        ),
        (
            vec!["Team 1", "Team 2", "Team 3", "Team 4", "Team 5"],
            vec!["Solo 1", "Solo 2", "Solo 3", "Solo 4", "Solo 5"],
            Some("TEAM"),
            Some("SOLO"),
        )
    );
}

#[rstest]
#[case(1, 1, (1, 1))]
#[case(3, 3, (3, 3))]
#[case(4, 4, (4, 4))]
#[case(5, 5, (5, 5))]
#[case(6, 6, (5, 5))]
#[case(11, 9, (5, 5))]
fn test_sections_mixed_field_caps_each_top_list_at_five(
    #[case] teams: u32,
    #[case] solo: u32,
    #[case] expected: (usize, usize),
) {
    // Arrange
    let placed = field(teams, solo);
    let payload = payload(placed.clone(), ExportOptions::default());

    // Act
    let plan = sections(&payload, &placed);

    // Assert
    assert_eq!((plan.top_teams.len(), plan.top_solo.len()), expected);
}

#[test]
fn test_sections_team_plates_holds_every_team_row_plate() {
    // Arrange
    let mut placed = field(2, 1);
    placed.push(dns("77", "Late Starters", SnapshotKind::Team));
    let payload = payload(placed.clone(), ExportOptions::default());

    // Act
    let plates = sections(&payload, &placed).team_plates;

    // Assert
    assert_eq!(plates, BTreeSet::from([77, 500, 501]));
}

// --------------------------------------------------------- the solo field

#[rstest]
#[case(0, 0)]
#[case(1, 1)]
#[case(9, 9)]
#[case(10, 10)]
#[case(11, 10)]
fn test_sections_solo_event_caps_the_top_list_at_ten(#[case] solo: u32, #[case] expected: usize) {
    // Arrange
    let placed = field(0, solo);
    let payload = payload(placed.clone(), ExportOptions::default());

    // Act
    let plan = sections(&payload, &placed);

    // Assert
    assert_eq!(
        (
            plan.top_solo.len(),
            plan.podium_teams.len(),
            plan.top_teams.len(),
            plan.laps_teams.len(),
        ),
        (expected, 0, 0, 0)
    );
}

#[test]
fn test_sections_solo_event_keeps_the_podium_at_three_and_board_at_ten() {
    // Arrange
    let placed = field(0, 11);
    let payload = payload(placed.clone(), ExportOptions::default());

    // Act
    let plan = sections(&payload, &placed);

    // Assert
    assert_eq!(
        (
            entries(&plan.podium_solo),
            plan.laps_solo.len(),
            plan.laps_solo.first().map(|row| row.entry.as_str()),
        ),
        (vec!["Solo 1", "Solo 2", "Solo 3"], 10, Some("Solo 1"))
    );
}

#[test]
fn test_sections_empty_field_plans_every_section_empty() {
    // Arrange
    let payload = payload(Vec::new(), ExportOptions::default());

    // Act
    let plan = sections(&payload, &[]);

    // Assert
    assert_eq!(
        (
            plan.podium_teams.len(),
            plan.podium_solo.len(),
            plan.top_teams.len(),
            plan.top_solo.len(),
            plan.laps_teams.len(),
            plan.laps_solo.len(),
            plan.time_board.len(),
            plan.teams.len(),
            plan.solo.len(),
        ),
        (0, 0, 0, 0, 0, 0, 0, 0, 0)
    );
}

// ---------------------------------------------------- the layout switches

#[test]
fn test_sections_treats_a_lowercase_team_row_as_a_team_event() {
    // Arrange
    let payload = assembled(
        vec![
            row(88, "Moss Ridge Riders", "team"),
            row(7, "Luca Ferrari", "solo"),
        ],
        Vec::new(),
        Vec::new(),
        ExportOptions::default(),
    );

    // Act
    let plan = sections(&payload, &[]);

    // Assert
    assert_eq!(
        (entries(&plan.podium_teams), entries(&plan.podium_solo)),
        (vec!["Moss Ridge Riders"], vec!["Luca Ferrari"])
    );
}

#[test]
fn test_sections_treats_display_suffixed_team_rows_as_a_team_event() {
    // Arrange
    let payload = assembled(
        vec![
            row(88, "Moss Ridge Riders", "TEAM ×4"),
            row(7, "Luca Ferrari", "SOLO"),
        ],
        Vec::new(),
        vec![time_row("Moss Ridge Riders")],
        ExportOptions::default(),
    );

    // Act
    let plan = sections(&payload, &[]);

    // Assert
    assert_eq!(
        (
            entries(&plan.podium_teams),
            plan.top_solo.len(),
            plan.laps_teams.len(),
            plan.time_board.first().map(|row| row.entry.as_str()),
        ),
        (vec!["Moss Ridge Riders"], 1, 0, Some("Moss Ridge Riders"))
    );
}

#[test]
fn test_sections_given_the_laps_board_off_returns_the_payload_plan_unchanged() {
    // Arrange
    let payload = assembled(
        vec![row(88, "Moss Ridge Riders", "TEAM"), row(7, "Luca", "SOLO")],
        vec![
            laps_row(88, "Moss Ridge Riders", "TEAM"),
            laps_row(7, "Luca", "SOLO"),
        ],
        Vec::new(),
        options(false, false, false),
    );

    // Act
    let plan = sections(&payload, &[]);

    // Assert
    assert_eq!(
        (
            board_entries(&plan.laps_teams),
            board_entries(&plan.laps_solo),
        ),
        (vec!["Moss Ridge Riders"], vec!["Luca"])
    );
}

#[test]
fn test_sections_given_an_unplaceable_plate_keeps_the_payload_plan() {
    // Arrange
    let payload = payload(
        vec![
            placed(Some(1), sample("500", "Team 1", SnapshotKind::Team, 3)),
            placed(Some(1), sample("10", "Solo 1", SnapshotKind::Solo, 2)),
        ],
        ExportOptions::default(),
    );
    let unplaceable = vec![placed(
        Some(1),
        sample("x1", "Bad Plate", SnapshotKind::Team, 1),
    )];

    // Act
    let plan = sections(&payload, &unplaceable);

    // Assert
    assert_eq!(
        (
            board_entries(&plan.laps_teams),
            board_entries(&plan.laps_solo),
        ),
        (vec!["Team 1"], vec!["Solo 1"])
    );
}

#[test]
fn test_sections_given_an_unplaceable_solo_plate_keeps_the_payload_plan() {
    // Arrange
    let payload = payload(
        vec![
            placed(Some(1), sample("500", "Team 1", SnapshotKind::Team, 3)),
            placed(Some(1), sample("10", "Solo 1", SnapshotKind::Solo, 2)),
        ],
        ExportOptions::default(),
    );
    let unplaceable = vec![
        placed(Some(1), sample("500", "Team 1", SnapshotKind::Team, 3)),
        placed(Some(1), sample("x2", "Bad Plate", SnapshotKind::Solo, 1)),
    ];

    // Act
    let plan = sections(&payload, &unplaceable);

    // Assert
    assert_eq!(
        (
            board_entries(&plan.laps_teams),
            board_entries(&plan.laps_solo),
        ),
        (vec!["Team 1"], vec!["Solo 1"])
    );
}

#[test]
fn test_sections_solo_event_given_an_unplaceable_plate_keeps_the_payload_plan() {
    // Arrange
    let payload = payload(
        vec![placed(
            Some(1),
            sample("10", "Solo 1", SnapshotKind::Solo, 2),
        )],
        ExportOptions::default(),
    );
    let unplaceable = vec![placed(
        Some(1),
        sample("x3", "Bad Plate", SnapshotKind::Solo, 1),
    )];

    // Act
    let plan = sections(&payload, &unplaceable);

    // Assert
    assert_eq!(
        (
            board_entries(&plan.laps_teams),
            board_entries(&plan.laps_solo),
        ),
        (Vec::new(), vec!["Solo 1"])
    );
}

#[test]
fn test_sections_keeps_laps_boards_empty_when_the_option_is_off() {
    // Arrange
    let placed = field(6, 6);
    let payload = payload(placed.clone(), options(false, false, false));

    // Act
    let plan = sections(&payload, &placed);

    // Assert
    assert_eq!((plan.laps_teams.len(), plan.laps_solo.len()), (0, 0));
}

#[test]
fn test_sections_flattens_the_time_board_from_the_payload() {
    // Arrange
    let placed = field(6, 6);
    let payload = payload(placed.clone(), options(true, true, true));

    // Act
    let plan = sections(&payload, &placed);

    // Assert
    assert_eq!(
        (
            plan.time_board.len(),
            plan.time_board.first().map(|row| row.entry.as_str()),
        ),
        (payload.time_board.len(), Some("Team 1"))
    );
}

// ---------------------------------------------------------- the DNS rows

#[test]
fn test_sections_given_a_dns_row_keeps_it_in_the_full_field_only() {
    // Arrange
    let mut placed = field(2, 2);
    placed.push(dns("31", "Rita Slow", SnapshotKind::Solo));
    let payload = payload(placed.clone(), ExportOptions::default());

    // Act
    let plan = sections(&payload, &placed);

    // Assert
    assert_eq!(
        (
            entries(&plan.solo),
            entries(&plan.podium_solo),
            entries(&plan.top_solo),
            entries(&plan.podium_teams),
            entries(&plan.top_teams),
        ),
        (
            vec!["Solo 1", "Solo 2", "Rita Slow"],
            vec!["Solo 1", "Solo 2"],
            vec!["Solo 1", "Solo 2"],
            vec!["Team 1", "Team 2"],
            vec!["Team 1", "Team 2"],
        )
    );
}

#[test]
fn test_sections_given_a_dns_row_leaves_it_off_the_laps_boards() {
    // Arrange
    let mut placed = field(2, 2);
    placed.push(dns("31", "Rita Slow", SnapshotKind::Solo));
    let payload = payload(placed.clone(), options(false, true, false));

    // Act
    let plan = sections(&payload, &placed);

    // Assert
    assert_eq!(
        (
            board_entries(&plan.laps_teams),
            board_entries(&plan.laps_solo),
        ),
        (vec!["Team 1", "Team 2"], vec!["Solo 1", "Solo 2"])
    );
}

#[test]
fn test_sections_given_only_dns_teams_keeps_the_mixed_solo_cap() {
    // Arrange
    let mut placed = vec![dns("77", "Late Starters", SnapshotKind::Team)];
    placed.extend(field(0, 7));
    let payload = payload(placed.clone(), options(false, true, false));

    // Act
    let plan = sections(&payload, &placed);

    // Assert
    assert_eq!((plan.top_solo.len(), plan.laps_solo.len()), (5, 5));
}

// -------------------------------------------------------------- the poster

#[test]
fn test_poster_sections_mixed_field_gives_titled_three_and_three() {
    // Arrange
    let payload = payload(field(6, 6), ExportOptions::default());

    // Act
    let sections = poster_sections(&payload);

    // Assert
    assert_eq!(
        sections
            .iter()
            .map(|(heading, rows)| (heading.as_str(), entries(rows)))
            .collect::<Vec<_>>(),
        vec![
            ("Teams", vec!["Team 1", "Team 2", "Team 3"]),
            ("Solo riders", vec!["Solo 1", "Solo 2", "Solo 3"]),
        ]
    );
}

#[test]
fn test_poster_sections_solo_event_gives_one_untitled_five() {
    // Arrange
    let payload = payload(field(0, 7), ExportOptions::default());

    // Act
    let sections = poster_sections(&payload);

    // Assert
    assert_eq!(
        sections
            .iter()
            .map(|(heading, rows)| (heading.as_str(), entries(rows)))
            .collect::<Vec<_>>(),
        vec![("", vec!["Solo 1", "Solo 2", "Solo 3", "Solo 4", "Solo 5"])]
    );
}

#[test]
fn test_poster_sections_team_only_event_still_yields_the_empty_solo_section() {
    // Arrange
    let payload = payload(field(3, 0), ExportOptions::default());

    // Act
    let sections = poster_sections(&payload);

    // Assert
    assert_eq!(
        (
            sections.len(),
            sections
                .first()
                .map(|(heading, rows)| (heading.as_str(), entries(rows))),
            sections
                .get(1)
                .map(|(heading, rows)| (heading.as_str(), rows.len())),
        ),
        (
            2,
            Some(("Teams", vec!["Team 1", "Team 2", "Team 3"])),
            Some(("Solo riders", 0)),
        )
    );
}

#[test]
fn test_poster_sections_given_a_dns_row_leaves_it_off_the_cards() {
    // Arrange
    let mut placed = field(2, 2);
    placed.push(dns("77", "Late Starters", SnapshotKind::Team));
    placed.push(dns("31", "Rita Slow", SnapshotKind::Solo));
    let payload = payload(placed, ExportOptions::default());

    // Act
    let sections = poster_sections(&payload);

    // Assert
    assert_eq!(
        sections
            .iter()
            .map(|(heading, rows)| (heading.as_str(), entries(rows)))
            .collect::<Vec<_>>(),
        vec![
            ("Teams", vec!["Team 1", "Team 2"]),
            ("Solo riders", vec!["Solo 1", "Solo 2"]),
        ]
    );
}

#[test]
fn test_poster_sections_empty_field_gives_one_empty_untitled_section() {
    // Arrange
    let payload = payload(Vec::new(), ExportOptions::default());

    // Act
    let sections = poster_sections(&payload);

    // Assert
    assert_eq!(
        sections
            .iter()
            .map(|(heading, rows)| (heading.as_str(), rows.len()))
            .collect::<Vec<_>>(),
        vec![("", 0)]
    );
}
