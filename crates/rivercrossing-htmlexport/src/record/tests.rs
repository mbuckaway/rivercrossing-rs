// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the canonical `race-data` record writer and reader.

use pretty_assertions::assert_eq;
use rstest::rstest;
use serde_json::Value;

use super::{Json, from_record, render, to_json};
use crate::error::ExportError;
use crate::types::{
    CardPair, EventInfo, ExportOptions, LapsBoardRow, RacePayload, ResultRow, TimeBoardRow,
};

fn pair(rank: &str, suit: &str) -> CardPair {
    CardPair {
        rank: rank.to_owned(),
        suit: suit.to_owned(),
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
        laps: 2,
        cards: 3,
        riders: 4,
    }
}

fn options() -> ExportOptions {
    ExportOptions {
        show_times: true,
        laps_board: true,
        time_board: false,
        full_field: true,
        all_cards: true,
        lap_km: 8.0,
    }
}

fn flagged_row() -> ResultRow {
    ResultRow {
        place: 1,
        plate: 7,
        entry: "e".to_owned(),
        entry_type: "SOLO".to_owned(),
        laps: 2,
        hand: "h".to_owned(),
        total: None,
        best_lap: None,
        sex: Some("M".to_owned()),
        tie: true,
        dnf: true,
        dns: true,
        cards: vec![pair("A", "s")],
        drawn: Vec::new(),
        draw: Some(pair("K", "h")),
        logo: Some("L".to_owned()),
    }
}

fn plain_row() -> ResultRow {
    ResultRow {
        place: 3,
        plate: 12,
        entry: "Ana".to_owned(),
        entry_type: "SOLO".to_owned(),
        laps: 0,
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

fn flagged_payload() -> RacePayload {
    RacePayload {
        event: event(),
        options: options(),
        tie_note: None,
        results: vec![flagged_row()],
        laps_board: vec![LapsBoardRow {
            plate: 7,
            entry: "e".to_owned(),
            entry_type: "TEAM".to_owned(),
            laps: 2,
            total: None,
        }],
        time_board: vec![TimeBoardRow {
            plate: 7,
            entry: "e".to_owned(),
            laps: 2,
            total: "t".to_owned(),
            avg: "a".to_owned(),
        }],
        self_test_note: Some("n".to_owned()),
    }
}

fn plain_payload() -> RacePayload {
    RacePayload {
        event: event(),
        options: ExportOptions {
            show_times: false,
            ..options()
        },
        tie_note: Some("note".to_owned()),
        results: vec![plain_row()],
        laps_board: Vec::new(),
        time_board: Vec::new(),
        self_test_note: None,
    }
}

fn record_of(payload: &RacePayload) -> Value {
    serde_json::from_str(&to_json(payload)).expect("the record parses")
}

#[rstest]
#[case("payload-times.json")]
#[case("payload-no-times.json")]
#[case("payload-solo.json")]
fn test_to_json_given_a_fixture_reads_back_byte_for_byte(#[case] name: &str) {
    // Arrange
    let raw = match name {
        "payload-no-times.json" => include_str!("../../fixtures/payload-no-times.json"),
        "payload-solo.json" => include_str!("../../fixtures/payload-solo.json"),
        _ => include_str!("../../fixtures/payload-times.json"),
    };
    let expected = raw
        .strip_suffix('\n')
        .expect("each fixture ends with one newline");

    // Act
    let value: Value = serde_json::from_str(raw).expect("the fixture is valid JSON");
    let payload = from_record(&value).expect("the fixture is a readable record");

    // Assert
    assert_eq!(to_json(&payload), expected);
}

#[expect(
    clippy::too_many_lines,
    reason = "the golden asserts the whole record byte for byte, so it is long by nature"
)]
#[test]
fn test_to_json_given_a_flagged_row_writes_the_canonical_order() {
    // Arrange
    let payload = flagged_payload();

    // Act
    let text = to_json(&payload);

    // Assert
    assert_eq!(
        text,
        r#"{
  "event": {
    "kicker": "K",
    "title": "T",
    "meta": "M",
    "organizer": "O",
    "scorer": "S",
    "generated": "G",
    "entries": 1,
    "laps": 2,
    "cards": 3
  },
  "options": {
    "showTimes": true,
    "lapsBoard": true,
    "timeBoard": false,
    "fullField": true,
    "allCards": true
  },
  "tieNote": null,
  "results": [
    {
      "place": 1,
      "plate": 7,
      "entry": "e",
      "type": "SOLO",
      "sex": "M",
      "laps": 2,
      "total": null,
      "bestLap": null,
      "hand": "h",
      "tie": true,
      "dnf": true,
      "dns": true,
      "cards": [
        [
          "A",
          "s"
        ]
      ],
      "drawn": [],
      "draw": [
        "K",
        "h"
      ],
      "logo": "L"
    }
  ],
  "lapsBoard": [
    {
      "plate": 7,
      "entry": "e",
      "type": "TEAM",
      "laps": 2,
      "total": null
    }
  ],
  "timeBoard": [
    {
      "plate": 7,
      "entry": "e",
      "laps": 2,
      "total": "t",
      "avg": "a"
    }
  ],
  "selfTestNote": "n"
}"#
    );
}

#[test]
fn test_to_json_given_a_plain_row_leaves_every_sparse_key_out() {
    // Arrange
    let payload = plain_payload();

    // Act
    let value = record_of(&payload);
    let row = value["results"][0]
        .as_object()
        .expect("the result row is an object");
    let keys: Vec<&str> = row.keys().map(String::as_str).collect();

    // Assert
    assert_eq!(
        keys,
        vec![
            "cards", "drawn", "entry", "hand", "laps", "place", "plate", "type"
        ]
    );
    assert_eq!(value.get("selfTestNote"), None);
    assert_eq!(value["tieNote"], Value::from("note"));
    assert_eq!(value["event"].get("riders"), None);
    assert_eq!(value["options"].get("lapKm"), None);
    assert_eq!(value["lapsBoard"], Value::Array(Vec::new()));
    assert_eq!(value["timeBoard"], Value::Array(Vec::new()));
}

#[test]
fn test_to_json_given_a_show_times_off_payload_omits_the_time_keys() {
    // Arrange
    let payload = plain_payload();

    // Act
    let text = to_json(&payload);

    // Assert
    assert!(!text.contains("\"total\""));
    assert!(!text.contains("\"bestLap\""));
}

#[test]
fn test_to_json_given_a_team_name_with_a_script_end_tag_escapes_the_slash() {
    // Arrange
    let mut payload = flagged_payload();
    payload.results[0].entry = "Moss </script> Riders".to_owned();

    // Act
    let text = to_json(&payload);

    // Assert
    assert!(text.contains(r#""entry": "Moss <\/script> Riders""#));
    assert!(!text.contains("</script>"));
}

#[test]
fn test_to_json_given_a_flagged_payload_round_trips_through_from_record() {
    // Arrange
    let payload = flagged_payload();
    let first = to_json(&payload);

    // Act
    let value: Value = serde_json::from_str(&first).expect("the record parses");
    let read = from_record(&value).expect("the record reads back");

    // Assert
    assert_eq!(to_json(&read), first);
}

#[test]
fn test_from_record_given_a_record_without_results_returns_a_record_error() {
    // Arrange
    let mut value = record_of(&flagged_payload());
    value
        .as_object_mut()
        .expect("the record is an object")
        .remove("results");

    // Act
    let result = from_record(&value);

    // Assert
    assert!(matches!(result, Err(ExportError::Json(_))));
}

#[test]
fn test_from_record_given_a_record_without_event_returns_a_record_error() {
    // Arrange
    let mut value = record_of(&flagged_payload());
    value
        .as_object_mut()
        .expect("the record is an object")
        .remove("event");

    // Act
    let result = from_record(&value);

    // Assert
    assert!(matches!(result, Err(ExportError::Json(_))));
}

#[test]
fn test_from_record_given_results_that_are_not_an_array_returns_a_record_error() {
    // Arrange
    let mut value = record_of(&flagged_payload());
    value["results"] = Value::Bool(false);

    // Act
    let result = from_record(&value);

    // Assert
    assert!(matches!(result, Err(ExportError::Json(_))));
}

#[test]
fn test_from_record_given_a_lap_count_above_the_u32_range_returns_a_record_error() {
    // Arrange
    let mut value = record_of(&flagged_payload());
    value["results"][0]["laps"] = Value::from(5_000_000_000_i64);

    // Act
    let result = from_record(&value);

    // Assert
    assert!(matches!(result, Err(ExportError::Json(_))));
}

#[test]
fn test_from_record_given_an_entry_count_above_the_usize_range_returns_a_record_error() {
    // Arrange
    let mut value = record_of(&flagged_payload());
    value["event"]["entries"] = Value::from(-1);

    // Act
    let result = from_record(&value);

    // Assert
    assert!(matches!(result, Err(ExportError::Json(_))));
}

#[test]
fn test_from_record_given_a_row_without_cards_reads_no_cards() {
    // Arrange
    let mut value = record_of(&flagged_payload());
    value["results"][0]
        .as_object_mut()
        .expect("the result row is an object")
        .remove("cards");

    // Act
    let read = from_record(&value).expect("the row reads back");

    // Assert
    assert_eq!(read.results[0].cards, Vec::new());
}

#[test]
fn test_from_record_given_a_laps_row_without_a_kind_reads_an_empty_kind() {
    // Arrange
    let mut value = record_of(&flagged_payload());
    value["lapsBoard"][0]
        .as_object_mut()
        .expect("the laps row is an object")
        .remove("type");

    // Act
    let read = from_record(&value).expect("the row reads back");

    // Assert
    assert_eq!(read.laps_board[0].entry_type, "");
}

#[test]
fn test_from_record_given_a_non_integer_lap_count_returns_a_record_error() {
    // Arrange
    let mut value = record_of(&flagged_payload());
    value["results"][0]["laps"] = Value::from("four");

    // Act
    let result = from_record(&value);

    // Assert
    assert!(matches!(result, Err(ExportError::Json(_))));
}

#[test]
fn test_from_record_given_a_tie_note_that_is_a_number_returns_a_record_error() {
    // Arrange
    let mut value = record_of(&flagged_payload());
    value["tieNote"] = Value::from(7);

    // Act
    let result = from_record(&value);

    // Assert
    assert!(matches!(result, Err(ExportError::Json(_))));
}

#[test]
fn test_from_record_given_a_flag_that_is_not_a_boolean_returns_a_record_error() {
    // Arrange
    let mut value = record_of(&flagged_payload());
    value["results"][0]["tie"] = Value::from("yes");

    // Act
    let result = from_record(&value);

    // Assert
    assert!(matches!(result, Err(ExportError::Json(_))));
}

#[test]
fn test_from_record_given_a_card_pair_of_the_wrong_length_returns_a_record_error() {
    // Arrange
    let mut value = record_of(&flagged_payload());
    value["results"][0]["cards"] = serde_json::json!([["A"]]);

    // Act
    let result = from_record(&value);

    // Assert
    assert!(matches!(result, Err(ExportError::Json(_))));
}

#[test]
fn test_from_record_given_a_card_suit_that_is_a_number_returns_a_record_error() {
    // Arrange
    let mut value = record_of(&flagged_payload());
    value["results"][0]["cards"] = serde_json::json!([["A", 2]]);

    // Act
    let result = from_record(&value);

    // Assert
    assert!(matches!(result, Err(ExportError::Json(_))));
}

#[test]
fn test_from_record_given_a_top_level_that_is_not_an_object_returns_a_record_error() {
    // Arrange
    let value = Value::Array(Vec::new());

    // Act
    let result = from_record(&value);

    // Assert
    assert!(matches!(result, Err(ExportError::Json(_))));
}

#[test]
fn test_render_given_an_empty_object_writes_empty_braces() {
    // Arrange
    let value = Json::Object(Vec::new());

    // Act
    let text = render(&value);

    // Assert
    assert_eq!(text, "{}");
}

#[test]
fn test_render_given_an_empty_array_writes_empty_brackets() {
    // Arrange
    let value = Json::Array(Vec::new());

    // Act
    let text = render(&value);

    // Assert
    assert_eq!(text, "[]");
}

#[test]
fn test_render_given_a_json_string_escapes_like_python() {
    // Arrange
    let value = Json::Str("\"\\\n\r\t\u{8}\u{c}\u{1}·".to_owned());

    // Act
    let text = render(&value);

    // Assert
    assert_eq!(text, r#""\"\\\n\r\t\b\f\u0001·""#);
}
