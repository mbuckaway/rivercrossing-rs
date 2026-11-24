// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the results page renderer.

#![expect(
    clippy::too_many_arguments,
    reason = "the fixture builder names each field of an entry separately"
)]

use std::collections::BTreeMap;

use pretty_assertions::assert_eq;
use rivercrossing_cards::Card;
use rivercrossing_hands::best_hand;
use rivercrossing_ride::EntryId;
use rivercrossing_standings::{EntryResult, Placed, SnapshotKind};
use rstest::rstest;
use serde_json::Value;
use time::{Date, Month};

use super::{render, render_page};
use crate::record::from_record;
use crate::types::{
    CardPair, ExportInput, ExportOptions, RacePayload, RideInfo, SELF_TEST_NOTE, TRANSPARENT_PNG,
};

const PAYLOAD_TIMES: &str = include_str!("../../fixtures/payload-times.json");
const PAYLOAD_NO_TIMES: &str = include_str!("../../fixtures/payload-no-times.json");
const PAYLOAD_SOLO: &str = include_str!("../../fixtures/payload-solo.json");
const GOLDEN_TIMES: &str = include_str!("../../fixtures/epic-2026-results.html");
const GOLDEN_NO_TIMES: &str = include_str!("../../fixtures/epic-2026-results-no-times.html");
const GOLDEN_SOLO: &str = include_str!("../../fixtures/epic-2026-results-solo.html");

const GENERATED: &str = "Generated 16:07, Sept 20 2026";

const FAKE_LOGO: &str = "data:image/png;base64,FAKE";

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

fn pair(rank: &str, suit: &str) -> CardPair {
    CardPair {
        rank: rank.to_owned(),
        suit: suit.to_owned(),
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

fn options(show_times: bool, laps_board: bool, time_board: bool) -> ExportOptions {
    ExportOptions {
        show_times,
        laps_board,
        time_board,
        ..ExportOptions::default()
    }
}

/// Read a fixture record into a payload carrying the rider census.
fn payload_from(record_text: &str, riders: usize) -> RacePayload {
    let record: Value = serde_json::from_str(record_text).expect("the fixture record parses");
    let mut payload = from_record(&record).expect("the fixture record reads");
    payload.event.riders = riders;
    payload
}

/// Return the times fixture's payload with its production rider census.
fn times_payload() -> RacePayload {
    payload_from(PAYLOAD_TIMES, 262)
}

/// Return the text of *page*'s `race-data` record.
fn embedded_record(page: &str) -> String {
    let (_, rest) = page
        .split_once("id=\"race-data\">")
        .expect("the page carries the record element");
    let (record, _) = rest
        .split_once("</script>")
        .expect("the record element closes");
    record.to_owned()
}

/// Read *page*'s `race-data` record back into a payload.
fn record_payload(page: &str) -> RacePayload {
    let text = embedded_record(page).replace("<\\/", "</");
    let value: Value = serde_json::from_str(&text).expect("the record is JSON");
    from_record(&value).expect("the record reads")
}

/// Return the first line on which *expected* and *actual* differ.
fn first_difference(expected: &str, actual: &str) -> Option<String> {
    let expected_lines: Vec<&str> = expected.lines().collect();
    let actual_lines: Vec<&str> = actual.lines().collect();
    for (index, (golden, rendered)) in expected_lines.iter().zip(actual_lines.iter()).enumerate() {
        if golden != rendered {
            return Some(format!(
                "line {}:\n  golden: {golden:?}\n  actual: {rendered:?}",
                index + 1
            ));
        }
    }
    if expected_lines.len() != actual_lines.len() {
        return Some(format!(
            "line count {} vs {} (actual tail: {:?})",
            expected_lines.len(),
            actual_lines.len(),
            actual_lines.get(expected_lines.len())
        ));
    }
    None
}

// ------------------------------------------------------------ the goldens

#[rstest]
#[case(PAYLOAD_TIMES, GOLDEN_TIMES, 262)]
#[case(PAYLOAD_NO_TIMES, GOLDEN_NO_TIMES, 262)]
#[case(PAYLOAD_SOLO, GOLDEN_SOLO, 180)]
fn test_render_page_given_a_record_reads_the_production_golden(
    #[case] record_text: &str,
    #[case] golden: &str,
    #[case] riders: usize,
) {
    // Arrange
    let payload = payload_from(record_text, riders);

    // Act
    let page = render_page(&payload, None, Some(TRANSPARENT_PNG)).expect("the page renders");

    // Assert
    assert_eq!(
        page,
        golden,
        "{}",
        first_difference(golden, &page).unwrap_or_default()
    );
}

#[test]
fn test_render_page_given_the_times_record_stays_self_contained() {
    // Arrange
    let payload = times_payload();

    // Act
    let page = render_page(&payload, None, Some(TRANSPARENT_PNG)).expect("the page renders");

    // Assert
    assert_eq!(
        (
            page.contains("http://"),
            page.contains("<link"),
            page.contains("<script src="),
            page.contains("@tailwindcss/browser"),
            page.matches("<script").count(),
            page.matches("<style").count(),
            page.contains("id=\"race-data\""),
        ),
        (false, false, false, false, 1, 1, true)
    );
}

#[test]
fn test_render_given_a_closing_tag_in_a_name_escapes_the_record() {
    // Arrange
    let mut rows = pair_field();
    rows[0].result.name = "</script>".to_owned();
    let input = input_with(rows, ExportOptions::default());

    // Act
    let page = render(&input).expect("the page renders");

    // Assert
    assert_eq!(
        (
            page.matches("</script>").count(),
            page.contains("<\\/script>")
        ),
        (1, true)
    );
}

#[test]
fn test_render_given_a_closing_tag_in_a_name_round_trips_the_record() {
    // Arrange
    let mut rows = pair_field();
    rows[0].result.name = "</script>".to_owned();
    let input = input_with(rows, ExportOptions::default());

    // Act
    let page = render(&input).expect("the page renders");
    let record = record_payload(&page);

    // Assert
    assert_eq!(
        record.results.first().map(|row| row.entry.as_str()),
        Some("</script>")
    );
}

// --------------------------------------------------------- the page text

#[test]
fn test_render_given_a_tiebreak_card_renders_the_draw_badge_and_record_pair() {
    // Arrange
    let mut result = sample("88", "Moss Ridge Riders", SnapshotKind::Solo, 11);
    result.tiebreak_card = Some(card("AD"));
    let mut row = placed(Some(1), result);
    row.draw_required = true;
    let input = input_with(vec![row], ExportOptions::default());

    // Act
    let page = render(&input).expect("the page renders");
    let record = record_payload(&page);

    // Assert
    assert_eq!(
        (
            page.contains("tie-break"),
            page.contains("draw <span class=\"chip r\">A"),
            record.results.first().and_then(|row| row.draw.clone()),
        ),
        (true, true, Some(pair("A", "d")))
    );
}

#[test]
fn test_render_given_a_pair_field_renders_the_page() {
    // Arrange
    let input = input_with(pair_field(), ExportOptions::default());

    // Act
    let page = render(&input).expect("the page renders");

    // Assert
    assert_eq!(
        (
            page.contains(
                "<title>Test Poker Run 2026 — Official Poker Run Results (no times)</title>"
            ),
            page.contains("Saturday June 6, 2026 · Test Venue · 8 km loop"),
            page.contains("Organizer: Test Org"),
            page.contains("Scorer: T. Ester"),
            page.contains("2 · 21 · 10 · 0"),
            page.contains("#88"),
            page.contains("Unofficial — 8 km per lap."),
            page.contains("Unofficial — 8.0 km per lap."),
            page.contains("\"riders\""),
        ),
        (true, true, true, true, true, true, true, false, false)
    );
}

#[test]
fn test_render_given_show_times_off_omits_the_time_cells_and_record_keys() {
    // Arrange
    let input = input_with(pair_field(), options(false, true, true));

    // Act
    let page = render(&input).expect("the page renders");
    let record = embedded_record(&page);

    // Assert
    assert_eq!(
        (
            page.contains("pr-3 t-col"),
            record.contains("\"total\""),
            record.contains("\"bestLap\""),
            record.contains("\"avg\""),
            record.contains("\"timeBoard\": []"),
            page.contains("Official Poker Run Results (no times)"),
        ),
        (false, false, false, false, true, true)
    );
}

// -------------------------------------------------------------- the logo

#[test]
fn test_render_page_given_no_logo_uses_the_transparent_png() {
    // Arrange
    let payload = times_payload();

    // Act
    let page = render_page(&payload, None, None).expect("the page renders");

    // Assert
    assert_eq!(page.matches(TRANSPARENT_PNG).count(), 1);
}

#[test]
fn test_render_page_given_a_logo_uses_it() {
    // Arrange
    let payload = times_payload();

    // Act
    let page = render_page(&payload, None, Some(FAKE_LOGO)).expect("the page renders");

    // Assert
    assert_eq!(
        (page.contains(FAKE_LOGO), page.contains(TRANSPARENT_PNG)),
        (true, false)
    );
}

#[test]
fn test_render_given_a_logo_in_the_input_uses_it() {
    // Arrange
    let mut input = input_with(pair_field(), ExportOptions::default());
    input.logo_src = Some(FAKE_LOGO.to_owned());

    // Act
    let page = render(&input).expect("the page renders");

    // Assert
    assert_eq!(
        (page.contains(FAKE_LOGO), page.contains(TRANSPARENT_PNG)),
        (true, false)
    );
}

// --------------------------------------------------------- the self test

#[test]
fn test_render_given_an_unverified_ride_renders_the_self_test_note() {
    // Arrange
    let mut input = input_with(pair_field(), ExportOptions::default());
    input.self_test_unverified = true;

    // Act
    let page = render(&input).expect("the page renders");
    let record = embedded_record(&page);

    // Assert
    assert_eq!(
        (
            page.contains(SELF_TEST_NOTE),
            record.contains("\"selfTestNote\""),
        ),
        (true, true)
    );
}

#[test]
fn test_render_given_a_verified_ride_omits_the_self_test_note() {
    // Arrange
    let input = input_with(pair_field(), ExportOptions::default());

    // Act
    let page = render(&input).expect("the page renders");
    let record = embedded_record(&page);

    // Assert
    assert_eq!(
        (
            page.contains(SELF_TEST_NOTE),
            record.contains("selfTestNote"),
        ),
        (false, false)
    );
}
