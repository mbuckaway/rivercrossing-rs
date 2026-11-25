// SPDX-License-Identifier: GPL-3.0-only

//! The one store thread: the UI sends commands, the thread owns the `Store`
//! and answers each command on a reply channel (R-02).

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use rivercrossing_ride::{Event, RideConfig, RideEngine, SystemClock};
use rivercrossing_roster::roster::Roster;
use rivercrossing_roster::types::AuditEvent;
use rivercrossing_store::session::PreviousSession;
use rivercrossing_store::{AuditRow, RideRow, Store, StoreError};

#[cfg(test)]
mod tests;

/// One command that the UI asks the store thread to run.
#[derive(Debug)]
pub(crate) enum StoreCommand {
    /// Read every ride summary.
    Rides,
    /// Persist a new ride and return its id.
    CreateRide {
        /// The ride's validated setup.
        config: RideConfig,
        /// The seed that builds the shoe, when the UI minted one.
        seed: Option<u64>,
    },
    /// Rebuild one ride's engine and roster.
    LoadRide {
        /// The ride to load.
        id: i64,
    },
    /// Read one ride's roster.
    RosterFor {
        /// The ride whose roster to read.
        id: i64,
    },
    /// Replace one ride's persisted roster.
    SaveRoster {
        /// The ride whose roster to replace.
        id: i64,
        /// The roster snapshot to persist.
        roster: Box<Roster>,
    },
    /// Append one ride event.
    Append {
        /// The ride that owns the event.
        id: i64,
        /// The event to persist.
        event: Box<Event>,
    },
    /// Append one roster audit event.
    AppendRosterEvent {
        /// The ride that owns the event.
        id: i64,
        /// The roster event to persist.
        event: Box<AuditEvent>,
    },
    /// Read one ride's audit trail.
    AuditRows {
        /// The ride whose audit trail to read.
        id: i64,
    },
    /// Read the previous session's resume record.
    PreviousSession,
    /// Record the running ride on the open session.
    SetActiveRide {
        /// The ride that is running.
        id: i64,
    },
    /// Clear the open session's running ride.
    ClearActiveRide,
    /// Stamp the open session closed.
    CloseSession,
    /// Write a manual backup and return its path.
    BackupNow,
}

/// One answer from the store thread.
#[derive(Debug)]
pub(crate) enum StoreReply {
    /// The ride library's rows.
    Rides(Vec<RideRow>),
    /// The new ride's id.
    RideCreated(i64),
    /// One loaded ride: its id, setup, roster and rebuilt engine.
    Ride {
        /// The ride's id.
        id: i64,
        /// The ride's persisted setup.
        config: RideConfig,
        /// The roster that the ride's rows rebuild.
        roster: Box<Roster>,
        /// The engine rebuilt from the audit log.
        engine: Box<RideEngine<Roster, SystemClock>>,
    },
    /// One ride's roster.
    Roster(Box<Roster>),
    /// The roster snapshot was written.
    RosterSaved,
    /// The ride event was written.
    Appended,
    /// The roster event was written.
    RosterEventAppended,
    /// One ride's audit trail, newest first.
    AuditRows(Vec<AuditRow>),
    /// The previous session's resume record.
    PreviousSession(PreviousSession),
    /// The open session's running ride, or `None` when it has none.
    ActiveRide(Option<i64>),
    /// The open session was stamped closed.
    SessionClosed,
    /// The backup file's path.
    Backup(PathBuf),
}

/// Apply one command to the store.
///
/// # Errors
///
/// Return the store's own failure: a missing ride, an unreadable row, or a
/// SQLite, I/O or JSON failure of the write.
pub(crate) fn apply(store: &mut Store, command: StoreCommand) -> Result<StoreReply, StoreError> {
    match command {
        StoreCommand::Rides => Ok(StoreReply::Rides(store.rides()?)),
        StoreCommand::CreateRide { config, seed } => {
            Ok(StoreReply::RideCreated(store.create_ride(&config, seed)?))
        }
        StoreCommand::LoadRide { id } => {
            let engine = store.load_engine(id)?;
            let roster = store.roster_for(id)?;
            let config = engine.config().clone();
            Ok(StoreReply::Ride {
                id,
                config,
                roster: Box::new(roster),
                engine: Box::new(engine),
            })
        }
        StoreCommand::RosterFor { id } => Ok(StoreReply::Roster(Box::new(store.roster_for(id)?))),
        StoreCommand::SaveRoster { id, roster } => {
            store.save_roster(id, &roster)?;
            Ok(StoreReply::RosterSaved)
        }
        StoreCommand::Append { id, event } => {
            store.append(id, &event)?;
            Ok(StoreReply::Appended)
        }
        StoreCommand::AppendRosterEvent { id, event } => {
            store.append_roster_event(id, &event)?;
            Ok(StoreReply::RosterEventAppended)
        }
        StoreCommand::AuditRows { id } => Ok(StoreReply::AuditRows(store.audit_rows(id)?)),
        StoreCommand::PreviousSession => Ok(StoreReply::PreviousSession(store.previous_session()?)),
        StoreCommand::SetActiveRide { id } => {
            store.set_active_ride(id)?;
            Ok(StoreReply::ActiveRide(Some(id)))
        }
        StoreCommand::ClearActiveRide => {
            store.clear_active_ride()?;
            Ok(StoreReply::ActiveRide(None))
        }
        StoreCommand::CloseSession => {
            store.close_session()?;
            Ok(StoreReply::SessionClosed)
        }
        StoreCommand::BackupNow => Ok(StoreReply::Backup(store.backup_now()?)),
    }
}

/// One reply from the store thread: the reply, or the failure's display text.
type Reply = Result<StoreReply, String>;

/// The store thread's handle: the command sender and the reply channel.
#[derive(Debug)]
pub(crate) struct StoreWorker {
    /// The commands waiting for the store thread.
    commands: Sender<StoreCommand>,
    /// The replies that the store thread has sent.
    replies: Receiver<Reply>,
    /// The store thread.
    thread: JoinHandle<Result<(), StoreError>>,
}

impl StoreWorker {
    /// Open the database at *path* and start its one owner thread.
    ///
    /// # Errors
    ///
    /// Return [`Store::open`]'s failure, and an I/O failure when the thread
    /// cannot be spawned.
    pub(crate) fn start(path: PathBuf) -> Result<Self, StoreError> {
        let store = Store::open(path)?;
        let (command_sender, command_receiver) = mpsc::channel();
        let (reply_sender, reply_receiver) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("rivercrossing-store".to_owned())
            .spawn(move || run(store, &command_receiver, &reply_sender))
            .map_err(StoreError::from)?;
        Ok(Self {
            commands: command_sender,
            replies: reply_receiver,
            thread,
        })
    }

    /// Queue one command for the store thread.
    ///
    /// A worker that has already shut down drops the command, because the app
    /// is exiting; the in-memory change stands either way.
    pub(crate) fn send(&self, command: StoreCommand) {
        let _ = self.commands.send(command);
    }

    /// Return the replies that have arrived, without waiting.
    pub(crate) fn drain(&self) -> Vec<Reply> {
        self.replies.try_iter().collect()
    }

    /// Return the next reply, waiting for the store thread to answer.
    ///
    /// The app drains without waiting; a test that drives one command end to end uses this.
    pub(crate) fn recv(&self) -> Option<Reply> {
        self.replies.recv().ok()
    }

    /// Stop the store thread and stamp the session closed.
    ///
    /// # Errors
    ///
    /// Return the failure of the session close, and an I/O failure when the
    /// store thread stopped unexpectedly.
    pub(crate) fn shutdown(self) -> Result<(), StoreError> {
        let Self {
            commands, thread, ..
        } = self;
        drop(commands);
        match thread.join() {
            Ok(result) => result,
            Err(_) => Err(StoreError::Io(std::io::Error::other(
                "the store worker thread stopped unexpectedly",
            ))),
        }
    }
}

/// Own the store on its one thread until the command channel closes.
///
/// Every command is answered on *outgoing*; a failure crosses the channel as
/// its display text, so the UI can post the status notice.
fn run(
    mut store: Store,
    incoming: &Receiver<StoreCommand>,
    outgoing: &Sender<Reply>,
) -> Result<(), StoreError> {
    while let Ok(command) = incoming.recv() {
        let answer = apply(&mut store, command).map_err(|error| error.to_string());
        let _ = outgoing.send(answer);
    }
    store.close_session()
}
