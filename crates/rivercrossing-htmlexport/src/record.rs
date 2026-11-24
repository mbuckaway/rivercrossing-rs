// SPDX-License-Identifier: GPL-3.0-only

//! The canonical `race-data` record: a hand-written ordered writer and reader.
//!
//! The record is `camelCase` and sparse while the template context is
//! `snake_case`, so one struct family cannot serve both. This module emits the
//! record with Python's `json.dumps(indent=2, ensure_ascii=False)` layout and the
//! oracle's exact key order, then escapes every `</` as `<\/` so the record can
//! never close its own `<script>` element.

use std::fmt::Write as _;

use serde_json::{Map, Value};

use crate::error::ExportError;
use crate::types::{
    CardPair, DEFAULT_LAP_KM, EventInfo, ExportOptions, LapsBoardRow, RacePayload, ResultRow,
    TimeBoardRow,
};

/// One node of the ordered JSON tree that the record is built from.
#[derive(Debug)]
enum Json {
    /// A JSON null.
    Null,
    /// A JSON boolean.
    Bool(bool),
    /// A JSON integer.
    Int(i64),
    /// A JSON string.
    Str(String),
    /// A JSON array.
    Array(Vec<Self>),
    /// A JSON object whose field order is its insertion order.
    Object(Vec<(&'static str, Self)>),
}

/// Return *payload* as the canonical `race-data` record.
///
/// The text carries no trailing newline and every `</` is escaped as `<\/`.
#[must_use]
pub fn to_json(payload: &RacePayload) -> String {
    render(&record_json(payload)).replace("</", "<\\/")
}

/// Read a `race-data` record back into a [`RacePayload`].
///
/// The fields that the record omits when unset read back as their defaults: a
/// row without `tie`/`dnf`/`dns` is unflagged, a row without `sex`/`total`/
/// `bestLap`/`draw`/`logo` has none, and a laps-board row without `type` reads
/// as the empty kind.
///
/// # Errors
///
/// Return [`ExportError::Json`] when the record is not an object, when a
/// required block or field is missing, or when a field has the wrong type.
pub fn from_record(value: &Value) -> Result<RacePayload, ExportError> {
    let root = object(value, "the record")?;
    Ok(RacePayload {
        event: event_from(required(root, "event", "the record")?)?,
        options: options_from(required(root, "options", "the record")?)?,
        tie_note: optional_text(root, "tieNote")?,
        results: results_from(required(root, "results", "the record")?)?,
        laps_board: laps_board_from(required(root, "lapsBoard", "the record")?)?,
        time_board: time_board_from(required(root, "timeBoard", "the record")?)?,
        self_test_note: optional_text(root, "selfTestNote")?,
    })
}

/// Return the ordered JSON tree of *payload*.
fn record_json(payload: &RacePayload) -> Json {
    let show_times = payload.options.show_times;
    let mut fields = vec![
        ("event", event_json(&payload.event)),
        ("options", options_json(&payload.options)),
        ("tieNote", optional_str_json(payload.tie_note.as_deref())),
        (
            "results",
            Json::Array(
                payload
                    .results
                    .iter()
                    .map(|row| result_json(row, show_times))
                    .collect(),
            ),
        ),
        (
            "lapsBoard",
            Json::Array(
                payload
                    .laps_board
                    .iter()
                    .map(|row| laps_json(row, show_times))
                    .collect(),
            ),
        ),
        (
            "timeBoard",
            Json::Array(payload.time_board.iter().map(time_json).collect()),
        ),
    ];
    if let Some(note) = &payload.self_test_note {
        fields.push(("selfTestNote", Json::Str(note.clone())));
    }
    Json::Object(fields)
}

/// Return the record's `event` block.
fn event_json(event: &EventInfo) -> Json {
    Json::Object(vec![
        ("kicker", Json::Str(event.kicker.clone())),
        ("title", Json::Str(event.title.clone())),
        ("meta", Json::Str(event.meta.clone())),
        ("organizer", Json::Str(event.organizer.clone())),
        ("scorer", Json::Str(event.scorer.clone())),
        ("generated", Json::Str(event.generated.clone())),
        ("entries", Json::Int(length(event.entries))),
        ("laps", Json::Int(i64::from(event.laps))),
        ("cards", Json::Int(length(event.cards))),
    ])
}

/// Return the record's `options` block.
fn options_json(options: &ExportOptions) -> Json {
    Json::Object(vec![
        ("showTimes", Json::Bool(options.show_times)),
        ("lapsBoard", Json::Bool(options.laps_board)),
        ("timeBoard", Json::Bool(options.time_board)),
        ("fullField", Json::Bool(options.full_field)),
        ("allCards", Json::Bool(options.all_cards)),
    ])
}

/// Return one result row, sparse exactly as the oracle writes it.
fn result_json(row: &ResultRow, show_times: bool) -> Json {
    let mut fields = vec![
        ("place", Json::Int(row.place)),
        ("plate", Json::Int(row.plate)),
        ("entry", Json::Str(row.entry.clone())),
        ("type", Json::Str(row.entry_type.clone())),
    ];
    if let Some(sex) = &row.sex {
        fields.push(("sex", Json::Str(sex.clone())));
    }
    fields.push(("laps", Json::Int(i64::from(row.laps))));
    if show_times {
        fields.push(("total", optional_str_json(row.total.as_deref())));
        fields.push(("bestLap", optional_str_json(row.best_lap.as_deref())));
    }
    fields.push(("hand", Json::Str(row.hand.clone())));
    for (key, flag) in [("tie", row.tie), ("dnf", row.dnf), ("dns", row.dns)] {
        if flag {
            fields.push((key, Json::Bool(true)));
        }
    }
    fields.push(("cards", cards_json(&row.cards)));
    fields.push(("drawn", cards_json(&row.drawn)));
    if let Some(draw) = &row.draw {
        fields.push(("draw", card_json(draw)));
    }
    if let Some(logo) = &row.logo {
        fields.push(("logo", Json::Str(logo.clone())));
    }
    Json::Object(fields)
}

/// Return one most-laps row.
fn laps_json(row: &LapsBoardRow, show_times: bool) -> Json {
    let mut fields = vec![
        ("plate", Json::Int(row.plate)),
        ("entry", Json::Str(row.entry.clone())),
        ("type", Json::Str(row.entry_type.clone())),
        ("laps", Json::Int(i64::from(row.laps))),
    ];
    if show_times {
        fields.push(("total", optional_str_json(row.total.as_deref())));
    }
    Json::Object(fields)
}

/// Return one fastest-laps row.
fn time_json(row: &TimeBoardRow) -> Json {
    Json::Object(vec![
        ("plate", Json::Int(row.plate)),
        ("entry", Json::Str(row.entry.clone())),
        ("laps", Json::Int(i64::from(row.laps))),
        ("total", Json::Str(row.total.clone())),
        ("avg", Json::Str(row.avg.clone())),
    ])
}

/// Return the record pair of *pair*.
fn card_json(pair: &CardPair) -> Json {
    Json::Array(vec![
        Json::Str(pair.rank.clone()),
        Json::Str(pair.suit.clone()),
    ])
}

/// Return the record array of *cards*.
fn cards_json(cards: &[CardPair]) -> Json {
    Json::Array(cards.iter().map(card_json).collect())
}

/// Return *value* as a JSON string, or null.
fn optional_str_json(value: Option<&str>) -> Json {
    value.map_or(Json::Null, |text| Json::Str(text.to_owned()))
}

/// Return *value* as the record's integer, saturated above `i64::MAX`.
fn length(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Return the Python-formatted text of *value*.
fn render(value: &Json) -> String {
    let mut out = String::new();
    write_value(value, 0, &mut out);
    out
}

/// Append *value* at *indent* to *out*.
fn write_value(value: &Json, indent: usize, out: &mut String) {
    match value {
        Json::Null => out.push_str("null"),
        Json::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        Json::Int(number) => out.push_str(&number.to_string()),
        Json::Str(text) => write_string(text, out),
        Json::Array(items) => write_array(items, indent, out),
        Json::Object(fields) => write_object(fields, indent, out),
    }
}

/// Append one array at *indent* to *out*.
fn write_array(items: &[Json], indent: usize, out: &mut String) {
    if items.is_empty() {
        out.push_str("[]");
        return;
    }
    out.push_str("[\n");
    for (index, item) in items.iter().enumerate() {
        indent_line(out, indent + 1);
        write_value(item, indent + 1, out);
        if index + 1 < items.len() {
            out.push(',');
        }
        out.push('\n');
    }
    indent_line(out, indent);
    out.push(']');
}

/// Append one object at *indent* to *out*.
fn write_object(fields: &[(&'static str, Json)], indent: usize, out: &mut String) {
    if fields.is_empty() {
        out.push_str("{}");
        return;
    }
    out.push_str("{\n");
    for (index, (key, value)) in fields.iter().enumerate() {
        indent_line(out, indent + 1);
        write_string(key, out);
        out.push_str(": ");
        write_value(value, indent + 1, out);
        if index + 1 < fields.len() {
            out.push(',');
        }
        out.push('\n');
    }
    indent_line(out, indent);
    out.push('}');
}

/// Append *level* two-space indents to *out*.
fn indent_line(out: &mut String, level: usize) {
    for _ in 0..level {
        out.push_str("  ");
    }
}

/// Append the Python-formatted JSON literal of *text* to *out*.
fn write_string(text: &str, out: &mut String) {
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            // Writing to a `String` cannot fail, so the result is discarded.
            control if control < ' ' => {
                let _ = write!(out, "\\u{:04x}", u32::from(control));
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

/// Return *value* as a JSON object, or the record error naming *context*.
fn object<'a>(value: &'a Value, context: &str) -> Result<&'a Map<String, Value>, ExportError> {
    value
        .as_object()
        .ok_or_else(|| ExportError::Json(format!("{context} is not a JSON object")))
}

/// Return *value* as a JSON array, or the record error naming *context*.
fn array<'a>(value: &'a Value, context: &str) -> Result<&'a Vec<Value>, ExportError> {
    value
        .as_array()
        .ok_or_else(|| ExportError::Json(format!("{context} is not a JSON array")))
}

/// Return the value of *key*, or the record error naming *context*.
fn required<'a>(
    map: &'a Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<&'a Value, ExportError> {
    map.get(key)
        .ok_or_else(|| ExportError::Json(format!("{context} is missing '{key}'")))
}

/// Return the string field *key*, or the record error naming *context*.
fn text(map: &Map<String, Value>, key: &str, context: &str) -> Result<String, ExportError> {
    let value = required(map, key, context)?;
    text_value(value, &format!("{context} field '{key}'"))
}

/// Return *value* as a string, or the record error naming *context*.
fn text_value(value: &Value, context: &str) -> Result<String, ExportError> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| ExportError::Json(format!("{context} is not a string")))
}

/// Return the integer field *key*, or the record error naming *context*.
fn integer(map: &Map<String, Value>, key: &str, context: &str) -> Result<i64, ExportError> {
    let value = required(map, key, context)?;
    value
        .as_i64()
        .ok_or_else(|| ExportError::Json(format!("{context} field '{key}' is not an integer")))
}

/// Return the `u32` field *key*, or the record error naming *context*.
fn count_u32(map: &Map<String, Value>, key: &str, context: &str) -> Result<u32, ExportError> {
    let value = integer(map, key, context)?;
    u32::try_from(value)
        .map_err(|_| ExportError::Json(format!("{context} field '{key}' is out of range")))
}

/// Return the `usize` field *key*, or the record error naming *context*.
fn count_usize(map: &Map<String, Value>, key: &str, context: &str) -> Result<usize, ExportError> {
    let value = integer(map, key, context)?;
    usize::try_from(value)
        .map_err(|_| ExportError::Json(format!("{context} field '{key}' is out of range")))
}

/// Return the optional string field *key*; an absent key or a null reads as none.
fn optional_text(map: &Map<String, Value>, key: &str) -> Result<Option<String>, ExportError> {
    match map.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(ExportError::Json(format!(
            "the record field '{key}' is not a string"
        ))),
    }
}

/// Return the optional boolean field *key*; an absent key reads as false.
fn optional_bool(map: &Map<String, Value>, key: &str) -> Result<bool, ExportError> {
    match map.get(key) {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(ExportError::Json(format!(
            "the record field '{key}' is not a boolean"
        ))),
    }
}

/// Return the card list of *key*; an absent key reads as no cards.
fn cards(map: &Map<String, Value>, key: &str) -> Result<Vec<CardPair>, ExportError> {
    let Some(value) = map.get(key) else {
        return Ok(Vec::new());
    };
    array(value, key)?.iter().map(card_pair).collect()
}

/// Return the optional card pair of *key*; an absent key or a null reads as none.
fn optional_card(map: &Map<String, Value>, key: &str) -> Result<Option<CardPair>, ExportError> {
    match map.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => card_pair(value).map(Some),
    }
}

/// Return one record card pair.
fn card_pair(value: &Value) -> Result<CardPair, ExportError> {
    let items = array(value, "a card pair")?;
    let [rank, suit] = items.as_slice() else {
        return Err(ExportError::Json(
            "a card pair must hold exactly two cards".to_owned(),
        ));
    };
    Ok(CardPair {
        rank: text_value(rank, "a card rank")?,
        suit: text_value(suit, "a card suit")?,
    })
}

/// Return the event block of *value*.
fn event_from(value: &Value) -> Result<EventInfo, ExportError> {
    let map = object(value, "event")?;
    Ok(EventInfo {
        kicker: text(map, "kicker", "event")?,
        title: text(map, "title", "event")?,
        meta: text(map, "meta", "event")?,
        organizer: text(map, "organizer", "event")?,
        scorer: text(map, "scorer", "event")?,
        generated: text(map, "generated", "event")?,
        entries: count_usize(map, "entries", "event")?,
        laps: count_u32(map, "laps", "event")?,
        cards: count_usize(map, "cards", "event")?,
        riders: 0,
    })
}

/// Return the options block of *value*; the record never carries `lap_km`.
fn options_from(value: &Value) -> Result<ExportOptions, ExportError> {
    let map = object(value, "options")?;
    Ok(ExportOptions {
        show_times: optional_bool(map, "showTimes")?,
        laps_board: optional_bool(map, "lapsBoard")?,
        time_board: optional_bool(map, "timeBoard")?,
        full_field: optional_bool(map, "fullField")?,
        all_cards: optional_bool(map, "allCards")?,
        lap_km: DEFAULT_LAP_KM,
    })
}

/// Return every result row of *value*.
fn results_from(value: &Value) -> Result<Vec<ResultRow>, ExportError> {
    array(value, "results")?.iter().map(result_from).collect()
}

/// Return one result row of *value*.
fn result_from(value: &Value) -> Result<ResultRow, ExportError> {
    let map = object(value, "a results row")?;
    Ok(ResultRow {
        place: integer(map, "place", "a results row")?,
        plate: integer(map, "plate", "a results row")?,
        entry: text(map, "entry", "a results row")?,
        entry_type: text(map, "type", "a results row")?,
        laps: count_u32(map, "laps", "a results row")?,
        hand: text(map, "hand", "a results row")?,
        total: optional_text(map, "total")?,
        best_lap: optional_text(map, "bestLap")?,
        sex: optional_text(map, "sex")?,
        tie: optional_bool(map, "tie")?,
        dnf: optional_bool(map, "dnf")?,
        dns: optional_bool(map, "dns")?,
        cards: cards(map, "cards")?,
        drawn: cards(map, "drawn")?,
        draw: optional_card(map, "draw")?,
        logo: optional_text(map, "logo")?,
    })
}

/// Return every most-laps row of *value*.
fn laps_board_from(value: &Value) -> Result<Vec<LapsBoardRow>, ExportError> {
    array(value, "lapsBoard")?
        .iter()
        .map(laps_row_from)
        .collect()
}

/// Return one most-laps row of *value*.
fn laps_row_from(value: &Value) -> Result<LapsBoardRow, ExportError> {
    let map = object(value, "a lapsBoard row")?;
    Ok(LapsBoardRow {
        plate: integer(map, "plate", "a lapsBoard row")?,
        entry: text(map, "entry", "a lapsBoard row")?,
        entry_type: optional_text(map, "type")?.unwrap_or_default(),
        laps: count_u32(map, "laps", "a lapsBoard row")?,
        total: optional_text(map, "total")?,
    })
}

/// Return every fastest-laps row of *value*.
fn time_board_from(value: &Value) -> Result<Vec<TimeBoardRow>, ExportError> {
    array(value, "timeBoard")?
        .iter()
        .map(time_row_from)
        .collect()
}

/// Return one fastest-laps row of *value*.
fn time_row_from(value: &Value) -> Result<TimeBoardRow, ExportError> {
    let map = object(value, "a timeBoard row")?;
    Ok(TimeBoardRow {
        plate: integer(map, "plate", "a timeBoard row")?,
        entry: text(map, "entry", "a timeBoard row")?,
        laps: count_u32(map, "laps", "a timeBoard row")?,
        total: text(map, "total", "a timeBoard row")?,
        avg: text(map, "avg", "a timeBoard row")?,
    })
}

#[cfg(test)]
mod tests;
