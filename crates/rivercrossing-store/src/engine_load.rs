// SPDX-License-Identifier: GPL-3.0-only

//! Rebuilding a ride config, roster and engine from stored rows.
//!
//! The store writes rows; this module reads them back into the ride crate's
//! own types. Every helper here is pure or takes the connection by reference,
//! so a test can drive one reconstruction step at a time.

use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

use rivercrossing_cards::Shoe;
use rivercrossing_ride::{
    EntryMode, Event, EventAction, PlateModel, RideConfig, RideConfigParams, RideEngine,
    RideStatus, SystemClock, TieBreak, Timestamp,
};
use rivercrossing_roster::enums::{EntryStatus, EntryType};
use rivercrossing_roster::roster::Roster;
use rivercrossing_roster::types::{Entry, Rider};
use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;
use time::PrimitiveDateTime;
use time::format_description::well_known::Iso8601;
use uuid::Uuid;

use crate::error::StoreError;
use crate::row::{event_date_from_str, instant_from_epoch, ride_status_from_str};

// The audit actions that record data against one entry. A ride audit row of
// one of these kinds is the third `has_data` source, beside the crossing and
// card tables that the app never writes.
const DATA_AUDIT_SQL: &str = "SELECT payload_json FROM audit WHERE ride_id = ?1 \
     AND action IN ('record_crossing', 'deal_manual', 'add_crossing_at', \
     'assign_plate_to_miss')";

// The payload keys whose ISO-8601 value dates an event for the audit `at`
// column. A payload that carries none of them falls back to the append time.
const EVENT_TIMESTAMP_KEYS: [&str; 5] = [
    "actual_start",
    "crossed_at",
    "stopped_at",
    "finished_at",
    "reopened_at",
];

/// The `ride` columns that rebuild a config, in read order.
const RIDE_RECORD_SQL: &str = "SELECT name, event_date, venue, lap_km, organizer, scorer, \
     logo_png, planned_start, planned_duration_s, entry_mode, max_team_size, plate_model, \
     min_lap_s, deck_count, jokers_per_deck, jokers_mode, max_cards, tiebreak_order, rng_seed, \
     hold_short_laps FROM ride WHERE id = ?1";

/// The `ride` columns that shape a roster, in read order.
const ROSTER_SHAPE_SQL: &str = "SELECT entry_mode, plate_model, max_team_size, rng_seed, status \
     FROM ride WHERE id = ?1";

/// The one ride's entry rows, in creation order.
const ENTRY_SQL: &str = "SELECT id, plate, key, display_name, type, status, notes, logo_card, \
     retired FROM entry WHERE ride_id = ?1 ORDER BY id";

/// One entry's riders, in stored order.
const RIDER_SQL: &str = "SELECT first_name, last_name, plate, sex, sort_order FROM rider \
     WHERE entry_id = ?1 ORDER BY sort_order, id";

/// One ride's audit rows, in append order.
const REPLAY_SQL: &str = "SELECT id, action, payload_json FROM audit WHERE ride_id = ?1 \
     ORDER BY id";

#[cfg(test)]
mod tests;

/// One reconstructed ride setup, with the shoe's own widths kept aside.
struct LoadedConfig {
    /// The rebuilt ride config.
    config: RideConfig,
    /// The ride's stored seed, unsigned.
    seed: u64,
    /// The stored deck count, in the shoe's width.
    decks: u8,
    /// The stored joker count, in the shoe's width.
    jokers_per_deck: u8,
}

/// One stored `ride` row's config columns, read before they are converted.
struct RideRecord {
    name: String,
    event_date: String,
    venue: String,
    lap_km: f64,
    organizer: String,
    scorer: String,
    logo_png: Option<Vec<u8>>,
    planned_start: i64,
    planned_duration_s: i64,
    entry_mode: String,
    max_team_size: i64,
    plate_model: String,
    min_lap_s: i64,
    deck_count: i64,
    jokers_per_deck: i64,
    jokers_mode: String,
    max_cards: Option<i64>,
    tiebreak_order: String,
    rng_seed: i64,
    hold_short_laps: i64,
}

/// One stored `entry` row, read before its riders are loaded.
struct StoredEntry {
    id: i64,
    plate: String,
    key: String,
    display_name: String,
    kind: String,
    status: String,
    notes: Option<String>,
    logo_card: Option<String>,
    retired: i64,
}

/// Rebuild one ride's engine: config, shoe, roster and the replayed log.
///
/// # Errors
///
/// Return [`StoreError::RideNotFound`] when no ride holds `ride_id`, the
/// conversion and replay failures of the helper steps, and a SQLite failure
/// when a query fails.
pub(crate) fn load_engine(
    conn: &Connection,
    ride_id: i64,
) -> Result<RideEngine<Roster, SystemClock>, StoreError> {
    let loaded = load_config(conn, ride_id)?;
    let shoe = build_shoe(&loaded)?;
    let roster = load_roster(conn, ride_id)?;
    let mut engine = RideEngine::new(loaded.config, shoe, SystemClock, roster);
    replay(conn, ride_id, &mut engine)?;
    Ok(engine)
}

/// Rebuild one ride's roster from its entry and rider rows.
///
/// # Errors
///
/// Return [`StoreError::RideNotFound`] when no ride holds `ride_id`,
/// [`StoreError::InvalidRow`] for an unreadable ride or entry column,
/// [`StoreError::OutOfRange`] for a count that does not fit, and
/// [`StoreError::InvalidRoster`] when the ride's shape is outside the roster's
/// bounds.
pub(crate) fn load_roster(conn: &Connection, ride_id: i64) -> Result<Roster, StoreError> {
    let (mode, plate_model, max_team_size, seed, status) = read_roster_shape(conn, ride_id)?;
    let mut roster = Roster::new(mode, max_team_size, plate_model, Some(seed))
        .map_err(|source| StoreError::InvalidRoster { source })?;
    let data_keys = audit_data_keys(conn, ride_id)?;
    let (live, retired) = read_entries(conn, ride_id, &data_keys)?;
    roster.load_entries(live);
    roster.load_retired_entries(retired);
    roster.set_status(status);
    Ok(roster)
}

/// Rebuild one ride's config and the seed its shoe needs.
///
/// # Errors
///
/// Return the conversion failures of each column and
/// [`StoreError::InvalidConfig`] when the stored values fail the setup bounds.
fn load_config(conn: &Connection, ride_id: i64) -> Result<LoadedConfig, StoreError> {
    let record = read_ride_record(conn, ride_id)?;
    let seed = seed_from_i64(record.rng_seed)?;
    let decks = u8_count("deck_count", record.deck_count)?;
    let jokers_per_deck = u8_count("jokers_per_deck", record.jokers_per_deck)?;
    let logo_path = materialize_logo(ride_id, record.logo_png.as_deref())?;
    let params = RideConfigParams {
        name: record.name,
        event_date: event_date_from_str(&record.event_date)?,
        venue: record.venue,
        lap_km: record.lap_km,
        organizer: record.organizer,
        scorer: record.scorer,
        planned_start: instant_from_epoch("ride", "planned_start", record.planned_start)?,
        planned_duration_s: i32_count("planned_duration_s", record.planned_duration_s)?,
        min_lap_s: i32_count("min_lap_s", record.min_lap_s)?,
        entry_mode: entry_mode_from_str(&record.entry_mode)?,
        plate_model: plate_model_from_str(&record.plate_model)?,
        max_team_size: i32_count("max_team_size", record.max_team_size)?,
        deck_count: i32::from(decks),
        jokers_per_deck: i32::from(jokers_per_deck),
        jokers_mode: record.jokers_mode,
        max_cards: max_cards_from(record.max_cards)?,
        tiebreak_order: parse_tiebreak_order(ride_id, &record.tiebreak_order)?,
        logo_path,
        hold_short_laps: record.hold_short_laps != 0,
    };
    let config = RideConfig::new(params).map_err(|source| StoreError::InvalidConfig { source })?;
    Ok(LoadedConfig {
        config,
        seed,
        decks,
        jokers_per_deck,
    })
}

/// Build the replay shoe from the ride's stored structure and seed.
///
/// # Errors
///
/// Return [`StoreError::InvalidShoe`] when the shoe refuses the structure.
fn build_shoe(loaded: &LoadedConfig) -> Result<Shoe, StoreError> {
    Shoe::new(
        loaded.decks,
        loaded.jokers_per_deck,
        loaded.seed,
        loaded.config.jokers_mode,
    )
    .map_err(|source| StoreError::InvalidShoe { source })
}

/// Replay one ride's audit rows onto *engine*, oldest first.
///
/// A row whose action text is not a ride action is skipped: a roster plate
/// change and any foreign row are display-only, never engine events.
///
/// # Errors
///
/// Return [`StoreError::Replay`] when a known action's payload does not decode
/// or the engine refuses it, and [`StoreError::Sqlite`] when the read fails.
fn replay(
    conn: &Connection,
    ride_id: i64,
    engine: &mut RideEngine<Roster, SystemClock>,
) -> Result<(), StoreError> {
    let stored = read_replay_rows(conn, ride_id)?;
    for (row_id, action_text, payload_json) in stored {
        let Some(action) = parse_event_action(&action_text) else {
            continue;
        };
        let payload =
            serde_json::from_str::<Value>(&payload_json).map_err(|error| StoreError::Replay {
                row_id,
                ride_id,
                detail: error.to_string(),
            })?;
        let event = Event { action, payload };
        engine.apply(&event).map_err(|error| StoreError::Replay {
            row_id,
            ride_id,
            detail: error.to_string(),
        })?;
    }
    Ok(())
}

/// Read every audit row that replay may apply, in append order.
fn read_replay_rows(
    conn: &Connection,
    ride_id: i64,
) -> Result<Vec<(i64, String, String)>, StoreError> {
    let mut statement = conn.prepare(REPLAY_SQL)?;
    let rows = statement.query_map([ride_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut stored = Vec::new();
    for row in rows {
        stored.push(row?);
    }
    Ok(stored)
}

/// Read a stored action spelling, or `None` when the ride does not know it.
fn parse_event_action(text: &str) -> Option<EventAction> {
    serde_json::from_value::<EventAction>(Value::String(text.to_owned())).ok()
}

/// Return the first ISO-8601 instant that *payload* carries, if any.
///
/// The keys are tried in the stored order, and a key that does not parse falls
/// through to the next. A naive value is read as UTC, matching the store's
/// instant contract.
pub(crate) fn event_timestamp(payload: &Value) -> Option<Timestamp> {
    for key in EVENT_TIMESTAMP_KEYS {
        let Some(Value::String(text)) = payload.get(key) else {
            continue;
        };
        if let Some(instant) = parse_payload_instant(text) {
            return Some(instant);
        }
    }
    None
}

/// Parse one payload instant.
///
/// An offset-bearing value keeps its recorded offset, so a value written in
/// another zone replays as the same instant. A naive value is read as UTC,
/// matching the store's instant contract.
fn parse_payload_instant(text: &str) -> Option<Timestamp> {
    if let Ok(instant) = Timestamp::parse(text, &Iso8601::DEFAULT) {
        return Some(instant);
    }
    PrimitiveDateTime::parse(text, &Iso8601::DEFAULT)
        .map(PrimitiveDateTime::assume_utc)
        .ok()
}

/// Return the ride status that a lifecycle action writes, if it writes one.
pub(crate) fn lifecycle_status(action: EventAction) -> Option<RideStatus> {
    match action {
        EventAction::Start | EventAction::Continue => Some(RideStatus::Running),
        EventAction::Finish => Some(RideStatus::Finished),
        EventAction::Reopen => Some(RideStatus::Reopened),
        _ => None,
    }
}

/// Return the stored spelling of one audit action.
///
/// # Errors
///
/// Return [`StoreError::Json`] when the action cannot be encoded, and
/// [`StoreError::InvalidRow`] in the unreachable case where it encodes as a
/// non-string.
pub(crate) fn action_text(action: EventAction) -> Result<String, StoreError> {
    match serde_json::to_value(action)? {
        Value::String(text) => Ok(text),
        other => Err(StoreError::InvalidRow {
            table: "audit",
            column: "action",
            stored: other.to_string(),
        }),
    }
}

/// Rebuild one ride's tie-break order from its stored JSON column.
///
/// # Errors
///
/// Return [`StoreError::TiebreakOrder`] unless the text is a JSON array of
/// exactly three known tie-break spellings.
pub(crate) fn parse_tiebreak_order(
    ride_id: i64,
    stored: &str,
) -> Result<[TieBreak; 3], StoreError> {
    serde_json::from_str::<[TieBreak; 3]>(stored).map_err(|_| StoreError::TiebreakOrder {
        ride_id,
        stored: stored.to_owned(),
    })
}

/// Read one ride's config columns.
fn read_ride_record(conn: &Connection, ride_id: i64) -> Result<RideRecord, StoreError> {
    conn.query_row(RIDE_RECORD_SQL, [ride_id], |row| {
        Ok(RideRecord {
            name: row.get(0)?,
            event_date: row.get(1)?,
            venue: row.get(2)?,
            lap_km: row.get(3)?,
            organizer: row.get(4)?,
            scorer: row.get(5)?,
            logo_png: row.get(6)?,
            planned_start: row.get(7)?,
            planned_duration_s: row.get(8)?,
            entry_mode: row.get(9)?,
            max_team_size: row.get(10)?,
            plate_model: row.get(11)?,
            min_lap_s: row.get(12)?,
            deck_count: row.get(13)?,
            jokers_per_deck: row.get(14)?,
            jokers_mode: row.get(15)?,
            max_cards: row.get(16)?,
            tiebreak_order: row.get(17)?,
            rng_seed: row.get(18)?,
            hold_short_laps: row.get(19)?,
        })
    })
    .optional()?
    .ok_or(StoreError::RideNotFound { id: ride_id })
}

/// Read one ride's roster shape columns.
fn read_roster_shape(
    conn: &Connection,
    ride_id: i64,
) -> Result<(EntryMode, PlateModel, u32, u64, RideStatus), StoreError> {
    let stored = conn
        .query_row(ROSTER_SHAPE_SQL, [ride_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .optional()?
        .ok_or(StoreError::RideNotFound { id: ride_id })?;
    Ok((
        entry_mode_from_str(&stored.0)?,
        plate_model_from_str(&stored.1)?,
        u32_count("max_team_size", stored.2)?,
        seed_from_i64(stored.3)?,
        ride_status_from_str(&stored.4)?,
    ))
}

/// Read one ride's entries, split into the live and the retired lists.
fn read_entries(
    conn: &Connection,
    ride_id: i64,
    data_keys: &HashSet<String>,
) -> Result<(Vec<Entry>, Vec<Entry>), StoreError> {
    let stored = read_stored_entries(conn, ride_id)?;
    let mut live = Vec::new();
    let mut retired = Vec::new();
    for row in stored {
        if row.retired != 0 {
            retired.push(restored_entry(row, Vec::new(), true)?);
            continue;
        }
        let riders = read_riders(conn, row.id)?;
        let has_data = entry_has_recorded_data(conn, row.id)? || data_keys.contains(&row.key);
        live.push(restored_entry(row, riders, has_data)?);
    }
    Ok((live, retired))
}

/// Read one ride's entry rows, in creation order.
fn read_stored_entries(conn: &Connection, ride_id: i64) -> Result<Vec<StoredEntry>, StoreError> {
    let mut statement = conn.prepare(ENTRY_SQL)?;
    let rows = statement.query_map([ride_id], stored_entry_from_row)?;
    let mut stored = Vec::new();
    for row in rows {
        stored.push(row?);
    }
    Ok(stored)
}

/// Map one entry row as it is read.
fn stored_entry_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredEntry> {
    Ok(StoredEntry {
        id: row.get(0)?,
        plate: row.get(1)?,
        key: row.get(2)?,
        display_name: row.get(3)?,
        kind: row.get(4)?,
        status: row.get(5)?,
        notes: row.get(6)?,
        logo_card: row.get(7)?,
        retired: row.get(8)?,
    })
}

/// Read one entry's riders, in stored order.
fn read_riders(conn: &Connection, entry_id: i64) -> Result<Vec<Rider>, StoreError> {
    let mut statement = conn.prepare(RIDER_SQL)?;
    let rows = statement.query_map([entry_id], |row| {
        Ok(Rider {
            first_name: row.get(0)?,
            last_name: row.get(1)?,
            plate: row.get(2)?,
            sex: row.get(3)?,
            sort_order: row.get(4)?,
        })
    })?;
    let mut riders = Vec::new();
    for row in rows {
        riders.push(row?);
    }
    Ok(riders)
}

/// Build one entry from its stored row and its restored riders.
fn restored_entry(
    stored: StoredEntry,
    riders: Vec<Rider>,
    has_data: bool,
) -> Result<Entry, StoreError> {
    let kind = EntryType::from_str(&stored.kind).map_err(|_| StoreError::InvalidRow {
        table: "entry",
        column: "type",
        stored: stored.kind.clone(),
    })?;
    let status = EntryStatus::from_str(&stored.status).map_err(|_| StoreError::InvalidRow {
        table: "entry",
        column: "status",
        stored: stored.status.clone(),
    })?;
    let mut entry = Entry::restored(stored.key, stored.plate, stored.display_name, kind, riders);
    entry.status = status;
    entry.notes = stored.notes.unwrap_or_default();
    entry.logo_card = stored.logo_card;
    entry.has_data = has_data;
    Ok(entry)
}

/// Return whether `entry_id` owns a crossing or card row.
fn entry_has_recorded_data(conn: &Connection, entry_id: i64) -> Result<bool, StoreError> {
    let found: i64 = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM crossing WHERE entry_id = ?1) \
         OR EXISTS(SELECT 1 FROM card WHERE entry_id = ?1)",
        [entry_id],
        |row| row.get(0),
    )?;
    Ok(found != 0)
}

/// Return every entry key that a ride's data-recording audit row names.
fn audit_data_keys(conn: &Connection, ride_id: i64) -> Result<HashSet<String>, StoreError> {
    let mut statement = conn.prepare(DATA_AUDIT_SQL)?;
    let rows = statement.query_map([ride_id], |row| row.get::<_, String>(0))?;
    let mut keys = HashSet::new();
    for row in rows {
        let payload_json = row?;
        let Ok(Value::Object(payload)) = serde_json::from_str::<Value>(&payload_json) else {
            continue;
        };
        if let Some(key) = payload.get("entry_id").map(payload_scalar) {
            keys.insert(key);
        }
    }
    Ok(keys)
}

/// Render one payload value as text: a string as itself, anything else as JSON.
fn payload_scalar(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Parse a stored entry-mode spelling.
fn entry_mode_from_str(stored: &str) -> Result<EntryMode, StoreError> {
    EntryMode::from_str(stored).map_err(|_| StoreError::InvalidRow {
        table: "ride",
        column: "entry_mode",
        stored: stored.to_owned(),
    })
}

/// Parse a stored plate-model spelling.
fn plate_model_from_str(stored: &str) -> Result<PlateModel, StoreError> {
    PlateModel::from_str(stored).map_err(|_| StoreError::InvalidRow {
        table: "ride",
        column: "plate_model",
        stored: stored.to_owned(),
    })
}

/// Convert a stored seed, refusing a negative value.
fn seed_from_i64(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::InvalidRow {
        table: "ride",
        column: "rng_seed",
        stored: value.to_string(),
    })
}

/// Convert a stored count to the shoe's `u8` width.
fn u8_count(field: &'static str, value: i64) -> Result<u8, StoreError> {
    u8::try_from(value).map_err(|_| StoreError::OutOfRange {
        field,
        value,
        max: u32::from(u8::MAX),
    })
}

/// Convert a stored count to `u32`.
fn u32_count(field: &'static str, value: i64) -> Result<u32, StoreError> {
    u32::try_from(value).map_err(|_| StoreError::OutOfRange {
        field,
        value,
        max: u32::MAX,
    })
}

/// Convert a stored count to the config's signed width.
fn i32_count(field: &'static str, value: i64) -> Result<i32, StoreError> {
    i32::try_from(value).map_err(|_| StoreError::OutOfRange {
        field,
        value,
        max: 2_147_483_647,
    })
}

/// Convert a stored card cap to `u32`.
fn max_cards_from(value: Option<i64>) -> Result<Option<u32>, StoreError> {
    match value {
        None => Ok(None),
        Some(value) => u32::try_from(value)
            .map(Some)
            .map_err(|_| StoreError::OutOfRange {
                field: "max_cards",
                value,
                max: u32::MAX,
            }),
    }
}

/// Write one ride's logo bytes to a unique temporary file and return its path.
fn materialize_logo(ride_id: i64, bytes: Option<&[u8]>) -> Result<Option<PathBuf>, StoreError> {
    let Some(bytes) = bytes.filter(|bytes| !bytes.is_empty()) else {
        return Ok(None);
    };
    let name = format!(
        "rivercrossing-ride-{ride_id}-{}.png",
        Uuid::new_v4().simple()
    );
    let path = std::env::temp_dir().join(name);
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)?;
    file.write_all(bytes)?;
    Ok(Some(path))
}
