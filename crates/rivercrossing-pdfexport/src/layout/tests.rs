// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the pure page model and the report and poster builders.

#![expect(
    clippy::too_many_arguments,
    reason = "the fixture builders name each field of a synthetic entry separately"
)]

use std::collections::BTreeMap;

use pretty_assertions::assert_eq;
use rivercrossing_cards::Card;
use rivercrossing_hands::best_hand;
use rivercrossing_htmlexport::payload::build_payload;
use rivercrossing_htmlexport::sections::sections;
use rivercrossing_htmlexport::types::{
    CardPair, ExportInput, ExportOptions, RacePayload, ResultRow, RideInfo, SELF_TEST_NOTE,
    Sections,
};
use rivercrossing_ride::EntryId;
use rivercrossing_standings::{EntryResult, Placed, SnapshotKind};
use rstest::rstest;
use time::{Date, Month};

use super::{
    Canvas, CardBox, Footer, Op, Page, Span, bragging_clause, card_color, draw_marker, format_km,
    group_digits, is_team_row, laps_cell, pair_text, poster_pages, report_pages, suit_glyph,
    time_cell, wrap,
};
use crate::error::PdfError;
use crate::fonts::FontFace;
use crate::geometry::{
    FOOTER_GAP_IN, INK, LETTER_HEIGHT_PT, POINTS_PER_INCH, STEEL, SUIT_RED, page_size,
};

const GENERATED: &str = "Generated 16:07, Sept 20 2026";

/// A minimal well-formed PNG signature; the layout validates the header only.
const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

const TEAM_NAMES: [&str; 6] = [
    "Team Alpha",
    "Team Bravo",
    "Team Charlie",
    "Team Delta",
    "Team Echo",
    "Team Foxtrot",
];

const TEAM_SOLO_NAMES: [&str; 6] = [
    "Rider One",
    "Rider Two",
    "Rider Three",
    "Rider Four",
    "Rider Five",
    "Rider Six",
];

const SOLO_12: [&str; 12] = [
    "Rider One",
    "Rider Two",
    "Rider Three",
    "Rider Four",
    "Rider Five",
    "Rider Six",
    "Rider Seven",
    "Rider Eight",
    "Rider Nine",
    "Rider Ten",
    "Rider Eleven",
    "Rider Twelve",
];

// ------------------------------------------------------------- fixtures

fn card(code: &str) -> Card {
    Card::parse(code).expect("the fixture card code is valid")
}

fn five_cards() -> Vec<Card> {
    ["9S", "9D", "9C", "KH", "2S"]
        .iter()
        .map(|code| card(code))
        .collect()
}

fn pair(rank: &str, suit: &str) -> CardPair {
    CardPair {
        rank: rank.to_owned(),
        suit: suit.to_owned(),
    }
}

fn result(plate: &str, name: &str, kind: SnapshotKind, laps: u32) -> EntryResult {
    result_with_cards(plate, name, kind, laps, five_cards())
}

fn result_with_cards(
    plate: &str,
    name: &str,
    kind: SnapshotKind,
    laps: u32,
    cards: Vec<Card>,
) -> EntryResult {
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

fn with_dnf(mut result: EntryResult) -> EntryResult {
    result.dnf = true;
    result
}

fn with_draw(mut result: EntryResult, draw: Card) -> EntryResult {
    result.tiebreak_card = Some(draw);
    result
}

fn place_row(place: Option<u32>, result: EntryResult) -> Placed {
    Placed {
        place,
        result,
        tie_note: None,
        draw_required: false,
    }
}

fn ride() -> RideInfo {
    RideInfo {
        name: "Test Poker Run 2026".to_owned(),
        event_date: Date::from_calendar_date(2026, Month::June, 6).expect("the fixture date"),
        venue: "Test Venue".to_owned(),
        lap_km: 8.0,
        organizer: "Test Org".to_owned(),
        scorer: "T. Ester".to_owned(),
    }
}

fn all_options() -> ExportOptions {
    ExportOptions {
        show_times: true,
        laps_board: true,
        time_board: true,
        full_field: true,
        all_cards: true,
        lap_km: 8.0,
    }
}

fn build(placed: Vec<Placed>, options: ExportOptions, self_test: bool) -> (RacePayload, Sections) {
    let input = ExportInput {
        ride: ride(),
        placed,
        options,
        generated: GENERATED.to_owned(),
        logo_src: None,
        team_logos: BTreeMap::new(),
        riders: 0,
        self_test_unverified: self_test,
        tie_note: None,
    };
    let payload = build_payload(&input).expect("the fixture payload builds");
    let plan = sections(&payload, &input.placed);
    (payload, plan)
}

fn team_event(options: ExportOptions, self_test: bool) -> (RacePayload, Sections) {
    let mut placed = Vec::new();
    for (index, name) in TEAM_NAMES.iter().enumerate() {
        let place = u32::try_from(index).expect("the fixture index fits") + 1;
        let laps = 20u32.saturating_sub(u32::try_from(index).expect("the index fits"));
        let bib = format!("{}", 901 + index);
        placed.push(place_row(
            Some(place),
            result(&bib, name, SnapshotKind::Team, laps),
        ));
    }
    for (index, name) in TEAM_SOLO_NAMES.iter().enumerate() {
        let place = u32::try_from(index).expect("the fixture index fits") + 7;
        let laps = 10u32.saturating_sub(u32::try_from(index).expect("the index fits"));
        let bib = format!("{}", 801 + index);
        placed.push(place_row(
            Some(place),
            result(&bib, name, SnapshotKind::Solo, laps),
        ));
    }
    build(placed, options, self_test)
}

fn team_only_event(count: usize, options: ExportOptions) -> (RacePayload, Sections) {
    let mut placed = Vec::new();
    for (index, name) in TEAM_NAMES.iter().take(count).enumerate() {
        let place = u32::try_from(index).expect("the fixture index fits") + 1;
        let laps = 20u32.saturating_sub(u32::try_from(index).expect("the index fits"));
        let bib = format!("{}", 901 + index);
        placed.push(place_row(
            Some(place),
            result(&bib, name, SnapshotKind::Team, laps),
        ));
    }
    build(placed, options, false)
}

fn solo_event(names: &[&str], options: ExportOptions, self_test: bool) -> (RacePayload, Sections) {
    let mut placed = Vec::new();
    for (index, name) in names.iter().enumerate() {
        let number = u32::try_from(index).expect("the fixture index fits");
        let plate = format!("{}", 701 + index);
        let laps = 60u32.saturating_sub(number);
        placed.push(place_row(
            Some(number + 1),
            result(&plate, name, SnapshotKind::Solo, laps),
        ));
    }
    build(placed, options, self_test)
}

fn sample_row(hand: &str) -> ResultRow {
    ResultRow {
        place: 1,
        plate: 1,
        entry: "Entry".to_owned(),
        entry_type: "SOLO".to_owned(),
        laps: 5,
        hand: hand.to_owned(),
        total: Some("5:00".to_owned()),
        best_lap: Some("1:00".to_owned()),
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

// ------------------------------------------------------------- inspection

fn text_of(op: &Op) -> Option<&str> {
    match op {
        Op::Text { text, .. } => Some(text.as_str()),
        _ => None,
    }
}

fn texts(pages: &[Page]) -> Vec<&str> {
    pages
        .iter()
        .flat_map(|page| page.ops.iter())
        .filter_map(text_of)
        .collect()
}

fn page_texts(pages: &[Page], index: usize) -> Vec<&str> {
    pages
        .get(index)
        .map(|page| page.ops.iter().filter_map(text_of).collect())
        .unwrap_or_default()
}

fn joined(pages: &[Page]) -> String {
    texts(pages).join(" ")
}

fn has_text(pages: &[Page], needle: &str) -> bool {
    texts(pages).into_iter().any(|text| text == needle)
}

fn has_name(pages: &[Page], name: &str) -> bool {
    texts(pages).into_iter().any(|text| text.ends_with(name))
}

fn count_text(pages: &[Page], needle: &str) -> usize {
    texts(pages)
        .into_iter()
        .filter(|text| *text == needle)
        .count()
}

fn index_of(pages: &[Page], needle: &str) -> usize {
    texts(pages)
        .into_iter()
        .position(|text| text == needle)
        .unwrap_or(usize::MAX)
}

fn region<'a>(pages: &'a [Page], start: &str, end: Option<&str>) -> Vec<&'a str> {
    let all = texts(pages);
    let start_at = all
        .iter()
        .position(|text| *text == start)
        .map_or(all.len(), |index| index + 1);
    let end_at = end
        .and_then(|needle| all.iter().position(|text| *text == needle))
        .unwrap_or(all.len());
    all.get(start_at..end_at)
        .map_or_else(Vec::new, <[&str]>::to_vec)
}

fn count_region(pages: &[Page], start: &str, end: &str, names: &[&str]) -> usize {
    region(pages, start, Some(end))
        .into_iter()
        .filter(|text| names.contains(text))
        .count()
}

fn max_rect_bottom(pages: &[Page]) -> f32 {
    pages
        .iter()
        .flat_map(|page| page.ops.iter())
        .filter_map(|op| match op {
            Op::FilledRect { y, h, .. } => Some(*y + *h),
            _ => None,
        })
        .fold(0.0, f32::max)
}

fn assert_in_order(pages: &[Page], headings: &[&str]) {
    let positions: Vec<usize> = headings
        .iter()
        .map(|heading| index_of(pages, heading))
        .collect();
    assert!(
        !positions.contains(&usize::MAX),
        "a heading is missing: {positions:?}"
    );
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "headings are out of order: {positions:?}"
    );
}

fn assert_page_furniture(pages: &[Page], generated: &str, credits: &str) {
    let total = pages.len();
    let found = (0..total).all(|index| {
        let page = page_texts(pages, index);
        let page_line = format!(
            "Page {} of {total} · {generated} · RiverCrossing",
            index + 1
        );
        page.contains(&page_line.as_str()) && page.contains(&credits)
    });
    assert!(found, "a page is missing its footer furniture");
}

fn blank_canvas() -> Canvas {
    Canvas::new(
        page_size(true),
        None,
        Footer {
            credits: String::new(),
            generated: String::new(),
            numbered: false,
        },
    )
}

// ------------------------------------------------------- the report order

#[test]
fn test_layout_report_draws_the_sections_in_the_oracle_order() {
    // Arrange
    let mut options = all_options();
    options.all_cards = false;
    let (payload, plan) = team_event(options, false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert_in_order(
        &pages,
        &[
            "Best hands — teams",
            "Best hands — solo riders",
            "Top teams",
            "Top solo riders",
            "Most laps — teams",
            "Most laps — solo riders",
            "Fastest — laps then time",
            "Full field",
            "Teams",
            "Solo riders",
        ],
    );
}

#[test]
fn test_layout_report_team_event_draws_three_podium_cards_per_kind() {
    // Arrange
    let (payload, plan) = team_event(all_options(), false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");
    let team_region = region(
        &pages,
        "Best hands — teams",
        Some("Best hands — solo riders"),
    );
    let solo_region = region(&pages, "Best hands — solo riders", Some("Top teams"));

    // Assert
    assert_eq!(
        team_region
            .iter()
            .filter(|text| TEAM_NAMES.contains(text))
            .count(),
        3
    );
    assert_eq!(
        solo_region
            .iter()
            .filter(|text| TEAM_SOLO_NAMES.iter().any(|name| text.ends_with(*name)))
            .count(),
        3
    );
    assert!(!team_region.contains(&"Team Delta"));
    assert!(!solo_region.iter().any(|text| text.ends_with("Rider Four")));
}

#[test]
fn test_layout_report_team_event_draws_five_top_list_rows_per_kind() {
    // Arrange
    let (payload, plan) = team_event(all_options(), false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");
    let team_region = region(&pages, "Top teams", Some("Top solo riders"));
    let solo_region = region(&pages, "Top solo riders", Some("Most laps — teams"));

    // Assert
    assert_eq!(
        count_region(&pages, "Top teams", "Top solo riders", &TEAM_NAMES),
        5
    );
    assert_eq!(
        count_region(
            &pages,
            "Top solo riders",
            "Most laps — teams",
            &TEAM_SOLO_NAMES
        ),
        5
    );
    assert!(!team_region.contains(&"Team Foxtrot"));
    assert!(!solo_region.contains(&"Rider Six"));
}

#[test]
fn test_layout_report_team_event_draws_five_laps_rows_per_kind() {
    // Arrange
    let (payload, plan) = team_event(all_options(), false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert_eq!(
        count_region(
            &pages,
            "Most laps — teams",
            "Most laps — solo riders",
            &TEAM_NAMES,
        ),
        5
    );
    assert_eq!(
        count_region(
            &pages,
            "Most laps — solo riders",
            "Fastest — laps then time",
            &TEAM_SOLO_NAMES,
        ),
        5
    );
}

#[test]
fn test_layout_report_solo_event_draws_a_top_ten() {
    // Arrange
    let mut options = all_options();
    options.time_board = false;
    let (payload, plan) = solo_event(&SOLO_12, options, false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");
    let top = region(&pages, "Top ten", Some("Most laps"));

    // Assert
    assert_eq!(count_region(&pages, "Top ten", "Most laps", &SOLO_12), 10);
    assert!(!top.contains(&"Rider Eleven"));
}

#[test]
fn test_layout_report_solo_event_draws_a_ten_row_laps_board() {
    // Arrange
    let mut options = all_options();
    options.time_board = false;
    let (payload, plan) = solo_event(&SOLO_12, options, false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert_eq!(
        count_region(&pages, "Most laps", "Full field", &SOLO_12),
        10
    );
}

#[test]
fn test_layout_report_time_board_draws_ten_flat_rows() {
    // Arrange
    let (payload, plan) = solo_event(&SOLO_12, all_options(), false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert_eq!(
        count_region(&pages, "Fastest — laps then time", "Full field", &SOLO_12),
        10
    );
}

// --------------------------------------------------------------- plates

#[test]
fn test_layout_report_team_rows_never_show_a_plate() {
    // Arrange
    let (payload, plan) = team_event(all_options(), false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");
    let all = joined(&pages);

    // Assert
    assert!(!all.contains("#901"), "a team plate leaked into the report");
    assert!(all.contains("#801"), "a solo plate is missing");
}

#[test]
fn test_layout_report_time_board_blanks_a_team_plate() {
    // Arrange
    let (payload, plan) = team_event(all_options(), false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");
    let board = region(&pages, "Fastest — laps then time", Some("Full field"));

    // Assert
    assert!(board.contains(&"Team Alpha"));
    assert!(!board.iter().any(|text| text.contains("#901")));
    assert!(board.contains(&"#801"));
}

#[test]
fn test_layout_report_full_field_draws_teams_before_solo_riders() {
    // Arrange
    let (payload, plan) = team_event(all_options(), false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");
    let all = texts(&pages);
    let full = all
        .iter()
        .position(|text| *text == "Full field")
        .expect("the full field heading is present");
    let teams = all
        .iter()
        .skip(full)
        .position(|text| *text == "Teams")
        .map(|index| index + full)
        .expect("the Teams subsection is present");
    let solo = all
        .iter()
        .skip(full)
        .position(|text| *text == "Solo riders")
        .map(|index| index + full)
        .expect("the Solo riders subsection is present");

    // Assert
    assert!(teams < solo);
}

// ----------------------------------------------------------- DNS and DNF

#[test]
fn test_layout_report_dns_row_prints_blank_place_dns_laps_and_blank_times() {
    // Arrange
    let ranked = place_row(Some(1), result("701", "Rider One", SnapshotKind::Solo, 5));
    let dns = place_row(
        None,
        result_with_cards("702", "No Show", SnapshotKind::Solo, 0, Vec::new()),
    );
    let mut options = all_options();
    options.all_cards = false;
    let (payload, plan) = build(vec![ranked, dns], options, false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert!(has_text(&pages, "DNS"));
    assert!(has_text(&pages, "No Show (F)"));
    assert!(!has_text(&pages, "0"));
    assert!(!joined(&pages).contains("None"));
}

#[test]
fn test_layout_report_dnf_solo_row_suffixes_the_sex_then_dnf() {
    // Arrange
    let entry = with_dnf(result("701", "Luca Ferrari", SnapshotKind::Solo, 5));
    let mut options = all_options();
    options.all_cards = false;
    let (payload, plan) = build(vec![place_row(Some(1), entry)], options, false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert!(has_text(&pages, "Luca Ferrari (F) DNF"));
}

#[test]
fn test_layout_report_solo_row_without_sex_omits_the_parentheses() {
    // Arrange
    let mut entry = result("701", "Luca Ferrari", SnapshotKind::Solo, 5);
    entry.sex = None;
    let mut options = all_options();
    options.all_cards = false;
    let (payload, plan) = build(vec![place_row(Some(1), entry)], options, false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert!(has_text(&pages, "Luca Ferrari"));
    assert!(
        !texts(&pages)
            .iter()
            .any(|text| text.contains("Luca Ferrari ("))
    );
}

#[test]
fn test_layout_report_dnf_team_row_suffixes_the_name_with_dnf() {
    // Arrange
    let entry = with_dnf(result("901", "Team Alpha", SnapshotKind::Team, 10));
    let mut options = all_options();
    options.all_cards = false;
    let (payload, plan) = build(vec![place_row(Some(1), entry)], options, false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert!(has_text(&pages, "Team Alpha DNF"));
}

// ----------------------------------------------------------- drawn cards

#[test]
fn test_layout_report_drawn_marker_follows_a_hand_prose() {
    // Arrange
    let entry = with_draw(
        result("701", "Rider One", SnapshotKind::Solo, 5),
        card("AH"),
    );
    let mut options = all_options();
    options.all_cards = false;
    let (payload, plan) = build(vec![place_row(Some(1), entry)], options, false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert!(has_text(&pages, "THREE OF A KIND — NINES"));
    assert!(has_text(&pages, " · DRAW A♥"));
}

#[test]
fn test_layout_report_drawn_marker_has_no_separator_without_hand_prose() {
    // Arrange
    let entry = with_draw(
        result_with_cards("701", "Rider One", SnapshotKind::Solo, 5, Vec::new()),
        card("AH"),
    );
    let mut options = all_options();
    options.all_cards = false;
    let (payload, plan) = build(vec![place_row(Some(1), entry)], options, false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert!(has_text(&pages, "DRAW A♥"));
    assert!(!has_text(&pages, " · DRAW A♥"));
}

#[test]
fn test_layout_report_drawn_row_names_every_card_in_draw_order() {
    // Arrange
    let entry = result("701", "Rider One", SnapshotKind::Solo, 5);
    let (payload, plan) = build(vec![place_row(Some(1), entry)], all_options(), false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");
    let all = joined(&pages);

    // Assert
    assert!(
        all.contains("All 5 cards, in draw order: 9♠ 9♦ 9♣ K♥ 2♠"),
        "{all}"
    );
}

#[test]
fn test_layout_report_full_field_note_names_the_organizer_option() {
    // Arrange
    let (payload, plan) = team_event(all_options(), false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");
    let all = joined(&pages);

    // Assert
    assert!(all.contains("teams ranked against teams, solo riders against solo riders"));
    assert!(all.contains("This export includes every card drawn (organizer option)."));
}

#[test]
fn test_layout_report_full_field_note_omits_the_option_when_off() {
    // Arrange
    let mut options = all_options();
    options.all_cards = false;
    let (payload, plan) = solo_event(&SOLO_12, options, false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");
    let all = joined(&pages);

    // Assert
    assert!(!all.contains("teams ranked against teams"));
    assert!(!all.contains("This export includes every card drawn"));
}

#[test]
fn test_layout_report_can_omit_the_full_field() {
    // Arrange
    let mut options = all_options();
    options.full_field = false;
    let (payload, plan) = solo_event(&SOLO_12, options, false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert!(!has_text(&pages, "Full field"));
    assert!(has_text(&pages, "Top ten"));
}

#[test]
fn test_layout_report_without_times_omits_the_time_columns() {
    // Arrange
    let mut options = all_options();
    options.show_times = false;
    options.time_board = false;
    let (payload, plan) = team_event(options, false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert!(!has_text(&pages, "Total time"));
    assert!(!has_text(&pages, "Best lap"));
    assert!(!has_text(&pages, "Total"));
    assert!(!has_text(&pages, "Fastest — laps then time"));
}

#[test]
fn test_layout_report_team_only_field_skips_the_solo_sections() {
    // Arrange
    let (payload, plan) = team_only_event(3, all_options());

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert!(has_text(&pages, "Best hands — teams"));
    assert!(!has_text(&pages, "Best hands — solo riders"));
    assert!(!has_text(&pages, "Best hands — top 3"));
    assert!(has_text(&pages, "Top teams"));
    assert!(!has_text(&pages, "Top solo riders"));
    assert!(!has_text(&pages, "Top ten"));
    assert!(has_text(&pages, "Most laps — teams"));
    assert!(!has_text(&pages, "Most laps — solo riders"));
    assert!(!has_text(&pages, "Most laps"));
    assert!(!has_text(&pages, "Solo riders"));
    assert!(has_text(&pages, "Teams"));
}

#[test]
fn test_layout_poster_team_only_field_skips_the_solo_section() {
    // Arrange
    let (payload, _plan) = team_only_event(3, all_options());

    // Act
    let pages = poster_pages(&payload, true, None).expect("the poster builds");

    // Assert
    assert!(has_text(&pages, "Teams"));
    assert!(!has_text(&pages, "Solo riders"));
}

#[test]
fn test_layout_poster_draws_the_self_test_note() {
    // Arrange
    let (payload, _plan) = solo_event(&SOLO_12, all_options(), true);

    // Act
    let pages = poster_pages(&payload, true, None).expect("the poster builds");

    // Assert
    assert!(joined(&pages).contains(SELF_TEST_NOTE));
}

#[test]
fn test_layout_poster_draws_the_organizer_logo() {
    // Arrange
    let (payload, _plan) = solo_event(&SOLO_12, all_options(), false);

    // Act
    let pages = poster_pages(&payload, true, Some(PNG)).expect("the poster builds");

    // Assert
    assert!(
        pages
            .iter()
            .flat_map(|page| page.ops.iter())
            .any(|op| matches!(op, Op::Image { .. }))
    );
}

// ---------------------------------------------------------------- cover

#[test]
fn test_layout_report_cover_draws_the_kicker_meta_and_counters() {
    // Arrange
    let mut options = all_options();
    options.all_cards = false;
    let (mut payload, plan) = solo_event(&SOLO_12, options, false);
    payload.event.entries = 1234;
    payload.event.laps = 5678;
    payload.event.cards = 90;
    payload.event.riders = 1234;

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert!(has_text(&pages, "Official results · poker run"));
    assert!(has_text(
        &pages,
        "entries · laps · cards dealt · unique riders"
    ));
    assert!(has_text(&pages, "1,234 · 5,678 · 90 · 1,234"));
    assert!(joined(&pages).contains(&payload.event.meta));
}

#[test]
fn test_layout_report_cover_draws_the_disclaimer_and_bragging_clause() {
    // Arrange
    let (payload, plan) = solo_event(&SOLO_12, all_options(), false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");
    let all = joined(&pages);

    // Assert
    assert!(all.contains(
        "It's not a race, it's a poker run — placings are by best poker hand. Lap counts and times below are unofficial and shown for bragging rights only."
    ));
}

#[test]
fn test_layout_report_cover_draws_the_self_test_note() {
    // Arrange
    let (payload, plan) = solo_event(&SOLO_12, all_options(), true);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert!(joined(&pages).contains(SELF_TEST_NOTE));
}

#[test]
fn test_layout_report_cover_draws_the_organizer_logo() {
    // Arrange
    let (payload, plan) = solo_event(&SOLO_12, all_options(), false);

    // Act
    let pages = report_pages(&payload, &plan, true, Some(PNG)).expect("the report builds");
    let image = pages
        .iter()
        .flat_map(|page| page.ops.iter())
        .find_map(|op| match op {
            Op::Image { x, y, w, h, png } => Some((*x, *y, *w, *h, png.as_slice())),
            _ => None,
        })
        .expect("the cover carries the logo");

    // Assert
    assert_eq!(image.4, PNG);
    assert!((image.2 - 0.8 * POINTS_PER_INCH).abs() < 1e-3);
    assert!((image.3 - 0.0).abs() < f32::EPSILON);
    assert!(image.0 > 0.0);
    assert!(image.1 > 0.0);
}

#[test]
fn test_layout_report_draws_no_image_without_a_logo() {
    // Arrange
    let (payload, plan) = solo_event(&SOLO_12, all_options(), false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert!(
        !pages
            .iter()
            .flat_map(|page| page.ops.iter())
            .any(|op| matches!(op, Op::Image { .. }))
    );
}

#[test]
fn test_layout_report_rejects_a_non_png_logo() {
    // Arrange
    let (payload, plan) = solo_event(&SOLO_12, all_options(), false);

    // Act
    let result = report_pages(&payload, &plan, true, Some(b"not a png"));

    // Assert
    assert!(matches!(result, Err(PdfError::Logo(_))));
}

#[test]
fn test_layout_poster_rejects_a_non_png_logo() {
    // Arrange
    let (payload, _plan) = solo_event(&SOLO_12, all_options(), false);

    // Act
    let result = poster_pages(&payload, true, Some(b"not a png"));

    // Assert
    assert!(matches!(result, Err(PdfError::Logo(_))));
}

// ---------------------------------------------------------- captions

#[test]
fn test_layout_report_laps_caption_names_a_whole_kilometre() {
    // Arrange
    let (payload, plan) = solo_event(&SOLO_12, all_options(), false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert!(has_text(&pages, "Unofficial — 8 km per lap."));
}

#[test]
fn test_layout_report_laps_caption_names_a_fractional_kilometre() {
    // Arrange
    let mut options = all_options();
    options.lap_km = 8.5;
    let (payload, plan) = solo_event(&SOLO_12, options, false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert!(has_text(&pages, "Unofficial — 8.5 km per lap."));
}

#[test]
fn test_layout_report_time_board_draws_its_caption() {
    // Arrange
    let (payload, plan) = solo_event(&SOLO_12, all_options(), false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert!(has_text(
        &pages,
        "Most laps, shortest elapsed to the last crossing."
    ));
}

// -------------------------------------------------------------- poster

#[test]
fn test_layout_poster_team_event_draws_three_teams_then_three_solo_riders() {
    // Arrange
    let (payload, _plan) = team_event(all_options(), false);

    // Act
    let pages = poster_pages(&payload, true, None).expect("the poster builds");

    // Assert
    assert!(has_text(&pages, "Teams"));
    assert!(has_text(&pages, "Solo riders"));
    assert!(has_name(&pages, "Team Charlie"));
    assert!(has_name(&pages, "Rider Three"));
    assert!(!has_name(&pages, "Team Delta"));
    assert!(!has_name(&pages, "Rider Four"));
}

#[test]
fn test_layout_poster_solo_event_draws_the_top_five() {
    // Arrange
    let (payload, _plan) = solo_event(&SOLO_12, all_options(), false);

    // Act
    let pages = poster_pages(&payload, true, None).expect("the poster builds");

    // Assert
    assert!(!has_text(&pages, "Teams"));
    assert!(!has_text(&pages, "Solo riders"));
    assert!(has_name(&pages, "Rider Five"));
    assert!(!has_name(&pages, "Rider Six"));
}

#[test]
fn test_layout_poster_team_with_only_dns_teams_skips_the_teams_section() {
    // Arrange
    let dns_team = place_row(
        None,
        result_with_cards("901", "Team Alpha", SnapshotKind::Team, 0, Vec::new()),
    );
    let dns_other = place_row(
        None,
        result_with_cards("902", "Team Bravo", SnapshotKind::Team, 0, Vec::new()),
    );
    let solo = place_row(Some(1), result("801", "Rider One", SnapshotKind::Solo, 5));
    let (payload, _plan) = build(vec![dns_team, dns_other, solo], all_options(), false);

    // Act
    let pages = poster_pages(&payload, true, None).expect("the poster builds");

    // Assert
    assert!(!has_text(&pages, "Teams"));
    assert!(has_text(&pages, "Solo riders"));
    assert!(has_name(&pages, "Rider One"));
}

#[test]
fn test_layout_poster_subtitles_name_the_kind_sex_and_laps() {
    // Arrange
    let (payload, _plan) = team_event(all_options(), false);

    // Act
    let pages = poster_pages(&payload, true, None).expect("the poster builds");

    // Assert
    assert!(has_text(&pages, "Team — Team Alpha · 20 laps"));
    assert!(has_text(&pages, "Solo (F) — Rider One · 10 laps"));
}

#[test]
fn test_layout_poster_hand_prose_stays_title_case() {
    // Arrange
    let (payload, _plan) = solo_event(&SOLO_12, all_options(), false);

    // Act
    let pages = poster_pages(&payload, true, None).expect("the poster builds");

    // Assert
    assert!(has_text(&pages, "Three of a Kind — Nines"));
    assert!(!has_text(&pages, "THREE OF A KIND — NINES"));
}

#[test]
fn test_layout_report_hand_prose_is_uppercased() {
    // Arrange
    let (payload, plan) = solo_event(&SOLO_12, all_options(), false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert!(has_text(&pages, "THREE OF A KIND — NINES"));
}

#[test]
fn test_layout_report_zero_card_entry_draws_a_blank_hand() {
    // Arrange
    let entry = result_with_cards("701", "Rider One", SnapshotKind::Solo, 5, Vec::new());
    let (payload, plan) = build(vec![place_row(Some(1), entry)], all_options(), false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert!(!texts(&pages).iter().any(|text| text.contains("KIND")));
}

#[test]
fn test_layout_poster_zero_card_entry_draws_a_blank_hand() {
    // Arrange
    let entry = result_with_cards("701", "Rider One", SnapshotKind::Solo, 5, Vec::new());
    let (payload, _plan) = build(vec![place_row(Some(1), entry)], all_options(), false);

    // Act
    let pages = poster_pages(&payload, true, None).expect("the poster builds");

    // Assert
    assert!(!texts(&pages).iter().any(|text| text.contains("KIND")));
}

#[test]
fn test_layout_poster_draws_the_whole_hand_when_all_cards_is_on() {
    // Arrange
    let (payload, _plan) = solo_event(&SOLO_12, all_options(), false);

    // Act
    let pages = poster_pages(&payload, true, None).expect("the poster builds");

    // Assert
    assert!(joined(&pages).contains("All 5 cards, in draw order:"));
}

#[test]
fn test_layout_poster_omits_the_whole_hand_when_all_cards_is_off() {
    // Arrange
    let mut options = all_options();
    options.all_cards = false;
    let (payload, _plan) = solo_event(&SOLO_12, options, false);

    // Act
    let pages = poster_pages(&payload, true, None).expect("the poster builds");

    // Assert
    assert!(!joined(&pages).contains("All "));
}

#[test]
fn test_layout_poster_fits_one_page_above_the_footer_gap() {
    // Arrange
    let (payload, _plan) = team_event(all_options(), false);

    // Act
    let pages = poster_pages(&payload, true, None).expect("the poster builds");

    // Assert
    assert_eq!(pages.len(), 1);
    let content_bottom = LETTER_HEIGHT_PT - FOOTER_GAP_IN * POINTS_PER_INCH;
    assert!(max_rect_bottom(&pages) <= content_bottom);
}

#[test]
fn test_layout_poster_footer_has_no_page_count() {
    // Arrange
    let (payload, _plan) = solo_event(&SOLO_12, all_options(), false);

    // Act
    let pages = poster_pages(&payload, true, None).expect("the poster builds");

    // Assert
    assert!(!texts(&pages).iter().any(|text| text.starts_with("Page ")));
    assert!(has_text(&pages, &format!("{GENERATED} · RiverCrossing")));
}

// ------------------------------------------------------------ pagination

#[test]
fn test_layout_report_fifty_row_field_spans_at_least_two_pages() {
    // Arrange
    let names: Vec<String> = (1..=50).map(|number| format!("Rider {number}")).collect();
    let references: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut options = all_options();
    options.all_cards = false;
    options.time_board = false;
    let (payload, plan) = solo_event(&references, options, false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert!(pages.len() >= 2, "pages: {}", pages.len());
}

#[test]
fn test_layout_report_runs_the_title_from_the_second_page_only() {
    // Arrange
    let names: Vec<String> = (1..=50).map(|number| format!("Rider {number}")).collect();
    let references: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut options = all_options();
    options.all_cards = false;
    let (payload, plan) = solo_event(&references, options, false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert_eq!(
        count_text(&pages, "Test Poker Run 2026 — Official results"),
        pages.len() - 1
    );
    assert!(has_text(&pages, "Test Poker Run 2026"));
}

#[test]
fn test_layout_report_every_page_carries_the_page_line_and_credits() {
    // Arrange
    let names: Vec<String> = (1..=50).map(|number| format!("Rider {number}")).collect();
    let references: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut options = all_options();
    options.all_cards = false;
    let (payload, plan) = solo_event(&references, options, false);
    let credits = format!("{} · {}", payload.event.organizer, payload.event.scorer);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");

    // Assert
    assert_page_furniture(&pages, GENERATED, &credits);
}

// ------------------------------------------------------------ page sizes

#[test]
fn test_layout_report_uses_the_letter_page_size() {
    // Arrange
    let (payload, plan) = solo_event(&SOLO_12, all_options(), false);

    // Act
    let pages = report_pages(&payload, &plan, true, None).expect("the report builds");
    let first = pages.first().expect("the report has a page");

    // Assert
    assert!((first.width_pt - 612.0).abs() < 1e-3);
    assert!((first.height_pt - 792.0).abs() < 1e-3);
}

#[test]
fn test_layout_report_uses_the_a4_page_size_when_letter_is_false() {
    // Arrange
    let (payload, plan) = solo_event(&SOLO_12, all_options(), false);

    // Act
    let pages = report_pages(&payload, &plan, false, None).expect("the report builds");
    let first = pages.first().expect("the report has a page");

    // Assert
    assert!((first.width_pt - 595.28).abs() < 1e-3);
    assert!((first.height_pt - 841.89).abs() < 1e-3);
}

#[test]
fn test_layout_poster_uses_the_a4_page_size_when_letter_is_false() {
    // Arrange
    let (payload, _plan) = solo_event(&SOLO_12, all_options(), false);

    // Act
    let pages = poster_pages(&payload, false, None).expect("the poster builds");
    let first = pages.first().expect("the poster has a page");

    // Assert
    assert!((first.width_pt - 595.28).abs() < 1e-3);
    assert!((first.height_pt - 841.89).abs() < 1e-3);
}

// ------------------------------------------------------- private helpers

#[rstest]
#[case("0", "0")]
#[case("262", "262")]
#[case("1234", "1,234")]
#[case("1234567", "1,234,567")]
fn test_layout_group_digits_inserts_commas(#[case] digits: &str, #[case] expected: &str) {
    // Arrange / Act
    let grouped = group_digits(digits);

    // Assert
    assert_eq!(grouped, expected);
}

#[rstest]
#[case("A", "h", "A♥")]
#[case("10", "s", "10♠")]
#[case("Q", "d", "Q♦")]
#[case("JK", "j", "★ JOKER")]
fn test_layout_pair_text_spells_the_face(
    #[case] rank: &str,
    #[case] suit: &str,
    #[case] expected: &str,
) {
    // Arrange
    let face = pair(rank, suit);

    // Act
    let text = pair_text(&face);

    // Assert
    assert_eq!(text, expected);
}

#[rstest]
#[case("s", "♠")]
#[case("h", "♥")]
#[case("d", "♦")]
#[case("c", "♣")]
#[case("j", "")]
#[case("x", "")]
fn test_layout_suit_glyph_maps_known_and_unknown_letters(
    #[case] letter: &str,
    #[case] expected: &str,
) {
    // Arrange / Act
    let glyph = suit_glyph(letter);

    // Assert
    assert_eq!(glyph, expected);
}

#[rstest]
#[case("h", SUIT_RED)]
#[case("d", SUIT_RED)]
#[case("s", INK)]
#[case("c", INK)]
#[case("j", STEEL)]
#[case("x", INK)]
fn test_layout_card_color_colours_by_suit(
    #[case] suit: &str,
    #[case] expected: crate::geometry::Color,
) {
    // Arrange / Act
    let colour = card_color(suit);

    // Assert
    assert_eq!(colour, expected);
}

#[test]
fn test_layout_draw_marker_is_empty_without_a_draw() {
    // Arrange
    let row = sample_row("Pair");

    // Act
    let marker = draw_marker(&row);

    // Assert
    assert_eq!(marker, "");
}

#[test]
fn test_layout_draw_marker_is_empty_for_a_dns_row() {
    // Arrange
    let mut row = sample_row("");
    row.draw = Some(pair("A", "h"));
    row.dns = true;

    // Act
    let marker = draw_marker(&row);

    // Assert
    assert_eq!(marker, "");
}

#[test]
fn test_layout_draw_marker_leads_with_draw_without_hand_prose() {
    // Arrange
    let mut row = sample_row("");
    row.draw = Some(pair("A", "h"));

    // Act
    let marker = draw_marker(&row);

    // Assert
    assert_eq!(marker, "DRAW A♥");
}

#[test]
fn test_layout_draw_marker_joins_the_hand_prose_with_a_separator() {
    // Arrange
    let mut row = sample_row("Pair");
    row.draw = Some(pair("A", "h"));

    // Act
    let marker = draw_marker(&row);

    // Assert
    assert_eq!(marker, " · DRAW A♥");
}

#[test]
fn test_layout_laps_cell_names_a_dns_row() {
    // Arrange
    let mut row = sample_row("Pair");
    row.dns = true;

    // Act
    let cell = laps_cell(&row);

    // Assert
    assert_eq!(cell, "DNS");
}

#[test]
fn test_layout_laps_cell_counts_a_started_row() {
    // Arrange
    let row = sample_row("Pair");

    // Act
    let cell = laps_cell(&row);

    // Assert
    assert_eq!(cell, "5");
}

#[test]
fn test_layout_time_cell_blanks_a_dns_row() {
    // Arrange
    let mut row = sample_row("Pair");
    row.dns = true;

    // Act
    let cell = time_cell(&row, Some("5:00"));

    // Assert
    assert_eq!(cell, "");
}

#[test]
fn test_layout_time_cell_blanks_a_missing_reading() {
    // Arrange
    let row = sample_row("Pair");

    // Act
    let cell = time_cell(&row, None);

    // Assert
    assert_eq!(cell, "");
}

#[test]
fn test_layout_time_cell_keeps_a_reading() {
    // Arrange
    let row = sample_row("Pair");

    // Act
    let cell = time_cell(&row, Some("5:00"));

    // Assert
    assert_eq!(cell, "5:00");
}

#[rstest]
#[case("TEAM", true)]
#[case("TEAM ×4", true)]
#[case("SOLO", false)]
fn test_layout_is_team_row_matches_the_upper_prefix(
    #[case] entry_type: &str,
    #[case] expected: bool,
) {
    // Arrange
    let mut row = sample_row("Pair");
    row.entry_type = entry_type.to_owned();

    // Act
    let team = is_team_row(&row);

    // Assert
    assert_eq!(team, expected);
}

#[rstest]
#[case(
    true,
    true,
    " Lap counts and times below are unofficial and shown for bragging rights only."
)]
#[case(
    true,
    false,
    " Lap counts below are unofficial and shown for bragging rights only."
)]
#[case(
    false,
    true,
    " Times below are unofficial and shown for bragging rights only."
)]
#[case(false, false, "")]
fn test_layout_bragging_clause_names_the_shown_boards(
    #[case] laps_board: bool,
    #[case] show_times: bool,
    #[case] expected: &str,
) {
    // Arrange
    let options = ExportOptions {
        laps_board,
        show_times,
        ..all_options()
    };

    // Act
    let clause = bragging_clause(&options);

    // Assert
    assert_eq!(clause, expected);
}

#[test]
fn test_layout_format_km_drops_a_whole_fraction() {
    // Arrange / Act
    let text = format_km(8.0);

    // Assert
    assert_eq!(text, "8");
}

#[test]
fn test_layout_format_km_keeps_a_fraction() {
    // Arrange / Act
    let text = format_km(8.5);

    // Assert
    assert_eq!(text, "8.5");
}

#[test]
fn test_layout_wrap_keeps_short_text_on_one_line() {
    // Arrange / Act
    let lines = wrap(FontFace::Body, 10.0, "one two", 1000.0);

    // Assert
    assert_eq!(lines, vec!["one two".to_owned()]);
}

#[test]
fn test_layout_wrap_breaks_at_a_space() {
    // Arrange / Act
    let lines = wrap(FontFace::Body, 10.0, "one two", 1.0);

    // Assert
    assert_eq!(lines, vec!["one".to_owned(), "two".to_owned()]);
}

#[test]
fn test_layout_wrap_keeps_a_long_word_on_its_own_line() {
    // Arrange / Act
    let lines = wrap(FontFace::Body, 10.0, "supercalifragilistic", 0.0);

    // Assert
    assert_eq!(lines, vec!["supercalifragilistic".to_owned()]);
}

#[test]
fn test_layout_wrap_of_empty_text_yields_one_empty_line() {
    // Arrange / Act
    let lines = wrap(FontFace::Body, 10.0, "", 100.0);

    // Assert
    assert_eq!(lines, vec![String::new()]);
}

#[test]
fn test_layout_boxed_cards_drops_a_face_that_would_cross_the_edge() {
    // Arrange
    let mut canvas = blank_canvas();
    let before = canvas.current.ops.len();

    // Act
    let after = canvas.boxed_cards(
        &[pair("9", "s")],
        Span { x: 0.58, w: 0.0 },
        CardBox {
            size_pt: 7.0,
            height_in: 0.24,
        },
    );

    // Assert
    assert!((after - 0.58).abs() < 1e-6);
    assert_eq!(canvas.current.ops.len(), before);
}

#[test]
fn test_layout_boxed_cards_draws_a_face_that_fits() {
    // Arrange
    let mut canvas = blank_canvas();
    let start = 0.58;

    // Act
    let after = canvas.boxed_cards(
        &[pair("9", "s")],
        Span { x: start, w: 2.0 },
        CardBox {
            size_pt: 7.0,
            height_in: 0.24,
        },
    );

    // Assert
    assert!(after > start);
    let filled = canvas
        .current
        .ops
        .iter()
        .filter(|op| matches!(op, Op::FilledRect { .. }))
        .count();
    assert_eq!(filled, 1);
}
