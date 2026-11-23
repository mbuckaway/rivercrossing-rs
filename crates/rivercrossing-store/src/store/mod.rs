// SPDX-License-Identifier: GPL-3.0-only

//! The `Store` over the one database connection.
//!
//! The store is synchronous and owns one connection. It opens the file with
//! the spec PRAGMAs, ensures the schema, records one `app_session` row per
//! launch, and exposes the ride library, the config writes, the delete guard
//! and the session reading.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rivercrossing_cards::JokersMode;
use rivercrossing_ride::{Clock, Event, RideConfig, RideEngine, RideStatus, SystemClock};
use rivercrossing_roster::roster::Roster;
use rivercrossing_roster::types::{AuditEvent, Entry, Rider};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use uuid::Uuid;

use crate::backup::{self, DEFAULT_KEEP};
use crate::engine_load;
use crate::error::StoreError;
use crate::row::{
    AuditRow, RideRow, event_date_from_str, instant_from_epoch, live_entries_from_i64,
    ride_status_from_str,
};
use crate::session::{PreviousSession, SessionRow, SessionState, previous_session_from_row};

#[cfg(test)]
mod tests;

/// The number of times an open attempt is retried.
const OPEN_RETRY_ATTEMPTS: u64 = 5;

/// The base retry delay in milliseconds.
const OPEN_RETRY_DELAY_MS: u64 = 50;

/// The insert that writes every setup column of a new ride.
const INSERT_RIDE_SQL: &str = "
    INSERT INTO ride (
        name, event_date, venue, course_name, lap_km, organizer, scorer,
        logo_png, planned_start, planned_duration_s, actual_start, finished_at,
        status, entry_mode, max_team_size, plate_model, min_lap_s, deck_count,
        jokers_per_deck, jokers_mode, max_cards, tiebreak_order, rng_seed,
        created_at, updated_at, hold_short_laps
    ) VALUES (
        ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, 'draft', ?, ?, ?, ?, ?, ?, ?,
        ?, ?, ?, ?, ?, ?
    )
";

/// The update that rewrites every setup column but the logo.
const UPDATE_RIDE_SQL: &str = "
    UPDATE ride SET
        name = ?, event_date = ?, venue = ?, course_name = ?, lap_km = ?,
        organizer = ?, scorer = ?, planned_start = ?, planned_duration_s = ?,
        entry_mode = ?, max_team_size = ?, plate_model = ?, min_lap_s = ?,
        deck_count = ?, jokers_per_deck = ?, jokers_mode = ?, max_cards = ?,
        tiebreak_order = ?, hold_short_laps = ?, updated_at = ?
    WHERE id = ?
";

/// The copy that writes a duplicate ride row from its source.
const DUPLICATE_RIDE_SQL: &str = "
    INSERT INTO ride (
        name, event_date, venue, course_name, lap_km, organizer, scorer,
        logo_png, planned_start, planned_duration_s, actual_start, finished_at,
        status, entry_mode, max_team_size, plate_model, min_lap_s, deck_count,
        jokers_per_deck, jokers_mode, max_cards, tiebreak_order, rng_seed,
        created_at, updated_at, hold_short_laps
    )
    SELECT
        ?1, event_date, venue, course_name, lap_km, organizer, scorer,
        logo_png, planned_start, planned_duration_s, NULL, NULL, 'draft',
        entry_mode, max_team_size, plate_model, min_lap_s, deck_count,
        jokers_per_deck, jokers_mode, max_cards, tiebreak_order, ?2,
        ?3, ?3, hold_short_laps
    FROM ride WHERE id = ?4
";

/// The insert that writes one copied entry row.
const INSERT_ENTRY_SQL: &str = "
    INSERT INTO entry (
        ride_id, plate, key, display_name, type, team_size, status, dnf_at,
        notes, logo_card, retired
    ) VALUES (?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?)
";

/// The copy that writes one entry's riders onto the copied entry.
const INSERT_RIDERS_SQL: &str = "
    INSERT INTO rider (
        entry_id, first_name, last_name, plate, sort_order, sex,
        emergency_contact, waiver_signed, ccn_reg_id
    )
    SELECT ?1, first_name, last_name, plate, sort_order, sex, NULL, NULL, NULL
    FROM rider WHERE entry_id = ?2 ORDER BY sort_order, id
";

/// The query that reads every ride summary.
const RIDES_SQL: &str = "
    SELECT r.id, r.name, r.event_date, r.status,
        (SELECT COUNT(*) FROM entry e WHERE e.ride_id = r.id AND e.retired = 0)
    FROM ride r ORDER BY r.created_at, r.id
";

/// The query that reads the previous session's row.
const PREVIOUS_SESSION_SQL: &str = "
    SELECT closed_at, active_ride_id, heartbeat_at, opened_at
    FROM app_session ORDER BY id DESC LIMIT 1 OFFSET 1
";

/// The predicate that names the newest session row.
const NEWEST_SESSION: &str = "id = (SELECT id FROM app_session ORDER BY id DESC LIMIT 1)";

/// One ride over one SQLite database file.
///
/// Create it with [`Store::open`], which creates the parent directories,
/// opens the file, applies the PRAGMAs, ensures the schema and records the
/// launch session.
#[derive(Debug)]
pub struct Store {
    conn: Connection,
    path: PathBuf,
}

impl Store {
    /// Open (or create) the database at *path* and record the launch session.
    ///
    /// A transient open failure is retried up to five times, with a delay of
    /// 50 ms times the attempt number. A persistent failure, such as a schema
    /// stamped newer than this build, surfaces on the first attempt.
    ///
    /// # Errors
    ///
    /// Return [`StoreError::Io`] when the parent directory or the file cannot
    /// be opened, a schema or migration failure from
    /// [`crate::schema::ensure_schema`], and [`StoreError::Sqlite`] when the
    /// session row cannot be inserted.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        let mut attempt = 0_u64;
        loop {
            let error = match Self::open_once(&path) {
                Ok(store) => return Ok(store),
                Err(error) => error,
            };
            if !should_retry(&error, attempt) {
                return Err(error);
            }
            std::thread::sleep(std::time::Duration::from_millis(
                OPEN_RETRY_DELAY_MS * (attempt + 1),
            ));
            attempt += 1;
        }
    }

    /// Open the connection and record one launch session.
    fn open_once(path: &Path) -> Result<Self, StoreError> {
        let mut conn = Connection::open(path)?;
        crate::schema::apply_pragmas(&conn)?;
        crate::schema::ensure_schema(&mut conn)?;
        let opened_at = SystemClock.now().unix_timestamp();
        conn.execute(
            "INSERT INTO app_session (opened_at, closed_at, active_ride_id, heartbeat_at) \
             VALUES (?1, NULL, NULL, NULL)",
            [opened_at],
        )?;
        Ok(Self {
            conn,
            path: path.to_path_buf(),
        })
    }

    /// Return every ride summary, oldest first by creation.
    ///
    /// The entry count is the ride's live entries alone; a retired row is a
    /// key that replay still resolves, not an entry on the field.
    ///
    /// # Errors
    ///
    /// Return [`StoreError::InvalidRow`] when a stored date or status is not
    /// a known value, [`StoreError::OutOfRange`] when an entry count does not
    /// fit `u32`, and [`StoreError::Sqlite`] when the query fails.
    pub fn rides(&self) -> Result<Vec<RideRow>, StoreError> {
        let mut statement = self.conn.prepare(RIDES_SQL)?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?;
        let mut rides = Vec::new();
        for row in rows {
            let (id, name, event_date, status, entries) = row?;
            rides.push(RideRow {
                id,
                name,
                event_date: event_date_from_str(&event_date)?,
                status: ride_status_from_str(&status)?,
                entries: live_entries_from_i64(entries)?,
            });
        }
        Ok(rides)
    }

    /// Persist one ride from *config* and return its id.
    ///
    /// The logo bytes are read before the insert, so a missing logo file
    /// writes no row. An absent *seed* draws a 63-bit random seed, which the
    /// store owns.
    ///
    /// # Errors
    ///
    /// Return [`StoreError::Io`] when the logo file cannot be read, and
    /// [`StoreError::Sqlite`] or [`StoreError::Json`] when the insert or the
    /// tiebreak encoding fails.
    pub fn create_ride(
        &mut self,
        config: &RideConfig,
        seed: Option<u64>,
    ) -> Result<i64, StoreError> {
        let logo = read_logo(config.logo_path.as_deref())?;
        let event_date = config.event_date.to_string();
        let tiebreak = serde_json::to_string(&config.tiebreak_order)?;
        let seed = seed_to_sql(seed.unwrap_or_else(random_63_bit))?;
        let now = SystemClock.now().unix_timestamp();
        let transaction = self.conn.transaction()?;
        transaction.execute(
            INSERT_RIDE_SQL,
            params![
                config.name,
                event_date,
                config.venue,
                config.venue,
                config.lap_km,
                config.organizer,
                config.scorer,
                logo,
                config.planned_start.unix_timestamp(),
                config.planned_duration_s,
                config.entry_mode.as_str(),
                config.max_team_size,
                config.plate_model.as_str(),
                config.min_lap_s,
                config.deck_count,
                config.jokers_per_deck,
                jokers_mode_str(config.jokers_mode),
                config.max_cards,
                tiebreak,
                seed,
                now,
                now,
                config.hold_short_laps,
            ],
        )?;
        let ride_id = transaction.last_insert_rowid();
        transaction.commit()?;
        Ok(ride_id)
    }

    /// Rewrite one ride's setup columns from *config*.
    ///
    /// The lifecycle columns, the ride's seed and its event log are never
    /// touched. The logo is replaced only when *config* names a file; an edit
    /// that picks no logo keeps the stored one. A missing logo file rolls the
    /// whole update back.
    ///
    /// # Errors
    ///
    /// Return [`StoreError::RideNotFound`] when no ride holds `ride_id`,
    /// [`StoreError::Io`] when the logo file cannot be read, and a SQLite or
    /// JSON failure when a write fails.
    pub fn update_ride_config(
        &mut self,
        ride_id: i64,
        config: &RideConfig,
    ) -> Result<(), StoreError> {
        if !self.ride_exists(ride_id)? {
            return Err(StoreError::RideNotFound { id: ride_id });
        }
        let event_date = config.event_date.to_string();
        let tiebreak = serde_json::to_string(&config.tiebreak_order)?;
        let now = SystemClock.now().unix_timestamp();
        let transaction = self.conn.transaction()?;
        transaction.execute(
            UPDATE_RIDE_SQL,
            params![
                config.name,
                event_date,
                config.venue,
                config.venue,
                config.lap_km,
                config.organizer,
                config.scorer,
                config.planned_start.unix_timestamp(),
                config.planned_duration_s,
                config.entry_mode.as_str(),
                config.max_team_size,
                config.plate_model.as_str(),
                config.min_lap_s,
                config.deck_count,
                config.jokers_per_deck,
                jokers_mode_str(config.jokers_mode),
                config.max_cards,
                tiebreak,
                config.hold_short_laps,
                now,
                ride_id,
            ],
        )?;
        if let Some(logo_path) = &config.logo_path {
            let bytes = std::fs::read(logo_path)?;
            transaction.execute(
                "UPDATE ride SET logo_png = ?1 WHERE id = ?2",
                params![bytes, ride_id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Copy one ride's setup and roster to a new DRAFT ride and return its id.
    ///
    /// The copy gets a fresh seed and fresh entry keys, no timing columns, and
    /// no crossing, card or audit rows. A retired entry copies too, so its key
    /// history survives.
    ///
    /// # Errors
    ///
    /// Return [`StoreError::RideNotFound`] when no ride holds `ride_id`, and a
    /// SQLite failure when a copy fails.
    pub fn duplicate_ride(&mut self, ride_id: i64, name: Option<&str>) -> Result<i64, StoreError> {
        let source_name = self.ride_name(ride_id)?;
        let copy_name = name.map_or_else(|| format!("{source_name} (copy)"), ToOwned::to_owned);
        let plan = CopyPlan {
            source_id: ride_id,
            copy_name,
            seed: seed_to_sql(random_63_bit())?,
            now: SystemClock.now().unix_timestamp(),
        };
        let transaction = self.conn.transaction()?;
        let new_id = insert_duplicate(&transaction, &plan)?;
        copy_roster_rows(&transaction, ride_id, new_id)?;
        transaction.commit()?;
        Ok(new_id)
    }

    /// Delete one ride and all its data, after an automatic backup.
    ///
    /// The guard runs first: the ride must exist, `typed_name` must equal the
    /// stored name byte for byte, and the ride must not be RUNNING. Only then
    /// does the backup run, and only after a successful backup does the one
    /// transaction remove the dependents and the ride row.
    ///
    /// # Errors
    ///
    /// Return [`StoreError::RideNotFound`], [`StoreError::RideNameMismatch`],
    /// [`StoreError::RideRunning`] for the guard, and the backup's own error
    /// when it fails. A guard refusal writes no backup and deletes nothing.
    pub fn delete_ride(&mut self, ride_id: i64, typed_name: &str) -> Result<(), StoreError> {
        let (name, status) = self.ride_name_and_status(ride_id)?;
        if typed_name != name.as_str() {
            return Err(StoreError::RideNameMismatch {
                id: ride_id,
                typed: typed_name.to_owned(),
                name,
            });
        }
        if ride_status_from_str(&status)? == RideStatus::Running {
            return Err(StoreError::RideRunning { id: ride_id });
        }
        backup::run(&self.path, DEFAULT_KEEP, &SystemClock)?;
        let transaction = self.conn.transaction()?;
        transaction.execute("DELETE FROM card WHERE ride_id = ?1", [ride_id])?;
        transaction.execute("DELETE FROM crossing WHERE ride_id = ?1", [ride_id])?;
        transaction.execute(
            "DELETE FROM rider WHERE entry_id IN (SELECT id FROM entry WHERE ride_id = ?1)",
            [ride_id],
        )?;
        transaction.execute("DELETE FROM entry WHERE ride_id = ?1", [ride_id])?;
        transaction.execute("DELETE FROM audit WHERE ride_id = ?1", [ride_id])?;
        transaction.execute(
            "UPDATE app_session SET active_ride_id = NULL WHERE active_ride_id = ?1",
            [ride_id],
        )?;
        transaction.execute("DELETE FROM ride WHERE id = ?1", [ride_id])?;
        transaction.commit()?;
        Ok(())
    }

    /// Return one ride's roster rebuilt from its entry and rider rows.
    ///
    /// The ride's own shape columns build the roster's settings, each entry's
    /// stored key survives, and a live entry's `has_data` is derived from its
    /// recorded rows. A retired entry rebuilds into the retired list with no
    /// riders and `has_data` set, because the retired flag is that fact.
    ///
    /// # Errors
    ///
    /// Return [`StoreError::RideNotFound`] when no ride holds `ride_id`,
    /// [`StoreError::InvalidRow`] for an unreadable stored value,
    /// [`StoreError::OutOfRange`] for a count that does not fit, and
    /// [`StoreError::InvalidRoster`] when the ride's shape is refused.
    pub fn roster_for(&self, ride_id: i64) -> Result<Roster, StoreError> {
        engine_load::load_roster(&self.conn, ride_id)
    }

    /// Replace one ride's persisted roster with *roster*.
    ///
    /// A save is a snapshot, never an append: the ride's previous entry and
    /// rider rows are removed first, then the live entries are written in list
    /// order, then the retired ones. Each entry's key is written verbatim and
    /// each rider's `sort_order` comes from the rider, so a reload reproduces
    /// the same roster.
    ///
    /// # Errors
    ///
    /// Return [`StoreError::RideNotFound`] when no ride holds `ride_id`, and
    /// [`StoreError::Sqlite`] when a write fails, including the live-plate
    /// partial index's refusal of two live entries on one plate.
    pub fn save_roster(&mut self, ride_id: i64, roster: &Roster) -> Result<(), StoreError> {
        if !self.ride_exists(ride_id)? {
            return Err(StoreError::RideNotFound { id: ride_id });
        }
        let transaction = self.conn.transaction()?;
        transaction.execute(
            "DELETE FROM rider WHERE entry_id IN (SELECT id FROM entry WHERE ride_id = ?1)",
            [ride_id],
        )?;
        transaction.execute("DELETE FROM entry WHERE ride_id = ?1", [ride_id])?;
        for entry in roster.entries() {
            insert_roster_entry(&transaction, ride_id, entry, false)?;
        }
        for entry in roster.retired_entries() {
            insert_roster_entry(&transaction, ride_id, entry, true)?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Persist one ride event as one audit row and sync the ride lifecycle.
    ///
    /// The stored `at` is the payload's own instant when it carries one of the
    /// event keys, else the clock. `start` and `continue` write RUNNING,
    /// `finish` FINISHED and `reopen` REOPENED, each also stamping
    /// `updated_at`. Every other action, `stop` and `record_crossing`
    /// included, appends the audit row alone: the timing columns stay NULL and
    /// the event log is authoritative.
    ///
    /// # Errors
    ///
    /// Return [`StoreError::Sqlite`] when the insert fails, including the
    /// foreign-key refusal of a ride that does not exist, and
    /// [`StoreError::Json`] when the payload or action cannot be encoded.
    pub fn append(&mut self, ride_id: i64, event: &Event) -> Result<(), StoreError> {
        let action = engine_load::action_text(event.action)?;
        let payload_json = serde_json::to_string(&event.payload)?;
        let now = SystemClock.now();
        let at = engine_load::event_timestamp(&event.payload).unwrap_or(now);
        let transaction = self.conn.transaction()?;
        transaction.execute(
            "INSERT INTO audit (ride_id, at, action, payload_json) VALUES (?1, ?2, ?3, ?4)",
            params![ride_id, at.unix_timestamp(), action, payload_json],
        )?;
        if let Some(status) = engine_load::lifecycle_status(event.action) {
            transaction.execute(
                "UPDATE ride SET status = ?1, updated_at = ?2 WHERE id = ?3",
                params![status.as_str(), now.unix_timestamp(), ride_id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Persist one roster audit event as one audit row.
    ///
    /// A roster event is never a ride mutation, so the ride's status and
    /// `updated_at` stay exactly as they were. The stored `at` is the append
    /// time.
    ///
    /// # Errors
    ///
    /// Return [`StoreError::RideNotFound`] when no ride holds `ride_id`,
    /// [`StoreError::Json`] when the payload cannot be encoded, and
    /// [`StoreError::Sqlite`] when the insert fails.
    pub fn append_roster_event(
        &mut self,
        ride_id: i64,
        event: &AuditEvent,
    ) -> Result<(), StoreError> {
        if !self.ride_exists(ride_id)? {
            return Err(StoreError::RideNotFound { id: ride_id });
        }
        let payload_json = serde_json::to_string(&event.payload)?;
        let now = SystemClock.now().unix_timestamp();
        self.conn.execute(
            "INSERT INTO audit (ride_id, at, action, payload_json) VALUES (?1, ?2, ?3, ?4)",
            params![ride_id, now, event.action, payload_json],
        )?;
        Ok(())
    }

    /// Rebuild one ride's engine by replaying its audit log.
    ///
    /// The config comes from the ride row, the shoe from the stored seed, and
    /// the roster from the entry and rider rows. Every audit row whose action
    /// is a ride action is applied in append order; a row whose action text is
    /// not a ride action is skipped, so a roster plate change never reaches
    /// the engine.
    ///
    /// # Errors
    ///
    /// Return [`StoreError::RideNotFound`] when no ride holds `ride_id`, the
    /// config, shoe, roster or payload conversion failures, and
    /// [`StoreError::Replay`] when a known action's payload does not decode or
    /// the engine refuses it.
    pub fn load_engine(&self, ride_id: i64) -> Result<RideEngine<Roster, SystemClock>, StoreError> {
        engine_load::load_engine(&self.conn, ride_id)
    }

    /// Return one ride's audit trail, newest first.
    ///
    /// `entry` is the payload's `dnf` display, else its `tiebreak_draw`
    /// summary, else its `entry_id` key mapped to the entry's plate (or the
    /// raw key), else a plate or name column, else an empty string. `reason`
    /// is the payload's `reason` text or an empty string.
    ///
    /// # Errors
    ///
    /// Return [`StoreError::RideNotFound`] when no ride holds `ride_id`,
    /// [`StoreError::InvalidPayload`] when a row's payload does not decode to
    /// a JSON object, [`StoreError::InvalidRow`] for an instant that does not
    /// convert, and [`StoreError::Sqlite`] when a query fails.
    pub fn audit_rows(&self, ride_id: i64) -> Result<Vec<AuditRow>, StoreError> {
        if !self.ride_exists(ride_id)? {
            return Err(StoreError::RideNotFound { id: ride_id });
        }
        let plates = live_plate_map(&self.conn, ride_id)?;
        let stored = read_audit_rows(&self.conn, ride_id)?;
        let mut audit = Vec::new();
        for row in stored {
            audit.push(project_audit_row(row, &plates)?);
        }
        Ok(audit)
    }

    /// Return the previous session's resume record.
    ///
    /// The reading is the second-newest row, the session that this launch's
    /// row supersedes. A fresh database reads as a clean quit with no ride.
    ///
    /// # Errors
    ///
    /// Return [`StoreError::InvalidRow`] when a stored epoch does not convert
    /// to an instant, and [`StoreError::Sqlite`] when the query fails.
    pub fn previous_session(&self) -> Result<PreviousSession, StoreError> {
        let stored = self
            .conn
            .query_row(PREVIOUS_SESSION_SQL, [], |row| {
                Ok(SessionRow {
                    closed_at: row.get(0)?,
                    active_ride_id: row.get(1)?,
                    heartbeat_at: row.get(2)?,
                    opened_at: row.get(3)?,
                })
            })
            .optional()?;
        match stored {
            None => Ok(PreviousSession::fresh()),
            Some(row) => previous_session_from_row(row),
        }
    }

    /// Return the previous session's state.
    ///
    /// # Errors
    ///
    /// Return the same failures as [`Store::previous_session`].
    pub fn session_state(&self) -> Result<SessionState, StoreError> {
        Ok(self.previous_session()?.state)
    }

    /// Stamp the open session's `closed_at`, so a clean quit is distinct from
    /// a crash. With no session row this is a no-op.
    ///
    /// # Errors
    ///
    /// Return [`StoreError::Sqlite`] when the update fails.
    pub fn close_session(&mut self) -> Result<(), StoreError> {
        let now = SystemClock.now().unix_timestamp();
        self.conn.execute(
            &format!("UPDATE app_session SET closed_at = ?1 WHERE {NEWEST_SESSION}"),
            [now],
        )?;
        Ok(())
    }

    /// Record the running ride on the open session. With no session row this
    /// is a no-op.
    ///
    /// # Errors
    ///
    /// Return [`StoreError::Sqlite`] when the update fails, including a
    /// foreign-key rejection when `ride_id` names no ride.
    pub fn set_active_ride(&mut self, ride_id: i64) -> Result<(), StoreError> {
        self.conn.execute(
            &format!("UPDATE app_session SET active_ride_id = ?1 WHERE {NEWEST_SESSION}"),
            [ride_id],
        )?;
        Ok(())
    }

    /// Clear the open session's running ride. With no session row this is a
    /// no-op.
    ///
    /// # Errors
    ///
    /// Return [`StoreError::Sqlite`] when the update fails.
    pub fn clear_active_ride(&mut self) -> Result<(), StoreError> {
        self.conn.execute(
            &format!("UPDATE app_session SET active_ride_id = NULL WHERE {NEWEST_SESSION}"),
            [],
        )?;
        Ok(())
    }

    /// Write a manual backup of the open database now and return its path.
    ///
    /// # Errors
    ///
    /// Return the backup's own failures: a busy checkpoint, a missing
    /// database file or a file-system failure.
    pub fn backup_now(&self) -> Result<PathBuf, StoreError> {
        backup::run(&self.path, DEFAULT_KEEP, &SystemClock)
    }

    /// Return whether a ride row holds `ride_id`.
    fn ride_exists(&self, ride_id: i64) -> Result<bool, StoreError> {
        let found = self
            .conn
            .query_row("SELECT id FROM ride WHERE id = ?1", [ride_id], |row| {
                row.get::<_, i64>(0)
            })
            .optional()?;
        Ok(found.is_some())
    }

    /// Return the stored name of `ride_id`, or the not-found error.
    fn ride_name(&self, ride_id: i64) -> Result<String, StoreError> {
        self.conn
            .query_row("SELECT name FROM ride WHERE id = ?1", [ride_id], |row| {
                row.get::<_, String>(0)
            })
            .optional()?
            .ok_or(StoreError::RideNotFound { id: ride_id })
    }

    /// Return the stored name and status of `ride_id`, or the not-found error.
    fn ride_name_and_status(&self, ride_id: i64) -> Result<(String, String), StoreError> {
        self.conn
            .query_row(
                "SELECT name, status FROM ride WHERE id = ?1",
                [ride_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or(StoreError::RideNotFound { id: ride_id })
    }
}

/// Return whether a failed open may be retried on the next attempt.
fn should_retry(error: &StoreError, attempt: u64) -> bool {
    attempt + 1 < OPEN_RETRY_ATTEMPTS && crate::schema::is_transient_open_error(&error.to_string())
}

/// The values a duplicate needs beyond the source row.
struct CopyPlan {
    /// The source ride's id.
    source_id: i64,
    /// The copy's name.
    copy_name: String,
    /// The fresh seed already converted for SQLite.
    seed: i64,
    /// The creation instant.
    now: i64,
}

/// One source entry row, read before the copy is written.
struct SourceEntry {
    /// The source entry's id.
    id: i64,
    /// The plate.
    plate: String,
    /// The display name.
    display_name: String,
    /// The stored entry type spelling.
    entry_type: String,
    /// The stored team size.
    team_size: i64,
    /// The stored status spelling.
    status: String,
    /// The entry's notes.
    notes: Option<String>,
    /// The entry's logo card.
    logo_card: Option<String>,
    /// The stored retired flag.
    retired: i64,
}

/// Insert the duplicate ride row and return its id.
fn insert_duplicate(conn: &Connection, plan: &CopyPlan) -> Result<i64, StoreError> {
    conn.execute(
        DUPLICATE_RIDE_SQL,
        params![plan.copy_name, plan.seed, plan.now, plan.source_id],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Copy `source_id`'s entries and riders onto `new_id`, with fresh keys.
fn copy_roster_rows(conn: &Connection, source_id: i64, new_id: i64) -> Result<(), StoreError> {
    for entry in source_entries(conn, source_id)? {
        conn.execute(
            INSERT_ENTRY_SQL,
            params![
                new_id,
                entry.plate,
                Uuid::new_v4().simple().to_string(),
                entry.display_name,
                entry.entry_type,
                entry.team_size,
                entry.status,
                entry.notes,
                entry.logo_card,
                entry.retired,
            ],
        )?;
        let new_entry_id = conn.last_insert_rowid();
        conn.execute(INSERT_RIDERS_SQL, params![new_entry_id, entry.id])?;
    }
    Ok(())
}

/// Return every entry row of `ride_id`, in creation order.
fn source_entries(conn: &Connection, ride_id: i64) -> Result<Vec<SourceEntry>, StoreError> {
    let mut statement = conn.prepare(
        "SELECT id, plate, display_name, type, team_size, status, notes, logo_card, retired \
         FROM entry WHERE ride_id = ?1 ORDER BY id",
    )?;
    let rows = statement.query_map([ride_id], |row| {
        Ok(SourceEntry {
            id: row.get(0)?,
            plate: row.get(1)?,
            display_name: row.get(2)?,
            entry_type: row.get(3)?,
            team_size: row.get(4)?,
            status: row.get(5)?,
            notes: row.get(6)?,
            logo_card: row.get(7)?,
            retired: row.get(8)?,
        })
    })?;
    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    Ok(entries)
}

/// Read the logo bytes that *path* names, when a logo is configured.
///
/// # Errors
///
/// Return [`StoreError::Io`] when the file cannot be read.
fn read_logo(path: Option<&Path>) -> Result<Option<Vec<u8>>, StoreError> {
    match path {
        Some(path) => Ok(Some(std::fs::read(path)?)),
        None => Ok(None),
    }
}

/// Return the stored spelling of a jokers mode.
fn jokers_mode_str(mode: JokersMode) -> &'static str {
    match mode {
        JokersMode::PerDeck => "per_deck",
        JokersMode::Total => "total",
    }
}

/// Convert a seed to the signed integer that SQLite stores.
///
/// A supplied seed above `i64::MAX` does not fit the column, so it is refused
/// rather than wrapped.
fn seed_to_sql(seed: u64) -> Result<i64, StoreError> {
    i64::try_from(seed).map_err(|_| StoreError::InvalidRow {
        table: "ride",
        column: "rng_seed",
        stored: seed.to_string(),
    })
}

/// Return a random 63-bit seed.
///
/// The draw is the low 63 bits of a version-4 UUID, so it needs no random
/// crate.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    reason = "the low 64 bits of the 128-bit UUID are masked to 63 bits"
)]
pub(crate) fn random_63_bit() -> u64 {
    Uuid::new_v4().as_u128() as u64 & ((1_u64 << 63) - 1)
}

/// One audit row as it is read, before it is projected for the viewer.
struct StoredAudit {
    /// The row's id, for the failure messages.
    row_id: i64,
    /// The stored epoch second.
    at: i64,
    /// The stored action spelling.
    action: String,
    /// The stored payload text.
    payload_json: String,
}

/// Write one entry row and its riders for a roster snapshot.
///
/// A retired entry is written with `retired = 1`, a team size of zero and no
/// rider rows: a dissolve only ever fires on an emptied entry.
///
/// # Errors
///
/// Return [`StoreError::Sqlite`] when a write fails, and
/// [`StoreError::OutOfRange`] in the unreachable case where the rider count
/// does not fit the column.
#[expect(
    clippy::too_many_arguments,
    reason = "the write needs the connection, the ride, the entry and its retired flag"
)]
fn insert_roster_entry(
    conn: &Connection,
    ride_id: i64,
    entry: &Entry,
    retired: bool,
) -> Result<(), StoreError> {
    let team_size = if retired {
        0
    } else {
        team_size_param(entry.riders.len())?
    };
    conn.execute(
        "INSERT INTO entry (ride_id, plate, key, display_name, type, team_size, status, dnf_at, \
         notes, logo_card, retired) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, ?9, ?10)",
        params![
            ride_id,
            entry.plate,
            entry.key,
            entry.display_name,
            entry.kind.as_str(),
            team_size,
            entry.status.as_str(),
            entry.notes,
            entry.logo_card,
            retired,
        ],
    )?;
    if retired {
        return Ok(());
    }
    let entry_id = conn.last_insert_rowid();
    for rider in &entry.riders {
        insert_roster_rider(conn, entry_id, rider)?;
    }
    Ok(())
}

/// Write one rider row for a roster snapshot.
///
/// The optional contact, waiver and registration columns stay NULL: the
/// in-memory model carries no such fields.
///
/// # Errors
///
/// Return [`StoreError::Sqlite`] when the insert fails.
fn insert_roster_rider(conn: &Connection, entry_id: i64, rider: &Rider) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO rider (entry_id, first_name, last_name, plate, sex, sort_order, \
         emergency_contact, waiver_signed, ccn_reg_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, \
         NULL, NULL)",
        params![
            entry_id,
            rider.first_name,
            rider.last_name,
            rider.plate,
            rider.sex,
            rider.sort_order
        ],
    )?;
    Ok(())
}

/// Convert a rider count for the `team_size` column.
///
/// The branch that refuses a count above `u32` is unreachable for a roster the
/// engine accepts; it keeps the write free of a truncating cast.
fn team_size_param(count: usize) -> Result<u32, StoreError> {
    u32::try_from(count).map_err(|_| StoreError::OutOfRange {
        field: "team_size",
        value: i64::MAX,
        max: u32::MAX,
    })
}

/// Read one ride's audit rows, newest first.
fn read_audit_rows(conn: &Connection, ride_id: i64) -> Result<Vec<StoredAudit>, StoreError> {
    let mut statement = conn.prepare(
        "SELECT id, at, action, payload_json FROM audit WHERE ride_id = ?1 ORDER BY id DESC",
    )?;
    let rows = statement.query_map([ride_id], |row| {
        Ok(StoredAudit {
            row_id: row.get(0)?,
            at: row.get(1)?,
            action: row.get(2)?,
            payload_json: row.get(3)?,
        })
    })?;
    let mut stored = Vec::new();
    for row in rows {
        stored.push(row?);
    }
    Ok(stored)
}

/// Project one stored audit row onto the viewer's columns.
///
/// # Errors
///
/// Return [`StoreError::InvalidPayload`] when the payload is not a JSON
/// object, and [`StoreError::InvalidRow`] when the stored instant does not
/// convert.
fn project_audit_row(
    stored: StoredAudit,
    plates: &HashMap<String, String>,
) -> Result<AuditRow, StoreError> {
    let payload = serde_json::from_str::<Value>(&stored.payload_json).map_err(|_| {
        StoreError::InvalidPayload {
            row_id: stored.row_id,
        }
    })?;
    if !payload.is_object() {
        return Err(StoreError::InvalidPayload {
            row_id: stored.row_id,
        });
    }
    let entry = audit_entry(&stored.action, &payload, plates);
    Ok(AuditRow {
        at: instant_from_epoch("audit", "at", stored.at)?,
        action: stored.action,
        entry,
        reason: payload
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
    })
}

/// Return one ride's live entry keys mapped to their plates.
fn live_plate_map(conn: &Connection, ride_id: i64) -> Result<HashMap<String, String>, StoreError> {
    let mut statement =
        conn.prepare("SELECT key, plate FROM entry WHERE ride_id = ?1 AND retired = 0")?;
    let rows = statement.query_map([ride_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut plates = HashMap::new();
    for row in rows {
        let (key, plate) = row?;
        plates.insert(key, plate);
    }
    Ok(plates)
}

/// Return one audit row's entry cell.
///
/// The precedence is the viewer's: a `dnf` row's carried display, a
/// `tiebreak_draw` row's summary, the `entry_id` key as its plate (or as
/// itself when the map does not know it), then a plate or name column, then an
/// empty string.
fn audit_entry(action: &str, payload: &Value, plates: &HashMap<String, String>) -> String {
    if action == "dnf"
        && let Some(display) = text_at(payload, "display")
    {
        return display;
    }
    if action == "tiebreak_draw"
        && let Some(summary) = text_at(payload, "summary")
    {
        return summary;
    }
    if let Some(key) = text_at(payload, "entry_id") {
        return plates.get(&key).cloned().unwrap_or(key);
    }
    for field in ["plate", "old_plate", "new_plate", "display_name"] {
        if let Some(text) = text_at(payload, field) {
            return text;
        }
    }
    String::new()
}

/// Return one payload value as non-empty text, if it carries any.
fn text_at(payload: &Value, key: &str) -> Option<String> {
    let text = match payload.get(key)? {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    if text.is_empty() { None } else { Some(text) }
}
