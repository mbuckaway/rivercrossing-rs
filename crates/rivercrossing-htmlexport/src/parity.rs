// SPDX-License-Identifier: GPL-3.0-only

//! Phase 0 parity spike — throwaway proof that minijinja reproduces the Python
//! production golden byte-for-byte. This module is replaced by the TDD
//! `env`/`render` implementation in Phases 1-3; it exists only to de-risk the
//! template-engine decision before any production model code is written.

#[cfg(test)]
mod tests {
    use minijinja::value::ValueKind;
    use minijinja::{AutoEscape, Environment, Error, Output, State, UndefinedBehavior, Value};

    const TRANSPARENT_PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

    /// MarkupSafe's escape table: `& < > " '` as above, `/` untouched.
    fn write_escaped_markupsafe(out: &mut Output, s: &str) -> std::fmt::Result {
        for c in s.chars() {
            match c {
                '&' => out.write_str("&amp;")?,
                '<' => out.write_str("&lt;")?,
                '>' => out.write_str("&gt;")?,
                '"' => out.write_str("&#34;")?,
                '\'' => out.write_str("&#39;")?,
                _ => {
                    let mut buf = [0u8; 4];
                    out.write_str(c.encode_utf8(&mut buf))?;
                }
            }
        }
        Ok(())
    }

    /// The one custom formatter: safe strings verbatim, MarkupSafe HTML escaping,
    /// and integral floats coerced to integers (Jinja's `finalize`).
    fn format_value(out: &mut Output, state: &mut State<'_, '_>, value: &Value) -> Result<(), Error> {
        if value.is_safe() {
            if let Some(s) = value.as_str() {
                return out.write_str(s).map_err(Error::from);
            }
        }
        match value.kind() {
            ValueKind::Number => {
                if let Some(i) = value.as_i64() {
                    write!(out, "{i}")
                } else {
                    write!(out, "{value}")
                }
            }
            ValueKind::String => match state.auto_escape() {
                AutoEscape::Html => write_escaped_markupsafe(out, value.as_str().unwrap_or("")),
                _ => out.write_str(value.as_str().unwrap_or("")),
            },
            _ => write!(out, "{value}"),
        }
        .map_err(Error::from)
    }

    fn racejson(s: &str) -> String {
        s.replace("</", "<\\/")
    }

    fn environment() -> Environment<'static> {
        let mut env = Environment::new();
        env.set_loader(|name| -> Result<Option<&'static str>, Error> {
            Ok(match name {
                "base.html.j2" => Some(include_str!("../templates/base.html.j2")),
                "macros.html.j2" => Some(include_str!("../templates/macros.html.j2")),
                "poster.html.j2" => Some(include_str!("../templates/poster.html.j2")),
                "wordpress.html.j2" => Some(include_str!("../templates/wordpress.html.j2")),
                "poster_wordpress.html.j2" => {
                    Some(include_str!("../templates/poster_wordpress.html.j2"))
                }
                "theme.css" => Some(include_str!("../templates/theme.css")),
                _ => None,
            })
        });
        env.set_auto_escape_callback(|_| AutoEscape::Html);
        env.set_undefined_behavior(UndefinedBehavior::Strict);
        env.set_formatter(format_value);
        env.add_filter("racejson", racejson);
        env
    }

    fn is_team_kind(kind: &str) -> bool {
        kind.to_uppercase().starts_with("TEAM")
    }

    /// Map one record result row to the always-present template row shape.
    fn row_ctx(row: &serde_json::Value) -> serde_json::Value {
        let mut m = serde_json::Map::new();
        m.insert("place".into(), row["place"].clone());
        m.insert("plate".into(), row["plate"].clone());
        m.insert("entry".into(), row["entry"].clone());
        m.insert("type".into(), row["type"].clone());
        m.insert("laps".into(), row["laps"].clone());
        m.insert("hand".into(), row["hand"].clone());
        m.insert("cards".into(), row["cards"].clone());
        m.insert("drawn".into(), row["drawn"].clone());
        for key in ["tie", "dnf", "dns"] {
            let present = row.get(key).and_then(serde_json::Value::as_bool).unwrap_or(false);
            m.insert(key.into(), serde_json::Value::from(present));
        }
        // Template attributes are snake_case; the record uses camelCase for `bestLap`.
        m.insert(
            "sex".into(),
            row.get("sex").cloned().unwrap_or(serde_json::Value::Null),
        );
        m.insert(
            "total".into(),
            row.get("total").cloned().unwrap_or(serde_json::Value::Null),
        );
        m.insert(
            "best_lap".into(),
            row.get("bestLap").cloned().unwrap_or(serde_json::Value::Null),
        );
        m.insert(
            "draw".into(),
            row.get("draw").cloned().unwrap_or(serde_json::Value::Null),
        );
        m.insert(
            "logo".into(),
            row.get("logo").cloned().unwrap_or(serde_json::Value::Null),
        );
        serde_json::Value::Object(m)
    }

    fn laps_row_ctx(row: &serde_json::Value) -> serde_json::Value {
        let mut m = serde_json::Map::new();
        m.insert("plate".into(), row["plate"].clone());
        m.insert("entry".into(), row["entry"].clone());
        m.insert("type".into(), row["type"].clone());
        m.insert("laps".into(), row["laps"].clone());
        m.insert(
            "total".into(),
            row.get("total").cloned().unwrap_or(serde_json::Value::Null),
        );
        serde_json::Value::Object(m)
    }

    fn sections_from_record(record: &serde_json::Value) -> serde_json::Value {
        let results: Vec<serde_json::Value> = record["results"]
            .as_array()
            .expect("results array")
            .iter()
            .map(row_ctx)
            .collect();

        let teams: Vec<serde_json::Value> = results
            .iter()
            .filter(|r| is_team_kind(r["type"].as_str().unwrap_or("")))
            .cloned()
            .collect();
        let solo: Vec<serde_json::Value> = results
            .iter()
            .filter(|r| !is_team_kind(r["type"].as_str().unwrap_or("")))
            .cloned()
            .collect();

        let ranked_teams: Vec<serde_json::Value> = teams
            .iter()
            .filter(|r| !r["dns"].as_bool().unwrap_or(false))
            .cloned()
            .collect();
        let ranked_solo: Vec<serde_json::Value> = solo
            .iter()
            .filter(|r| !r["dns"].as_bool().unwrap_or(false))
            .cloned()
            .collect();
        let top_solo = if teams.is_empty() { 10 } else { 5 };

        let board: Vec<serde_json::Value> = record["lapsBoard"]
            .as_array()
            .expect("lapsBoard array")
            .iter()
            .map(laps_row_ctx)
            .collect();
        let laps_teams: Vec<serde_json::Value> = board
            .iter()
            .filter(|r| is_team_kind(r["type"].as_str().unwrap_or("")))
            .take(5)
            .cloned()
            .collect();
        let laps_solo: Vec<serde_json::Value> = board
            .iter()
            .filter(|r| !is_team_kind(r["type"].as_str().unwrap_or("")))
            .take(top_solo)
            .cloned()
            .collect();

        let team_plates: Vec<i64> = teams
            .iter()
            .filter_map(|r| r["plate"].as_i64())
            .collect();

        let mut m = serde_json::Map::new();
        m.insert(
            "podium_teams".into(),
            serde_json::Value::Array(ranked_teams.iter().take(3).cloned().collect()),
        );
        m.insert(
            "podium_solo".into(),
            serde_json::Value::Array(ranked_solo.iter().take(3).cloned().collect()),
        );
        m.insert(
            "top_teams".into(),
            serde_json::Value::Array(ranked_teams.iter().take(5).cloned().collect()),
        );
        m.insert(
            "top_solo".into(),
            serde_json::Value::Array(ranked_solo.iter().take(top_solo).cloned().collect()),
        );
        m.insert("laps_teams".into(), serde_json::Value::Array(laps_teams));
        m.insert("laps_solo".into(), serde_json::Value::Array(laps_solo));
        m.insert("time_board".into(), record["timeBoard"].clone());
        m.insert("teams".into(), serde_json::Value::Array(teams));
        m.insert("solo".into(), serde_json::Value::Array(solo));
        m.insert("team_plates".into(), serde_json::Value::from(team_plates));
        serde_json::Value::Object(m)
    }

    /// Build the production page context from a parsed payload record.
    fn page_context(record: &serde_json::Value, riders: u64, record_text: &str) -> serde_json::Value {
        let mut event = record["event"].as_object().expect("event").clone();
        event.insert("riders".into(), serde_json::Value::from(riders));

        let opt = record["options"].as_object().expect("options");
        let mut options = serde_json::Map::new();
        options.insert("show_times".into(), opt["showTimes"].clone());
        options.insert("laps_board".into(), opt["lapsBoard"].clone());
        options.insert("time_board".into(), opt["timeBoard"].clone());
        options.insert("full_field".into(), opt["fullField"].clone());
        options.insert("all_cards".into(), opt["allCards"].clone());
        options.insert("lap_km".into(), serde_json::Value::from(8.0f64));

        let mut m = serde_json::Map::new();
        m.insert("event".into(), serde_json::Value::Object(event));
        m.insert("options".into(), serde_json::Value::Object(options));
        m.insert("sections".into(), sections_from_record(record));
        m.insert("tie_note".into(), record["tieNote"].clone());
        m.insert(
            "self_test_note".into(),
            record.get("selfTestNote").cloned().unwrap_or(serde_json::Value::Null),
        );
        m.insert("logo_src".into(), serde_json::Value::from(TRANSPARENT_PNG));
        m.insert("logo_alt".into(), record["event"]["organizer"].clone());
        m.insert("dev".into(), serde_json::Value::from(false));
        m.insert(
            "compiled_css".into(),
            serde_json::Value::from(include_str!("../templates/compiled_css")),
        );
        m.insert(
            "fonts_css".into(),
            serde_json::Value::from(include_str!("../templates/fonts_css")),
        );
        // The race-data record: the fixture file minus its single trailing newline.
        let payload = record_text.strip_suffix('\n').unwrap_or(record_text);
        m.insert("payload".into(), serde_json::Value::from(payload));
        serde_json::Value::Object(m)
    }

    fn first_difference(expected: &str, actual: &str) -> Option<String> {
        let e: Vec<&str> = expected.lines().collect();
        let a: Vec<&str> = actual.lines().collect();
        for (i, (x, y)) in e.iter().zip(a.iter()).enumerate() {
            if x != y {
                return Some(format!("line {}:\n  golden: {x:?}\n  actual: {y:?}", i + 1));
            }
        }
        if e.len() != a.len() {
            Some(format!(
                "line count {} vs {} (actual tail: {:?})",
                e.len(),
                a.len(),
                a.get(e.len())
            ))
        } else {
            None
        }
    }

    #[test]
    fn results_pages_match_production_goldens_byte_for_byte() {
        let env = environment();
        let cases: [(&str, &str, u64); 3] = [
            ("payload-times.json", "epic-2026-results.html", 262),
            ("payload-no-times.json", "epic-2026-results-no-times.html", 262),
            ("payload-solo.json", "epic-2026-results-solo.html", 180),
        ];
        for (payload_file, golden_file, riders) in cases {
            let payload_text = match payload_file {
                "payload-times.json" => include_str!("../fixtures/payload-times.json"),
                "payload-no-times.json" => include_str!("../fixtures/payload-no-times.json"),
                _ => include_str!("../fixtures/payload-solo.json"),
            };
            let golden = match golden_file {
                "epic-2026-results.html" => include_str!("../fixtures/epic-2026-results.html"),
                "epic-2026-results-no-times.html" => {
                    include_str!("../fixtures/epic-2026-results-no-times.html")
                }
                _ => include_str!("../fixtures/epic-2026-results-solo.html"),
            };
            let record: serde_json::Value =
                serde_json::from_str(payload_text).expect("payload");
            let context: serde_json::Value = page_context(&record, riders, payload_text);
            let ctx = minijinja::value::Value::from(minijinja::value::Serde(context));
            let rendered = env
                .get_template("base.html.j2")
                .and_then(|t| t.render(&ctx))
                .expect("render");
            assert_eq!(
                rendered,
                golden,
                "{golden_file}: {}",
                first_difference(golden, &rendered).unwrap_or_default()
            );
        }
    }
}
