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

use super::{
    render, render_page, render_poster, render_poster_wordpress, render_template, render_wordpress,
};
use crate::error::ExportError;
use crate::format::png_data_uri;
use crate::record::from_record;
use crate::types::{
    CardPair, ExportInput, ExportOptions, RacePayload, RideInfo, SELF_TEST_NOTE, TRANSPARENT_PNG,
};
use minijinja::Value as JinjaValue;

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

// ------------------------------------------------------- the poster model

/// Split a fixture's space-separated card codes.
fn card_codes(text: &str) -> Vec<&str> {
    text.split_whitespace().collect()
}

/// Build one finished-ride entry from card codes, with no sex marker.
fn coded_entry(
    plate: &str,
    name: &str,
    kind: SnapshotKind,
    laps: u32,
    codes: &[&str],
) -> EntryResult {
    let cards: Vec<Card> = codes.iter().map(|code| card(code)).collect();
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
        sex: None,
        tiebreak_card: None,
    }
}

/// Build one DID-NOT-START row: unplaced, zero laps.
fn dns_row(plate: &str, name: &str, kind: SnapshotKind) -> Placed {
    placed(
        None,
        coded_entry(plate, name, kind, 0, &["9S", "9D", "9C", "KH", "2S"]),
    )
}

/// Build a per-kind field: teams take places 1..N, then solo restarts at 1.
fn kind_field(teams: u32, solo: u32) -> Vec<Placed> {
    let team_rows = (0..teams).map(|index| {
        let number = index + 1;
        placed(
            Some(number),
            coded_entry(
                &(500 + index).to_string(),
                &format!("Team {number}"),
                SnapshotKind::Team,
                30 - index,
                &["9S", "9D", "9C", "KH", "2S"],
            ),
        )
    });
    let solo_rows = (0..solo).map(|index| {
        let number = index + 1;
        placed(
            Some(number),
            coded_entry(
                &(10 + index).to_string(),
                &format!("Solo {number}"),
                SnapshotKind::Solo,
                20 - index,
                &["9S", "9D", "9C", "KH", "2S"],
            ),
        )
    });
    team_rows.chain(solo_rows).collect()
}

/// The poster fixture's four teams, transcribed from the oracle.
const POSTER_TEAMS: [(&str, &str, &str); 4] = [
    ("88", "Moss Ridge Riders", "9S 9D 9C KH 2S"),
    ("77", "Dirt Dynamos", "AS KS QS JS 10S"),
    ("66", "River Rats", "5S 5D 5C 5H 3S"),
    ("55", "Fourth Team", "7S 7D 7C 6H 2S"),
];

/// The poster fixture's four solo riders, transcribed from the oracle.
const POSTER_SOLO: [(&str, &str, &str); 4] = [
    ("7", "Luca Ferrari", "2S 2D 2C 3H 4S"),
    ("8", "Ana Silva", "AS AD AC KH 2S"),
    ("9", "Bo Jones", "QS QD QC JH 2S"),
    ("10", "Cy Marsh", "JS JD JC 10H 2S"),
];

/// The six hands the poster prints, in section order.
const POSTER_PODIUM_HANDS: [&str; 6] = [
    "Three of a Kind — Nines",
    "Royal Flush",
    "Four of a Kind — Fives",
    "Three of a Kind — Twos",
    "Three of a Kind — Aces",
    "Three of a Kind — Queens",
];

/// The leading team's lap count; one lap back per place.
const POSTER_TOP_TEAM_LAPS: u32 = 30;

/// The leading solo rider's lap count; one lap back per place.
const POSTER_TOP_SOLO_LAPS: u32 = 20;

/// Build the oracle's poster field: four teams, then four solo riders.
fn poster_field() -> Vec<Placed> {
    let teams = POSTER_TEAMS
        .iter()
        .enumerate()
        .map(|(index, (plate, name, codes))| {
            let offset = u32::try_from(index).expect("the fixture index fits");
            placed(
                Some(offset + 1),
                coded_entry(
                    plate,
                    name,
                    SnapshotKind::Team,
                    POSTER_TOP_TEAM_LAPS - offset,
                    &card_codes(codes),
                ),
            )
        });
    let solo = POSTER_SOLO
        .iter()
        .enumerate()
        .map(|(index, (plate, name, codes))| {
            let offset = u32::try_from(index).expect("the fixture index fits");
            placed(
                Some(offset + 1),
                coded_entry(
                    plate,
                    name,
                    SnapshotKind::Solo,
                    POSTER_TOP_SOLO_LAPS - offset,
                    &card_codes(codes),
                ),
            )
        });
    teams.chain(solo).collect()
}

/// Render *placed* as the poster under the fixture's pinned stamp.
fn poster_page(placed: Vec<Placed>) -> String {
    let input = input_with(placed, ExportOptions::default());
    render_poster(&input).expect("the poster renders")
}

/// Render *placed* as the `WordPress` results fragment.
fn wordpress_page(placed: Vec<Placed>, options: ExportOptions) -> String {
    let input = input_with(placed, options);
    render_wordpress(&input).expect("the fragment renders")
}

/// Render *placed* as the `WordPress` poster fragment.
fn wordpress_poster_page(placed: Vec<Placed>) -> String {
    let input = input_with(placed, ExportOptions::default());
    render_poster_wordpress(&input).expect("the fragment renders")
}

/// Return the macros' markup: the event header through the footer.
fn macro_output(markup: &str) -> &str {
    let start = markup
        .find("<header class=\"bp")
        .expect("the render carries the event header");
    let end = markup
        .find("</footer>")
        .expect("the render carries the footer")
        + "</footer>".len();
    &markup[start..end]
}

/// Return whether *markup* carries *tag*, case-insensitively at a boundary.
fn has_tag(markup: &str, tag: &str) -> bool {
    let lower = markup.to_lowercase();
    ["<", "</"].iter().any(|prefix| {
        let needle = format!("{prefix}{tag}");
        lower.match_indices(&needle).any(|(index, _)| {
            lower
                .get(index + needle.len()..)
                .and_then(|rest| rest.chars().next())
                .is_none_or(|next| !next.is_ascii_alphanumeric())
        })
    })
}

/// Return the text of each `<hN>` heading in *markup*.
fn heading_texts(markup: &str, level: u8) -> Vec<String> {
    let open = format!("<h{level}");
    let close = format!("</h{level}>");
    let mut texts = Vec::new();
    let mut rest = markup;
    while let Some((_, after_open)) = rest.split_once(&open) {
        let Some((_, after_tag)) = after_open.split_once('>') else {
            break;
        };
        let Some((text, after_close)) = after_tag.split_once(&close) else {
            break;
        };
        texts.push(text.to_owned());
        rest = after_close;
    }
    texts
}

// ---------------------------------------------------------- the poster

#[test]
fn test_render_poster_given_a_ride_titles_the_page_and_its_header() {
    // Arrange
    let page = poster_page(poster_field());

    // Assert
    assert_eq!(
        (
            page.contains("<title>Test Poker Run 2026 — Podium poster</title>"),
            page.contains("Organizer: Test Org"),
            page.contains("Saturday June 6, 2026 · Test Venue · 8 km loop"),
            page.contains(">Best poker hands</h1>"),
            page.contains(">Test Poker Run 2026</h2>"),
            page.contains("Scorer: T. Ester"),
            page.contains(GENERATED),
        ),
        (true, true, true, true, true, true, true)
    );
}

#[test]
fn test_render_poster_given_a_team_event_names_each_kinds_top_three() {
    // Arrange
    let page = poster_page(poster_field());

    // Assert
    assert_eq!(
        (
            page.contains("Moss Ridge Riders"),
            page.contains("Dirt Dynamos"),
            page.contains("River Rats"),
            page.contains("Luca Ferrari"),
            page.contains("Ana Silva"),
            page.contains("Bo Jones"),
            page.contains("Fourth Team"),
            page.contains("Cy Marsh"),
        ),
        (true, true, true, true, true, true, false, false)
    );
}

#[test]
fn test_render_poster_given_a_team_event_titles_each_section() {
    // Arrange
    let page = poster_page(poster_field());
    let teams = page.find(">Teams</h3>");
    let solo = page.find(">Solo riders</h3>");

    // Assert
    assert!(matches!((teams, solo), (Some(teams), Some(solo)) if teams < solo));
}

#[test]
fn test_render_poster_given_a_team_event_omits_each_kinds_fourth_place() {
    // Arrange
    let page = poster_page(poster_field());

    // Assert
    assert_eq!(
        (
            page.contains("Fourth Team"),
            page.contains("Cy Marsh"),
            page.contains("Three of a Kind — Sevens"),
            page.contains("Three of a Kind — Jacks"),
        ),
        (false, false, false, false)
    );
}

#[test]
fn test_render_poster_given_a_team_card_omits_the_plate() {
    // Arrange
    let page = poster_page(poster_field());

    // Assert
    assert_eq!(
        (
            page.contains(">#88"),
            page.contains(">#77"),
            page.contains(">#66"),
            page.contains(">Moss Ridge Riders</div>"),
        ),
        (false, false, false, true)
    );
}

#[test]
fn test_render_poster_given_a_solo_card_keeps_the_plate_prefix() {
    // Arrange
    let page = poster_page(poster_field());

    // Assert
    assert!(page.contains(">#7 Luca Ferrari</div>"));
}

#[test]
fn test_render_poster_given_a_team_card_names_entry_kind_and_laps() {
    // Arrange
    let page = poster_page(poster_field());

    // Assert
    assert!(page.contains(">Team — Moss Ridge Riders · 30 laps</div>"));
}

#[test]
fn test_render_poster_given_a_solo_card_names_entry_kind_and_laps() {
    // Arrange
    let page = poster_page(poster_field());

    // Assert
    assert!(page.contains(">Solo — Luca Ferrari · 20 laps</div>"));
}

#[rstest]
#[case(POSTER_PODIUM_HANDS[0])]
#[case(POSTER_PODIUM_HANDS[1])]
#[case(POSTER_PODIUM_HANDS[2])]
#[case(POSTER_PODIUM_HANDS[3])]
#[case(POSTER_PODIUM_HANDS[4])]
#[case(POSTER_PODIUM_HANDS[5])]
fn test_render_poster_given_a_podium_hand_prints_the_title_case_prose(#[case] hand: &str) {
    // Arrange
    let page = poster_page(poster_field());

    // Assert
    assert_eq!(
        (
            page.contains(&format!(">{hand}</div>")),
            page.contains(&hand.to_uppercase())
        ),
        (true, false)
    );
}

#[test]
fn test_render_poster_given_show_times_off_omits_every_total_time() {
    // Arrange
    let page = poster_page(poster_field());

    // Assert
    assert_eq!(
        (page.contains("15:02:00"), page.contains("10:02:00")),
        (false, false)
    );
}

#[test]
fn test_render_poster_given_show_times_on_prints_the_podium_totals() {
    // Arrange
    let input = input_with(
        poster_field(),
        ExportOptions {
            show_times: true,
            ..ExportOptions::default()
        },
    );

    // Act
    let page = render_poster(&input).expect("the poster renders");

    // Assert
    assert_eq!(
        (
            page.contains(" laps · 15:02:00</div>"),
            page.contains(" laps · 10:02:00</div>"),
        ),
        (true, true)
    );
}

#[test]
fn test_render_poster_given_a_solo_only_event_lists_the_top_five() {
    // Arrange
    let page = poster_page(kind_field(0, 7));

    // Assert
    assert_eq!(
        (page.contains("Solo 5"), page.contains("Solo 6")),
        (true, false)
    );
}

#[test]
fn test_render_poster_given_a_solo_only_event_prints_no_section_heading() {
    // Arrange
    let page = poster_page(kind_field(0, 3));

    // Assert
    assert_eq!(
        (
            page.contains(">Teams</h3>"),
            page.contains(">Solo riders</h3>")
        ),
        (false, false)
    );
}

#[test]
fn test_render_poster_given_an_empty_field_renders_the_cover_alone() {
    // Arrange
    let page = poster_page(Vec::new());

    // Assert
    assert_eq!(
        (
            page.starts_with("<!DOCTYPE html>"),
            page.contains(">Best poker hands</h1>"),
            page.contains(">Teams</h3>"),
            page.matches("<style").count(),
        ),
        (true, true, false, 1)
    );
}

#[test]
fn test_render_poster_given_all_cards_on_prints_the_whole_hand_line() {
    // Arrange
    let page = poster_page(poster_field());

    // Assert
    assert!(page.contains("All 5 cards, in draw order: <span"));
}

#[test]
fn test_render_poster_given_all_cards_off_omits_the_whole_hand_line() {
    // Arrange
    let input = input_with(
        poster_field(),
        ExportOptions {
            all_cards: false,
            ..ExportOptions::default()
        },
    );

    // Act
    let page = render_poster(&input).expect("the poster renders");

    // Assert
    assert!(!page.contains("cards, in draw order:"));
}

#[test]
fn test_render_poster_given_no_logo_embeds_the_transparent_fallback() {
    // Arrange
    let page = poster_page(poster_field());

    // Assert
    assert_eq!(
        (
            page.matches(TRANSPARENT_PNG).count(),
            page.contains("src=\"\"")
        ),
        (1, false)
    );
}

#[test]
fn test_render_poster_given_a_logo_uses_it() {
    // Arrange
    let mut input = input_with(poster_field(), ExportOptions::default());
    input.logo_src = Some(FAKE_LOGO.to_owned());

    // Act
    let page = render_poster(&input).expect("the poster renders");

    // Assert
    assert_eq!(
        (page.contains(FAKE_LOGO), page.contains(TRANSPARENT_PNG)),
        (true, false)
    );
}

#[test]
fn test_render_poster_given_an_unverified_ride_renders_the_self_test_note() {
    // Arrange
    let mut input = input_with(poster_field(), ExportOptions::default());
    input.self_test_unverified = true;

    // Act
    let page = render_poster(&input).expect("the poster renders");

    // Assert
    assert_eq!(
        page.contains(&format!(
            "<p class=\"mt-2 text-xs font-semibold text-steel-700\">{SELF_TEST_NOTE}</p>"
        )),
        true
    );
}

#[test]
fn test_render_poster_given_a_verified_ride_renders_no_self_test_note() {
    // Arrange
    let page = poster_page(poster_field());

    // Assert
    assert!(!page.contains(SELF_TEST_NOTE));
}

#[test]
fn test_render_poster_page_is_self_contained_with_no_fetchable_reference() {
    // Arrange
    let page = poster_page(poster_field());

    // Assert
    assert_eq!(
        (
            page.contains("http://"),
            page.contains("url(http"),
            page.contains("<script"),
            page.contains("<link"),
            page.matches("<style").count(),
            page.contains("generated by tools/gen_css.py"),
        ),
        (false, false, false, false, 1, true)
    );
}

#[test]
fn test_render_poster_given_a_dns_row_omits_it_from_the_solo_five() {
    // Arrange
    let mut placed = vec![placed(
        Some(1),
        coded_entry("88", "Moss Ridge Riders", SnapshotKind::Solo, 11, &["9S"]),
    )];
    placed.push(dns_row("31", "Rita Slow", SnapshotKind::Solo));

    // Act
    let page = poster_page(placed);

    // Assert
    assert_eq!(
        (
            page.contains("Moss Ridge Riders"),
            page.contains("Rita Slow")
        ),
        (true, false)
    );
}

#[test]
fn test_render_poster_given_a_dns_team_row_omits_it_from_the_team_three() {
    // Arrange
    let mut placed = vec![placed(
        Some(1),
        coded_entry("88", "Moss Ridge Riders", SnapshotKind::Team, 11, &["9S"]),
    )];
    placed.push(dns_row("77", "Late Starters", SnapshotKind::Team));

    // Act
    let page = poster_page(placed);

    // Assert
    assert_eq!(
        (
            page.contains("Moss Ridge Riders"),
            page.contains("Late Starters"),
            page.contains(">#77"),
        ),
        (true, false, false)
    );
}

#[test]
fn test_render_poster_given_a_drawn_card_renders_the_draw_badge() {
    // Arrange
    let mut result = coded_entry("88", "Moss Ridge Riders", SnapshotKind::Team, 11, &["9S"]);
    result.tiebreak_card = Some(card("AH"));
    let page = poster_page(vec![placed(Some(1), result)]);

    // Assert
    assert!(page.contains("draw <span class=\"chip r\">A ♥\u{fe0e}</span>"));
}

#[test]
fn test_render_poster_given_a_non_numeric_plate_returns_the_plate_error() {
    // Arrange
    let input = input_with(
        vec![placed(
            Some(1),
            coded_entry("ABC", "Bad Plate", SnapshotKind::Solo, 3, &["9S"]),
        )],
        ExportOptions::default(),
    );

    // Act
    let result = render_poster(&input);

    // Assert
    assert!(matches!(
        result,
        Err(ExportError::PlateNotNumeric { plate }) if plate == "ABC"
    ));
}

// ------------------------------------------- the WordPress results fragment

/// The fixed first sentence of the fragment's bragging-rights disclaimer.
const DISCLAIMER: &str = "It's not a race, it's a poker run — placings are by best poker hand.";

#[rstest]
#[case("!doctype")]
#[case("html")]
#[case("head")]
#[case("body")]
fn test_render_wordpress_given_each_document_tag_omits_the_scaffolding(#[case] tag: &str) {
    // Arrange
    let fragment = wordpress_page(pair_field(), options(true, true, false));

    // Assert
    assert!(!has_tag(&fragment, tag));
}

#[rstest]
#[case("<script")]
#[case("id=\"race-data\"")]
fn test_render_wordpress_given_each_record_token_omits_it(#[case] token: &str) {
    // Arrange
    let fragment = wordpress_page(pair_field(), options(true, true, false));

    // Assert
    assert!(!fragment.contains(token));
}

#[test]
fn test_render_wordpress_wraps_the_content_in_the_scope_wrapper() {
    // Arrange
    let fragment = wordpress_page(pair_field(), options(true, true, false));

    // Assert
    assert_eq!(
        (
            fragment.starts_with("<style>"),
            fragment.matches("<div class=\"rc-results\"").count(),
            fragment.trim_end().ends_with("</div>"),
        ),
        (true, 1, true)
    );
}

#[test]
fn test_render_wordpress_inlines_the_scoped_stylesheet_not_the_unscoped_one() {
    // Arrange
    let fragment = wordpress_page(pair_field(), options(true, true, false));

    // Assert
    assert_eq!(
        (
            fragment.contains("/* rivercrossing compiled_css_wp"),
            fragment.contains("/* rivercrossing compiled_css —"),
            fragment.contains(".rc-results .mt-1{"),
        ),
        (true, false, true)
    );
}

#[test]
fn test_render_wordpress_macro_markup_matches_the_standalone_page() {
    // Arrange
    let input = input_with(pair_field(), options(true, true, false));

    // Act
    let page = render(&input).expect("the page renders");
    let fragment = render_wordpress(&input).expect("the fragment renders");

    // Assert
    assert_eq!(macro_output(&fragment), macro_output(&page));
}

#[test]
fn test_render_wordpress_macro_markup_matches_the_page_for_a_team_field() {
    // Arrange
    let input = input_with(kind_field(2, 2), options(true, true, false));

    // Act
    let page = render(&input).expect("the page renders");
    let fragment = render_wordpress(&input).expect("the fragment renders");

    // Assert
    assert_eq!(macro_output(&fragment), macro_output(&page));
}

#[rstest]
#[case("<section")]
#[case("<table")]
#[case("<tr")]
#[case("<th")]
#[case("<td")]
#[case("class=\"chip")]
fn test_render_wordpress_element_counts_match_the_standalone_page(#[case] token: &str) {
    // Arrange
    let input = input_with(kind_field(2, 2), options(true, true, false));

    // Act
    let page = render(&input).expect("the page renders");
    let fragment = render_wordpress(&input).expect("the fragment renders");

    // Assert
    assert_eq!(fragment.matches(token).count(), page.matches(token).count());
}

#[test]
fn test_render_wordpress_headings_match_the_standalone_page() {
    // Arrange
    let input = input_with(pair_field(), options(true, true, false));

    // Act
    let page = render(&input).expect("the page renders");
    let fragment = render_wordpress(&input).expect("the fragment renders");

    // Assert
    assert_eq!(
        (
            heading_texts(&fragment, 2),
            heading_texts(&fragment, 2) == heading_texts(&page, 2),
        ),
        (
            vec![
                "Best hands — top 3".to_owned(),
                "Top ten".to_owned(),
                "Most laps".to_owned(),
                "Full field".to_owned(),
            ],
            true,
        )
    );
}

#[test]
fn test_render_wordpress_given_a_rider_count_renders_it_in_the_header() {
    // Arrange
    let mut input = input_with(pair_field(), options(true, true, false));
    input.riders = 207;

    // Act
    let fragment = render_wordpress(&input).expect("the fragment renders");

    // Assert
    assert!(fragment.contains("2 · 21 · 10 · 207"));
}

#[test]
fn test_render_wordpress_omitted_rider_count_renders_zero_in_the_header() {
    // Arrange
    let fragment = wordpress_page(pair_field(), options(true, true, false));

    // Assert
    assert!(fragment.contains("2 · 21 · 10 · 0"));
}

#[test]
fn test_render_wordpress_given_an_unverified_ride_carries_the_self_test_note() {
    // Arrange
    let mut input = input_with(pair_field(), options(true, true, false));
    input.self_test_unverified = true;

    // Act
    let fragment = render_wordpress(&input).expect("the fragment renders");

    // Assert
    assert!(fragment.contains(SELF_TEST_NOTE));
}

#[test]
fn test_render_wordpress_given_a_verified_ride_carries_no_self_test_note() {
    // Arrange
    let fragment = wordpress_page(pair_field(), options(true, true, false));

    // Assert
    assert!(!fragment.contains(SELF_TEST_NOTE));
}

#[test]
fn test_render_wordpress_given_no_logo_embeds_the_transparent_png() {
    // Arrange
    let fragment = wordpress_page(pair_field(), options(true, true, false));

    // Assert
    assert_eq!(
        (
            fragment.contains(TRANSPARENT_PNG),
            fragment.contains("src=\"\""),
        ),
        (true, false)
    );
}

#[test]
fn test_render_wordpress_given_a_logo_embeds_its_data_uri() {
    // Arrange
    let logo = png_data_uri(b"\x89PNG\r\n\x1a\nlogo-bytes");
    let mut input = input_with(pair_field(), options(true, true, false));
    input.logo_src = Some(logo.clone());

    // Act
    let fragment = render_wordpress(&input).expect("the fragment renders");

    // Assert
    assert!(fragment.contains(&logo));
}

#[rstest]
#[case(
    true,
    true,
    " Lap counts and times below are unofficial and shown for bragging rights only."
)]
#[case(
    false,
    true,
    " Lap counts below are unofficial and shown for bragging rights only."
)]
#[case(
    true,
    false,
    " Times below are unofficial and shown for bragging rights only."
)]
#[case(false, false, "")]
fn test_render_wordpress_disclaimer_names_only_the_boards_the_options_show(
    #[case] show_times: bool,
    #[case] laps_board: bool,
    #[case] clause: &str,
) {
    // Arrange
    let fragment = wordpress_page(pair_field(), options(show_times, laps_board, false));

    // Assert
    assert!(fragment.contains(&format!("{DISCLAIMER}{clause}</p>")));
}

#[test]
fn test_render_wordpress_disclaimer_given_no_board_shown_omits_the_clause() {
    // Arrange
    let fragment = wordpress_page(pair_field(), options(false, false, false));

    // Assert
    assert_eq!(
        (
            fragment.contains("bragging rights"),
            fragment.contains("unofficial and shown for bragging rights"),
        ),
        (false, false)
    );
}

// -------------------------------------------- the WordPress poster fragment

#[rstest]
#[case("!doctype")]
#[case("html")]
#[case("head")]
#[case("body")]
fn test_render_poster_wordpress_given_each_document_tag_omits_the_scaffolding(#[case] tag: &str) {
    // Arrange
    let fragment = wordpress_poster_page(kind_field(2, 2));

    // Assert
    assert!(!has_tag(&fragment, tag));
}

#[rstest]
#[case("<script")]
#[case("id=\"race-data\"")]
fn test_render_poster_wordpress_given_each_record_token_omits_it(#[case] token: &str) {
    // Arrange
    let fragment = wordpress_poster_page(kind_field(2, 2));

    // Assert
    assert!(!fragment.contains(token));
}

#[test]
fn test_render_poster_wordpress_wraps_the_content_in_the_scope_wrapper() {
    // Arrange
    let fragment = wordpress_poster_page(kind_field(2, 2));

    // Assert
    assert_eq!(
        (
            fragment.starts_with("<style>"),
            fragment.matches("<div class=\"rc-results\"").count(),
            fragment.trim_end().ends_with("</div>"),
        ),
        (true, 1, true)
    );
}

#[test]
fn test_render_poster_wordpress_inlines_the_scoped_stylesheet_not_the_unscoped_one() {
    // Arrange
    let fragment = wordpress_poster_page(kind_field(2, 2));

    // Assert
    assert_eq!(
        (
            fragment.contains("/* rivercrossing compiled_css_wp"),
            fragment.contains("/* rivercrossing compiled_css —"),
            fragment.contains(".rc-results "),
        ),
        (true, false, true)
    );
}

#[test]
fn test_render_poster_wordpress_header_carries_the_unique_riders_counter() {
    // Arrange
    let mut input = input_with(pair_field(), ExportOptions::default());
    input.riders = 207;

    // Act
    let fragment = render_poster_wordpress(&input).expect("the fragment renders");

    // Assert
    assert_eq!(
        (
            fragment.contains("entries · laps · cards dealt · unique riders"),
            fragment.contains("2 · 21 · 10 · 207"),
        ),
        (true, true)
    );
}

#[test]
fn test_render_poster_wordpress_given_no_rider_count_renders_zero() {
    // Arrange
    let fragment = wordpress_poster_page(pair_field());

    // Assert
    assert!(fragment.contains("2 · 21 · 10 · 0"));
}

#[test]
fn test_render_poster_wordpress_renders_podium_cards() {
    // Arrange
    let fragment = wordpress_poster_page(pair_field());

    // Assert
    assert_eq!(
        (
            fragment.contains("Best poker hands"),
            fragment.contains("class=\"chip"),
        ),
        (true, true)
    );
}

#[test]
fn test_render_poster_wordpress_given_a_single_solo_entry_renders_one_card() {
    // Arrange
    let fragment = wordpress_poster_page(vec![placed(
        Some(1),
        coded_entry("88", "Moss Ridge Riders", SnapshotKind::Solo, 11, &["9S"]),
    )]);

    // Assert
    assert_eq!(fragment.matches("<div class=\"bp p-5").count(), 1);
}

#[test]
fn test_render_poster_wordpress_given_a_team_field_titles_each_kinds_section() {
    // Arrange
    let fragment = wordpress_poster_page(kind_field(2, 2));

    // Assert
    assert_eq!(
        heading_texts(&fragment, 3),
        vec!["Teams".to_owned(), "Solo riders".to_owned()]
    );
}

#[test]
fn test_render_poster_wordpress_given_an_empty_field_renders_no_section() {
    // Arrange
    let fragment = wordpress_poster_page(Vec::new());

    // Assert
    assert_eq!(
        (
            fragment.contains("<section"),
            fragment.contains("Best poker hands"),
        ),
        (false, true)
    );
}

#[test]
fn test_render_poster_wordpress_given_all_cards_prints_the_whole_hand_line() {
    // Arrange
    let fragment = wordpress_poster_page(pair_field());

    // Assert
    assert!(fragment.contains("All 5 cards, in draw order: "));
}

#[test]
fn test_render_poster_wordpress_given_a_drawn_card_renders_the_draw_badge() {
    // Arrange
    let mut result = coded_entry("88", "Moss Ridge Riders", SnapshotKind::Solo, 11, &["9S"]);
    result.tiebreak_card = Some(card("AH"));
    let fragment = wordpress_poster_page(vec![placed(Some(1), result)]);

    // Assert
    assert!(fragment.contains("draw <span class=\"chip r\">A ♥\u{fe0e}</span>"));
}

#[test]
fn test_render_poster_wordpress_given_an_unverified_ride_carries_the_self_test_note() {
    // Arrange
    let mut input = input_with(pair_field(), ExportOptions::default());
    input.self_test_unverified = true;

    // Act
    let fragment = render_poster_wordpress(&input).expect("the fragment renders");

    // Assert
    assert!(fragment.contains(SELF_TEST_NOTE));
}

#[test]
fn test_render_poster_wordpress_given_a_non_numeric_plate_returns_the_plate_error() {
    // Arrange
    let input = input_with(
        vec![placed(
            Some(1),
            coded_entry("ABC", "Bad Plate", SnapshotKind::Solo, 3, &["9S"]),
        )],
        ExportOptions::default(),
    );

    // Act
    let result = render_poster_wordpress(&input);

    // Assert
    assert!(matches!(
        result,
        Err(ExportError::PlateNotNumeric { plate }) if plate == "ABC"
    ));
}

#[test]
fn test_render_poster_wordpress_given_no_logo_embeds_the_transparent_png() {
    // Arrange
    let fragment = wordpress_poster_page(pair_field());

    // Assert
    assert_eq!(
        (
            fragment.contains(TRANSPARENT_PNG),
            fragment.contains("src=\"\""),
        ),
        (true, false)
    );
}

// ------------------------------------------------- renderer edge cases

#[rstest]
#[case("!doctype")]
#[case("html")]
#[case("head")]
#[case("body")]
fn test_render_page_given_a_record_carries_each_document_tag(#[case] tag: &str) {
    // Arrange
    let input = input_with(pair_field(), options(true, true, false));

    // Act
    let page = render(&input).expect("the page renders");

    // Assert
    assert!(has_tag(&page, tag));
}

#[test]
fn test_render_wordpress_given_a_non_numeric_plate_returns_the_plate_error() {
    // Arrange
    let input = input_with(
        vec![placed(
            Some(1),
            coded_entry("ABC", "Bad Plate", SnapshotKind::Solo, 3, &["9S"]),
        )],
        ExportOptions::default(),
    );

    // Act
    let result = render_wordpress(&input);

    // Assert
    assert!(matches!(
        result,
        Err(ExportError::PlateNotNumeric { plate }) if plate == "ABC"
    ));
}

#[test]
fn test_render_poster_given_a_team_logo_omits_it() {
    // Arrange
    let mut input = input_with(poster_field(), ExportOptions::default());
    input
        .team_logos
        .insert("88".to_owned(), FAKE_LOGO.to_owned());

    // Act
    let page = render_poster(&input).expect("the poster renders");

    // Assert
    assert!(!page.contains(FAKE_LOGO));
}

#[test]
fn test_render_wordpress_given_a_team_logo_renders_it() {
    // Arrange
    let mut input = input_with(kind_field(1, 0), options(true, true, false));
    input
        .team_logos
        .insert("500".to_owned(), FAKE_LOGO.to_owned());

    // Act
    let fragment = render_wordpress(&input).expect("the fragment renders");

    // Assert
    assert!(fragment.contains(FAKE_LOGO));
}

#[test]
fn test_render_template_given_a_missing_template_returns_the_template_error() {
    // Arrange
    let context = JinjaValue::from(0);

    // Act
    let result = render_template("absent.html.j2", &context);

    // Assert
    assert!(matches!(result, Err(ExportError::Template(_))));
}

#[test]
fn test_render_template_given_a_context_missing_a_key_returns_the_template_error() {
    // Arrange
    let context = JinjaValue::from(0);

    // Act
    let result = render_template("poster.html.j2", &context);

    // Assert
    assert!(matches!(result, Err(ExportError::Template(_))));
}
