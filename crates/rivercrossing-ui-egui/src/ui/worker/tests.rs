// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the store worker: `apply` against a real temporary
//! database, and the thread handshake through the command and reply channels.

use std::time::{Duration, Instant};

use pretty_assertions::assert_eq;
use rivercrossing_ride::{
    EntryMode, Event, EventAction, PlateModel, RideConfig, RideConfigParams, RideEngine,
    RideStatus, SystemClock,
};
use rivercrossing_roster::roster::Roster;
use rivercrossing_roster::types::{AuditEvent, Rider};
use rivercrossing_store::session::SessionState;
use rivercrossing_store::{AuditRow, PreviousSession, RideRow, Store, StoreError};
use serde_json::json;
use tempfile::TempDir;

use super::{StoreCommand, StoreReply, StoreWorker, apply};

/// The ride name every fixture uses.
const RIDE_NAME: &str = "GORBA EPIC 2026";

/// The plate that the fixture's solo entry carries.
const PLATE: &str = "101";

/// The seed every fixture ride stores.
const SEED: u64 = 7;

/// The id that no ride holds.
const UNKNOWN_ID: i64 = 999;

/// The longest a thread handshake waits for a reply.
const REPLY_TIMEOUT: Duration = Duration::from_secs(5);

/// The pause between two drain polls.
const POLL_PAUSE: Duration = Duration::from_millis(2);

/// One reply as the worker's channel carries it.
type Reply = Result<StoreReply, String>;

// ------------------------------------------------------------- fixtures

/// Return the fixture ride config; its setup clears the start minimum.
fn fixture_config() -> Result<RideConfig, String> {
    RideConfig::new(RideConfigParams {
        name: RIDE_NAME.to_owned(),
        venue: "Sea to Sky Gondola".to_owned(),
        lap_km: 8.0,
        organizer: "GORBA".to_owned(),
        scorer: "K. Singh".to_owned(),
        entry_mode: EntryMode::Mixed,
        plate_model: PlateModel::RiderPooled,
        ..RideConfigParams::default()
    })
    .map_err(|error| error.to_string())
}

/// Return a roster that holds one solo entry on `PLATE`.
fn fixture_roster() -> Result<Roster, String> {
    let mut roster = Roster::new(EntryMode::Mixed, 4, PlateModel::RiderPooled, Some(SEED))
        .map_err(|error| error.to_string())?;
    roster
        .create_solo_entry(
            Rider {
                first_name: "Ada".to_owned(),
                last_name: "Lovelace".to_owned(),
                ..Rider::default()
            },
            PLATE,
        )
        .map_err(|error| error.to_string())?;
    Ok(roster)
}

/// Open a store over a fresh database, keeping the directory alive.
fn open_store() -> Result<(TempDir, Store), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let store =
        Store::open(directory.path().join("rides.db")).map_err(|error| error.to_string())?;
    Ok((directory, store))
}

/// Run one command and render its failure as text.
fn applied(store: &mut Store, command: StoreCommand) -> Result<StoreReply, String> {
    apply(store, command).map_err(|error| error.to_string())
}

// ------------------------------------------------------- reply readers

/// Return the created ride's id.
fn created_id(reply: Reply) -> Result<i64, String> {
    match reply {
        Ok(StoreReply::RideCreated(id)) => Ok(id),
        Ok(other) => Err(format!("expected RideCreated, got {other:?}")),
        Err(message) => Err(message),
    }
}

/// Return the ride library's rows.
fn ride_rows(reply: Reply) -> Result<Vec<RideRow>, String> {
    match reply {
        Ok(StoreReply::Rides(rows)) => Ok(rows),
        Ok(other) => Err(format!("expected Rides, got {other:?}")),
        Err(message) => Err(message),
    }
}

/// Return a loaded ride's parts.
/// A loaded ride's parts, as the `Ride` reply carries them.
#[derive(Debug)]
struct LoadedParts {
    /// The ride's id.
    id: i64,
    /// The ride's persisted setup.
    config: RideConfig,
    /// The roster that the ride's rows rebuild.
    roster: Box<Roster>,
    /// The engine rebuilt from the audit log.
    engine: Box<RideEngine<Roster, SystemClock>>,
}

/// Return a loaded ride's parts.
fn loaded_ride(reply: Reply) -> Result<LoadedParts, String> {
    match reply {
        Ok(StoreReply::Ride {
            id,
            config,
            roster,
            engine,
        }) => Ok(LoadedParts {
            id,
            config,
            roster,
            engine,
        }),
        Ok(other) => Err(format!("expected Ride, got {other:?}")),
        Err(message) => Err(message),
    }
}

/// Return the roster that a reply carries.
fn roster_reply(reply: Reply) -> Result<Box<Roster>, String> {
    match reply {
        Ok(StoreReply::Roster(roster)) => Ok(roster),
        Ok(other) => Err(format!("expected Roster, got {other:?}")),
        Err(message) => Err(message),
    }
}

/// Return the audit rows that a reply carries.
fn audit_rows_reply(reply: Reply) -> Result<Vec<AuditRow>, String> {
    match reply {
        Ok(StoreReply::AuditRows(rows)) => Ok(rows),
        Ok(other) => Err(format!("expected AuditRows, got {other:?}")),
        Err(message) => Err(message),
    }
}

/// Return the previous session that a reply carries.
fn previous_session_reply(reply: Reply) -> Result<PreviousSession, String> {
    match reply {
        Ok(StoreReply::PreviousSession(session)) => Ok(session),
        Ok(other) => Err(format!("expected PreviousSession, got {other:?}")),
        Err(message) => Err(message),
    }
}

/// Return the active ride that a reply carries.
fn active_ride_reply(reply: Reply) -> Result<Option<i64>, String> {
    match reply {
        Ok(StoreReply::ActiveRide(id)) => Ok(id),
        Ok(other) => Err(format!("expected ActiveRide, got {other:?}")),
        Err(message) => Err(message),
    }
}

/// Return the backup path that a reply carries.
fn backup_reply(reply: Reply) -> Result<std::path::PathBuf, String> {
    match reply {
        Ok(StoreReply::Backup(path)) => Ok(path),
        Ok(other) => Err(format!("expected Backup, got {other:?}")),
        Err(message) => Err(message),
    }
}

/// Check that a reply is exactly `RosterSaved`.
fn expect_roster_saved(reply: Reply) -> Result<(), String> {
    match reply {
        Ok(StoreReply::RosterSaved) => Ok(()),
        Ok(other) => Err(format!("expected RosterSaved, got {other:?}")),
        Err(message) => Err(message),
    }
}

/// Check that a reply is exactly `Appended`.
fn expect_appended(reply: Reply) -> Result<(), String> {
    match reply {
        Ok(StoreReply::Appended) => Ok(()),
        Ok(other) => Err(format!("expected Appended, got {other:?}")),
        Err(message) => Err(message),
    }
}

/// Check that a reply is exactly `RosterEventAppended`.
fn expect_roster_event_appended(reply: Reply) -> Result<(), String> {
    match reply {
        Ok(StoreReply::RosterEventAppended) => Ok(()),
        Ok(other) => Err(format!("expected RosterEventAppended, got {other:?}")),
        Err(message) => Err(message),
    }
}

/// Check that a reply is exactly `SessionClosed`.
fn expect_session_closed(reply: Reply) -> Result<(), String> {
    match reply {
        Ok(StoreReply::SessionClosed) => Ok(()),
        Ok(other) => Err(format!("expected SessionClosed, got {other:?}")),
        Err(message) => Err(message),
    }
}

/// Return the one reply that a drain produced.
fn only_reply(mut replies: Vec<Reply>) -> Result<StoreReply, String> {
    let count = replies.len();
    if count != 1 {
        return Err(format!("expected one reply, got {count}"));
    }
    replies.pop().ok_or_else(|| "no reply arrived".to_owned())?
}

/// Drain the worker until a reply arrives, with a bounded wait.
///
/// The wait is bounded, so a worker that never answers fails the test after
/// `REPLY_TIMEOUT` instead of hanging it.
fn poll_drain(worker: &StoreWorker) -> Vec<Reply> {
    let deadline = Instant::now() + REPLY_TIMEOUT;
    loop {
        let replies = worker.drain();
        if !replies.is_empty() || Instant::now() >= deadline {
            return replies;
        }
        std::thread::park_timeout(POLL_PAUSE);
    }
}

// ----------------------------------------------------- apply: the store

#[test]
fn test_apply_creates_and_loads_a_ride() -> Result<(), String> {
    // Arrange
    let (_directory, mut store) = open_store()?;
    let config = fixture_config()?;

    // Act
    let created = created_id(applied(
        &mut store,
        StoreCommand::CreateRide {
            config: config.clone(),
            seed: Some(SEED),
        },
    ))?;
    let rows = ride_rows(applied(&mut store, StoreCommand::Rides))?;
    let loaded = loaded_ride(applied(&mut store, StoreCommand::LoadRide { id: created }))?;

    // Assert
    assert_eq!(rows.len(), 1);
    assert_eq!(rows.first().map(|row| row.name.as_str()), Some(RIDE_NAME));
    assert_eq!(loaded.id, created);
    assert_eq!(loaded.config, config);
    assert_eq!(loaded.roster.entries().len(), 0);
    assert_eq!(loaded.engine.state(), RideStatus::Draft);
    Ok(())
}

#[test]
fn test_apply_appends_an_event_and_persists_the_roster() -> Result<(), String> {
    // Arrange
    let (_directory, mut store) = open_store()?;
    let id = created_id(applied(
        &mut store,
        StoreCommand::CreateRide {
            config: fixture_config()?,
            seed: Some(SEED),
        },
    ))?;

    // Act
    expect_roster_saved(applied(
        &mut store,
        StoreCommand::SaveRoster {
            id,
            roster: Box::new(fixture_roster()?),
        },
    ))?;
    expect_appended(applied(
        &mut store,
        StoreCommand::Append {
            id,
            event: Box::new(Event {
                action: EventAction::Stop,
                payload: json!({}),
            }),
        },
    ))?;
    let reloaded = roster_reply(applied(&mut store, StoreCommand::RosterFor { id }))?;
    let audit = audit_rows_reply(applied(&mut store, StoreCommand::AuditRows { id }))?;

    // Assert
    assert_eq!(reloaded.entries().len(), 1);
    assert_eq!(
        reloaded
            .entries()
            .first()
            .map(|entry| entry.display_name.as_str()),
        Some("Ada Lovelace")
    );
    assert_eq!(audit.len(), 1);
    assert_eq!(audit.first().map(|row| row.action.as_str()), Some("stop"));
    Ok(())
}

#[test]
fn test_apply_appends_a_roster_event_and_reads_the_audit_rows() -> Result<(), String> {
    // Arrange
    let (_directory, mut store) = open_store()?;
    let id = created_id(applied(
        &mut store,
        StoreCommand::CreateRide {
            config: fixture_config()?,
            seed: Some(SEED),
        },
    ))?;

    // Act
    expect_roster_event_appended(applied(
        &mut store,
        StoreCommand::AppendRosterEvent {
            id,
            event: Box::new(AuditEvent {
                action: "create_solo_entry".to_owned(),
                payload: json!({ "plate": PLATE }),
            }),
        },
    ))?;
    let audit = audit_rows_reply(applied(&mut store, StoreCommand::AuditRows { id }))?;

    // Assert
    assert_eq!(audit.len(), 1);
    assert_eq!(
        audit.first().map(|row| row.action.as_str()),
        Some("create_solo_entry")
    );
    assert_eq!(audit.first().map(|row| row.entry.as_str()), Some(PLATE));
    Ok(())
}

#[test]
fn test_apply_returns_the_previous_session() -> Result<(), String> {
    // Arrange
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = directory.path().join("rides.db");
    let mut store = Store::open(&path).map_err(|error| error.to_string())?;
    let fresh = previous_session_reply(applied(&mut store, StoreCommand::PreviousSession))?;
    expect_session_closed(applied(&mut store, StoreCommand::CloseSession))?;
    drop(store);
    let mut relaunched = Store::open(&path).map_err(|error| error.to_string())?;

    // Act
    let previous = previous_session_reply(applied(&mut relaunched, StoreCommand::PreviousSession))?;

    // Assert
    assert_eq!(fresh, PreviousSession::fresh());
    assert_eq!(previous.state, SessionState::CleanQuit);
    assert_eq!(previous.ride_id, None);
    assert!(previous.ended_at.is_some());
    Ok(())
}

#[test]
fn test_apply_reports_a_store_error_for_an_unknown_ride() -> Result<(), String> {
    // Arrange
    let (_directory, mut store) = open_store()?;

    // Act
    let result = apply(&mut store, StoreCommand::RosterFor { id: UNKNOWN_ID });

    // Assert
    assert!(matches!(
        result,
        Err(StoreError::RideNotFound { id: UNKNOWN_ID })
    ));
    Ok(())
}

#[test]
fn test_apply_sets_the_active_ride() -> Result<(), String> {
    // Arrange
    let (_directory, mut store) = open_store()?;
    let id = created_id(applied(
        &mut store,
        StoreCommand::CreateRide {
            config: fixture_config()?,
            seed: Some(SEED),
        },
    ))?;

    // Act
    let active = active_ride_reply(applied(&mut store, StoreCommand::SetActiveRide { id }))?;

    // Assert
    assert_eq!(active, Some(id));
    Ok(())
}

#[test]
fn test_apply_clears_the_active_ride() -> Result<(), String> {
    // Arrange
    let (_directory, mut store) = open_store()?;
    let id = created_id(applied(
        &mut store,
        StoreCommand::CreateRide {
            config: fixture_config()?,
            seed: Some(SEED),
        },
    ))?;
    let _ = active_ride_reply(applied(&mut store, StoreCommand::SetActiveRide { id }))?;

    // Act
    let active = active_ride_reply(applied(&mut store, StoreCommand::ClearActiveRide))?;

    // Assert
    assert_eq!(active, None);
    Ok(())
}

#[test]
fn test_apply_writes_a_backup() -> Result<(), String> {
    // Arrange
    let (_directory, mut store) = open_store()?;

    // Act
    let path = backup_reply(applied(&mut store, StoreCommand::BackupNow))?;

    // Assert
    assert!(path.is_file());
    Ok(())
}

// ------------------------------------------------- the worker thread

#[test]
fn test_worker_round_trips_a_ride_create_and_load() -> Result<(), String> {
    // Arrange
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let worker =
        StoreWorker::start(directory.path().join("rides.db")).map_err(|error| error.to_string())?;
    let config = fixture_config()?;

    // Act
    worker.send(StoreCommand::CreateRide {
        config: config.clone(),
        seed: Some(SEED),
    });
    let id = created_id(only_reply(poll_drain(&worker)))?;
    worker.send(StoreCommand::LoadRide { id });
    let loaded = loaded_ride(only_reply(poll_drain(&worker)))?;
    worker.shutdown().map_err(|error| error.to_string())?;

    // Assert
    assert_eq!(loaded.id, id);
    assert_eq!(loaded.config, config);
    assert_eq!(loaded.roster.entries().len(), 0);
    assert_eq!(loaded.engine.state(), RideStatus::Draft);
    Ok(())
}

#[test]
fn test_worker_shutdown_closes_the_session() -> Result<(), String> {
    // Arrange
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let path = directory.path().join("rides.db");
    let worker = StoreWorker::start(path.clone()).map_err(|error| error.to_string())?;

    // Act
    worker.shutdown().map_err(|error| error.to_string())?;

    // Assert
    let store = Store::open(&path).map_err(|error| error.to_string())?;
    let previous = store
        .previous_session()
        .map_err(|error| error.to_string())?;
    assert_eq!(previous.state, SessionState::CleanQuit);
    assert_eq!(previous.ride_id, None);
    assert!(previous.ended_at.is_some());
    Ok(())
}

#[test]
fn test_worker_recv_returns_the_next_reply() -> Result<(), String> {
    // Arrange
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let worker =
        StoreWorker::start(directory.path().join("rides.db")).map_err(|error| error.to_string())?;

    // Act
    worker.send(StoreCommand::Rides);
    let reply = worker
        .recv()
        .ok_or_else(|| "the worker sent no reply".to_owned())?;
    let rows = ride_rows(reply)?;
    worker.shutdown().map_err(|error| error.to_string())?;

    // Assert
    assert_eq!(rows.len(), 0);
    Ok(())
}
