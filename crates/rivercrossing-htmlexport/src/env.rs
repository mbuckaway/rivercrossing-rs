// SPDX-License-Identifier: GPL-3.0-only

//! The `minijinja` environment and the page context.
//!
//! The environment loads the five vendored templates and the dev stand-in CSS
//! sheet, auto-escapes as `HTML` everywhere, refuses an undefined name, and
//! installs the two oracle extras: a `MarkupSafe`-compatible formatter and the
//! `racejson` filter that keeps a record from closing its own `<script>`
//! element. [`page_context`] maps the typed [`RacePayload`] and [`Sections`] to
//! the template names the page reads.

use minijinja::value::{Serde, Value, ValueKind};
use minijinja::{AutoEscape, Environment, Error, Output, State, UndefinedBehavior};

use crate::record;
use crate::types::{
    CardPair, ExportOptions, LapsBoardRow, RacePayload, ResultRow, Sections, TimeBoardRow,
};

/// Return the environment that renders the vendored templates.
#[must_use]
pub(crate) fn environment() -> Environment<'static> {
    let mut environment = Environment::new();
    environment.set_loader(template);
    environment.set_auto_escape_callback(|_| AutoEscape::Html);
    environment.set_undefined_behavior(UndefinedBehavior::Strict);
    environment.set_formatter(format_value);
    environment.add_filter("racejson", racejson);
    environment
}

/// Return the vendored template *name*, or none.
#[expect(
    clippy::unnecessary_wraps,
    reason = "the loader callback must return a Result, so a bare Option cannot be returned"
)]
fn template(name: &str) -> Result<Option<&'static str>, Error> {
    Ok(match name {
        "base.html.j2" => Some(include_str!("../templates/base.html.j2")),
        "macros.html.j2" => Some(include_str!("../templates/macros.html.j2")),
        "poster.html.j2" => Some(include_str!("../templates/poster.html.j2")),
        "wordpress.html.j2" => Some(include_str!("../templates/wordpress.html.j2")),
        "poster_wordpress.html.j2" => Some(include_str!("../templates/poster_wordpress.html.j2")),
        "theme.css" => Some(include_str!("../templates/theme.css")),
        _ => None,
    })
}

/// Write *value* as the oracle's template text.
///
/// A safe string passes through verbatim; an integral number loses its
/// fraction (`8`, not `8.0`); a plain string is escaped as `MarkupSafe` escapes
/// it when the template auto-escapes as `HTML`; every other kind reads as its
/// own display text.
fn format_value(out: &mut Output, state: &mut State<'_, '_>, value: &Value) -> Result<(), Error> {
    if value.is_safe()
        && let Some(text) = value.as_str()
    {
        return out.write_str(text).map_err(Error::from);
    }
    match value.kind() {
        ValueKind::Number => match value.as_i64() {
            Some(number) => write!(out, "{number}"),
            None => write!(out, "{value}"),
        },
        ValueKind::String => match state.auto_escape() {
            AutoEscape::Html => write_escaped(out, value.as_str().unwrap_or_default()),
            _ => out.write_str(value.as_str().unwrap_or_default()),
        },
        _ => write!(out, "{value}"),
    }
    .map_err(Error::from)
}

/// Append *text* to *out* with the `MarkupSafe` escape table.
///
/// `& < > " '` become their entities; `/` stays untouched.
fn write_escaped(out: &mut Output, text: &str) -> std::fmt::Result {
    for character in text.chars() {
        match character {
            '&' => out.write_str("&amp;")?,
            '<' => out.write_str("&lt;")?,
            '>' => out.write_str("&gt;")?,
            '"' => out.write_str("&#34;")?,
            '\'' => out.write_str("&#39;")?,
            other => {
                let mut buffer = [0_u8; 4];
                out.write_str(other.encode_utf8(&mut buffer))?;
            }
        }
    }
    Ok(())
}

/// Return *text* with every `</` escaped as `<\/`.
fn racejson(text: &str) -> String {
    text.replace("</", "<\\/")
}

/// Return the template context for *payload*, *sections* and the header logo.
///
/// The event block carries the render-only rider counter; the sections carry
/// the page's row names, with every optional row field present as null when it
/// is unset. An absent logo falls back to the transparent PNG. The `payload`
/// entry is the canonical `race-data` record.
#[must_use]
pub(crate) fn page_context(
    payload: &RacePayload,
    sections: &Sections,
    logo_src: Option<&str>,
) -> Value {
    let mut context = serde_json::Map::new();
    context.insert("event".to_owned(), event_value(payload));
    context.insert("options".to_owned(), options_value(&payload.options));
    context.insert("sections".to_owned(), sections_value(sections));
    context.insert(
        "tie_note".to_owned(),
        optional_text(payload.tie_note.as_deref()),
    );
    context.insert(
        "self_test_note".to_owned(),
        optional_text(payload.self_test_note.as_deref()),
    );
    context.insert(
        "logo_src".to_owned(),
        serde_json::Value::from(logo_src.unwrap_or(crate::types::TRANSPARENT_PNG)),
    );
    context.insert(
        "logo_alt".to_owned(),
        serde_json::Value::from(payload.event.organizer.as_str()),
    );
    context.insert("dev".to_owned(), serde_json::Value::from(false));
    context.insert(
        "compiled_css".to_owned(),
        serde_json::Value::from(include_str!("../templates/compiled_css")),
    );
    context.insert(
        "fonts_css".to_owned(),
        serde_json::Value::from(include_str!("../templates/fonts_css")),
    );
    context.insert(
        "payload".to_owned(),
        serde_json::Value::from(record::to_json(payload)),
    );
    Value::from(Serde(serde_json::Value::Object(context)))
}

/// Return *payload*'s event block for the header macro.
fn event_value(payload: &RacePayload) -> serde_json::Value {
    let event = &payload.event;
    serde_json::json!({
        "kicker": event.kicker,
        "title": event.title,
        "meta": event.meta,
        "organizer": event.organizer,
        "scorer": event.scorer,
        "generated": event.generated,
        "entries": event.entries,
        "laps": event.laps,
        "cards": event.cards,
        "riders": event.riders,
    })
}

/// Return *options* as the template's snake-case publish settings.
fn options_value(options: &ExportOptions) -> serde_json::Value {
    serde_json::json!({
        "show_times": options.show_times,
        "laps_board": options.laps_board,
        "time_board": options.time_board,
        "full_field": options.full_field,
        "all_cards": options.all_cards,
        "lap_km": options.lap_km,
    })
}

/// Return *sections* as the template's snake-case section plan.
fn sections_value(sections: &Sections) -> serde_json::Value {
    serde_json::json!({
        "podium_teams": rows_value(&sections.podium_teams),
        "podium_solo": rows_value(&sections.podium_solo),
        "top_teams": rows_value(&sections.top_teams),
        "top_solo": rows_value(&sections.top_solo),
        "laps_teams": laps_value(&sections.laps_teams),
        "laps_solo": laps_value(&sections.laps_solo),
        "time_board": times_value(&sections.time_board),
        "teams": rows_value(&sections.teams),
        "solo": rows_value(&sections.solo),
        "team_plates": sections.team_plates.iter().copied().collect::<Vec<i64>>(),
    })
}

/// Return *rows* as an array of result-row objects.
fn rows_value(rows: &[ResultRow]) -> serde_json::Value {
    serde_json::Value::Array(rows.iter().map(row_value).collect())
}

/// Return one result row under its template names.
fn row_value(row: &ResultRow) -> serde_json::Value {
    serde_json::json!({
        "place": row.place,
        "plate": row.plate,
        "entry": row.entry,
        "type": row.entry_type,
        "laps": row.laps,
        "hand": row.hand,
        "total": row.total,
        "best_lap": row.best_lap,
        "sex": row.sex,
        "tie": row.tie,
        "dnf": row.dnf,
        "dns": row.dns,
        "cards": cards_value(&row.cards),
        "drawn": cards_value(&row.drawn),
        "draw": row.draw.as_ref().map(card_value),
        "logo": row.logo,
    })
}

/// Return *rows* as an array of most-laps row objects.
fn laps_value(rows: &[LapsBoardRow]) -> serde_json::Value {
    serde_json::Value::Array(
        rows.iter()
            .map(|row| {
                serde_json::json!({
                    "plate": row.plate,
                    "entry": row.entry,
                    "type": row.entry_type,
                    "laps": row.laps,
                    "total": row.total,
                })
            })
            .collect(),
    )
}

/// Return *rows* as an array of fastest-laps row objects.
fn times_value(rows: &[TimeBoardRow]) -> serde_json::Value {
    serde_json::Value::Array(
        rows.iter()
            .map(|row| {
                serde_json::json!({
                    "plate": row.plate,
                    "entry": row.entry,
                    "laps": row.laps,
                    "total": row.total,
                    "avg": row.avg,
                })
            })
            .collect(),
    )
}

/// Return *cards* as an array of `[rank, suit]` pairs.
fn cards_value(cards: &[CardPair]) -> serde_json::Value {
    serde_json::Value::Array(cards.iter().map(card_value).collect())
}

/// Return *pair* as the template's two-element card array.
fn card_value(pair: &CardPair) -> serde_json::Value {
    serde_json::Value::from(vec![pair.rank.as_str(), pair.suit.as_str()])
}

/// Return *value* as a JSON string, or null.
fn optional_text(value: Option<&str>) -> serde_json::Value {
    value.map_or(serde_json::Value::Null, serde_json::Value::from)
}

#[cfg(test)]
mod tests;
