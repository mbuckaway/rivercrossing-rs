// SPDX-License-Identifier: GPL-3.0-only

//! The frozen v1 to v2 and v2 to v3 migrations.
//!
//! A step is frozen history: it describes one jump between two versions once,
//! for good, and is never edited to match a later DDL. The chain covers every
//! version below [`crate::schema::SCHEMA_VERSION`].

use std::collections::{HashMap, HashSet};

use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, params};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::error::StoreError;

#[cfg(test)]
mod tests;

/// One migration step: it upgrades a file from version N to version N + 1.
pub type Migration = fn(&mut Connection) -> Result<(), StoreError>;

/// The chain: source version to the step that upgrades it to source + 1.
pub const MIGRATIONS: &[(u32, Migration)] = &[(1, migrate_v1_to_v2), (2, migrate_v2_to_v3)];

/// Upgrade a database from *from* to *to*, one step at a time.
///
/// Each step runs in its own transaction, and the ledger is stamped once,
/// after the last step commits, so the recorded version only ever names a
/// chain that finished. A `from` equal to `to` stamps the target as a no-op.
///
/// # Errors
///
/// Return [`StoreError::UnknownMigration`] when a version names no step —
/// version 0 is an empty file, which `ensure_schema` creates rather than
/// migrates — and the failing step's own error otherwise.
pub fn run_migrations(conn: &mut Connection, from: u32, to: u32) -> Result<(), StoreError> {
    for version in from..to {
        let step = MIGRATIONS
            .iter()
            .find_map(|(registered, step)| (*registered == version).then_some(*step))
            .ok_or(StoreError::UnknownMigration { version })?;
        step(conn)?;
    }
    conn.execute(
        "INSERT OR REPLACE INTO schema_version (id, version) VALUES (1, ?1)",
        [to],
    )?;
    Ok(())
}

// -------------------------------------------------------------- v1 to v2

/// The v2 `entry` table, frozen: v1's table plus `key` and `retired`, and
/// without v1's inline `UNIQUE (ride_id, plate)`.
const ENTRY_V2_DDL: &str = r"
CREATE TABLE entry_new (
    id           INTEGER PRIMARY KEY,
    ride_id      INTEGER NOT NULL REFERENCES ride(id),
    plate        TEXT    NOT NULL,
    key          TEXT    NOT NULL,
    display_name TEXT    NOT NULL,
    type         TEXT    NOT NULL CHECK (type IN ('solo', 'team')),
    team_size    INTEGER NOT NULL,
    status       TEXT    NOT NULL CHECK (status IN ('active', 'dnf')),
    dnf_at       INTEGER,
    notes        TEXT,
    logo_card    TEXT,
    retired      INTEGER NOT NULL DEFAULT 0
)
";

/// The per-ride plate uniqueness for the live rows alone.
const ENTRY_PLATE_LIVE_INDEX_DDL: &str =
    "CREATE UNIQUE INDEX entry_plate_live_unique ON entry (ride_id, plate) WHERE retired = 0";

/// Every v1 `entry` column, in the order the rebuild copies.
const V1_ENTRY_SELECT_SQL: &str = "SELECT id, ride_id, plate, display_name, type, \
     team_size, status, dnf_at, notes, logo_card FROM entry ORDER BY id";

/// The copy into the rebuilt table; each row mints its own key.
const ENTRY_V2_INSERT_SQL: &str = "INSERT INTO entry_new (id, ride_id, plate, key, \
     display_name, type, team_size, status, dnf_at, notes, logo_card, retired) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0)";

/// Rebuild `entry` in the v2 shape, minting one fresh key per row.
///
/// SQLite cannot drop v1's inline `UNIQUE (ride_id, plate)`, so this is the
/// table-rebuild procedure: create the v2 table under a temporary name, copy
/// every row (keeping its `id`, which the child tables reference), drop the
/// v1 table and rename the new one into place. Each key is a uuid4 in simple
/// hex, the form `Entry::key` mints, and every row starts live.
fn migrate_v1_to_v2(conn: &mut Connection) -> Result<(), StoreError> {
    // Foreign keys must be off before BEGIN: the pragma is ignored inside a
    // transaction, and with enforcement on `DROP TABLE entry` would clear the
    // child tables' rows.
    conn.pragma_update(None, "foreign_keys", false)?;
    let migrated = rebuild_entry(conn);
    let restored = conn.pragma_update(None, "foreign_keys", true);
    migrated?;
    restored?;
    Ok(())
}

/// Rebuild the `entry` table inside one transaction.
fn rebuild_entry(conn: &mut Connection) -> Result<(), StoreError> {
    let transaction = conn.transaction()?;
    transaction.execute(ENTRY_V2_DDL, [])?;
    {
        let mut select = transaction.prepare(V1_ENTRY_SELECT_SQL)?;
        let mut insert = transaction.prepare(ENTRY_V2_INSERT_SQL)?;
        let mut rows = select.query([])?;
        while let Some(row) = rows.next()? {
            insert.execute(params![
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                Uuid::new_v4().simple().to_string(),
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, Option<i64>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
            ])?;
        }
    }
    transaction.execute("DROP TABLE entry", [])?;
    transaction.execute("ALTER TABLE entry_new RENAME TO entry", [])?;
    transaction.execute(ENTRY_PLATE_LIVE_INDEX_DDL, [])?;
    let violations = foreign_key_violation_tables(&transaction)?;
    if !violations.is_empty() {
        return Err(StoreError::ForeignKeys { tables: violations });
    }
    transaction.commit()?;
    Ok(())
}

/// Return the distinct child tables that hold a foreign key violation,
/// sorted and joined, or an empty text when the check passes.
fn foreign_key_violation_tables(conn: &Connection) -> Result<String, StoreError> {
    let mut statement = conn.prepare("PRAGMA foreign_key_check")?;
    let mut rows = statement.query([])?;
    let mut tables: Vec<String> = Vec::new();
    while let Some(row) = rows.next()? {
        let table: String = row.get(0)?;
        if !tables.contains(&table) {
            tables.push(table);
        }
    }
    tables.sort();
    Ok(tables.join(", "))
}

// -------------------------------------------------------------- v2 to v3

/// The v2 `entry` identity columns that the rewrite resolves against.
const V2_ENTRY_IDENTITY_SQL: &str = "SELECT ride_id, plate, key, retired FROM entry";

/// The `audit` rows that the rewrite may change.
const V2_AUDIT_IDENTITY_SQL: &str = "SELECT id, ride_id, payload_json FROM audit";

/// The one-column update for a payload that the step repaired.
const V2_AUDIT_IDENTITY_UPDATE_SQL: &str = "UPDATE audit SET payload_json = ?1 WHERE id = ?2";

/// The payload fields that name an entry: a crossing's subject and a pooled
/// move's two sides.
const ENTRY_IDENTITY_FIELDS: [&str; 3] = ["entry_id", "old_entry_id", "new_entry_id"];

/// The plate-to-key lookup of one v2 file.
struct EntryIdentity {
    /// One key per live `(ride_id, plate)` pair.
    plate_to_key: HashMap<(i64, String), String>,
    /// Every key the file holds, live or retired.
    keys: HashSet<String>,
}

/// One stored audit row whose payload the rewrite may repair.
struct StoredAudit {
    id: i64,
    ride_id: i64,
    payload: String,
}

/// Rewrite legacy plated entry identities in `audit` as keys.
///
/// A data-only step: v2 rebuilt `entry` around the stable `key` but left the
/// `audit` payloads alone, so every event a v1 build recorded still names its
/// entry by the plate the operator typed. The plate-to-key map is built from
/// the live rows alone, because a retired row may hold the plate that the
/// destination team of a pooled move has since adopted. A value that is
/// already a key is skipped, which makes the step a no-op the second time it
/// runs.
fn migrate_v2_to_v3(conn: &mut Connection) -> Result<(), StoreError> {
    let identities = read_identities(conn)?;
    let stored = read_audit_payloads(conn)?;
    let transaction = conn.transaction()?;
    for audit in stored {
        let Some(mut object) = payload_object(&audit.payload) else {
            continue;
        };
        if !rewrite_entry_identities(&mut object, audit.ride_id, &identities) {
            continue;
        }
        let encoded = serde_json::to_string(&object)?;
        transaction.execute(V2_AUDIT_IDENTITY_UPDATE_SQL, params![encoded, audit.id])?;
    }
    transaction.commit()?;
    Ok(())
}

/// Read every entry's key, and the live plate-to-key pairs.
fn read_identities(conn: &Connection) -> Result<EntryIdentity, StoreError> {
    let mut plate_to_key = HashMap::new();
    let mut keys = HashSet::new();
    let mut statement = conn.prepare(V2_ENTRY_IDENTITY_SQL)?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let ride_id: i64 = row.get(0)?;
        let plate: String = row.get(1)?;
        let key: String = row.get(2)?;
        let retired: i64 = row.get(3)?;
        keys.insert(key.clone());
        if retired == 0 {
            plate_to_key.insert((ride_id, plate), key);
        }
    }
    Ok(EntryIdentity { plate_to_key, keys })
}

/// Read every audit row whose payload is text.
///
/// A payload that is not text is left exactly as it is: the replay step
/// already reports it cleanly, and one bad row must not strand the file.
fn read_audit_payloads(conn: &Connection) -> Result<Vec<StoredAudit>, StoreError> {
    let mut statement = conn.prepare(V2_AUDIT_IDENTITY_SQL)?;
    let mut rows = statement.query([])?;
    let mut stored = Vec::new();
    while let Some(row) = rows.next()? {
        let SqlValue::Text(payload) = row.get::<_, SqlValue>(2)? else {
            continue;
        };
        stored.push(StoredAudit {
            id: row.get(0)?,
            ride_id: row.get(1)?,
            payload,
        });
    }
    Ok(stored)
}

/// Return one stored payload as a JSON object, or `None` when it is not one.
fn payload_object(payload: &str) -> Option<Map<String, Value>> {
    match serde_json::from_str::<Value>(payload) {
        Ok(Value::Object(object)) => Some(object),
        _ => None,
    }
}

/// Rewrite *payload*'s plated entry identities in place.
///
/// Return whether anything changed, so the caller writes back only the rows
/// this step repaired.
fn rewrite_entry_identities(
    payload: &mut Map<String, Value>,
    ride_id: i64,
    identities: &EntryIdentity,
) -> bool {
    let mut changed = false;
    for field in ENTRY_IDENTITY_FIELDS {
        let Some(replacement) = identity_key(payload.get(field), ride_id, identities) else {
            continue;
        };
        payload.insert(field.to_owned(), Value::String(replacement));
        changed = true;
    }
    changed
}

/// Return the key that one identity value names, or `None` when the value is
/// not text, is already a key, or names no live plate in this ride.
fn identity_key(value: Option<&Value>, ride_id: i64, identities: &EntryIdentity) -> Option<String> {
    let Some(Value::String(text)) = value else {
        return None;
    };
    if identities.keys.contains(text) {
        return None;
    }
    identities
        .plate_to_key
        .get(&(ride_id, text.clone()))
        .cloned()
}
