// SPDX-License-Identifier: GPL-3.0-only

//! The session bookkeeping that the resume dialog reads.

use rivercrossing_ride::Timestamp;

use crate::error::StoreError;

#[cfg(test)]
mod tests;

/// How the previous app session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// The previous session closed cleanly with no running ride.
    CleanQuit,
    /// The previous session never wrote `closed_at`.
    Crashed,
    /// The previous session closed cleanly while a ride was running.
    RunningAtExit,
}

impl SessionState {
    /// Return the stable stored spelling of this state.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::CleanQuit => "clean_quit",
            Self::Crashed => "crashed",
            Self::RunningAtExit => "running_at_exit",
        }
    }
}

/// The previous session's resume record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviousSession {
    /// How the previous session ended.
    pub state: SessionState,
    /// The ride that was running at the session's end, when one was.
    pub ride_id: Option<i64>,
    /// When the session ended: `closed_at` for a clean exit, else the
    /// last heartbeat or, while no heartbeat is written, `opened_at`.
    pub ended_at: Option<Timestamp>,
}

impl PreviousSession {
    /// Return the reading for a database with no prior session.
    ///
    /// A first launch is not a crash, and there is no ride to resume, so
    /// the reading is a clean quit with no ride and no instant.
    #[must_use]
    pub const fn fresh() -> Self {
        Self {
            state: SessionState::CleanQuit,
            ride_id: None,
            ended_at: None,
        }
    }
}

/// The stored `app_session` columns that the resume reading needs.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SessionRow {
    /// The clean-quit stamp.
    pub closed_at: Option<i64>,
    /// The ride that was running at the session's end.
    pub active_ride_id: Option<i64>,
    /// The last heartbeat stamp.
    pub heartbeat_at: Option<i64>,
    /// The session's open stamp.
    pub opened_at: i64,
}

/// Build the reading from one stored `app_session` row.
///
/// A NULL `closed_at` is a crash whose instant is the last heartbeat, or
/// `opened_at` while no heartbeat is stored. A closed session with a
/// running ride is [`SessionState::RunningAtExit`]; otherwise it is
/// [`SessionState::CleanQuit`].
///
/// # Errors
///
/// Return [`StoreError::InvalidRow`] when a stored epoch does not convert
/// to an instant.
pub(crate) fn previous_session_from_row(row: SessionRow) -> Result<PreviousSession, StoreError> {
    let Some(closed_at) = row.closed_at else {
        let (column, epoch) = row
            .heartbeat_at
            .map_or(("opened_at", row.opened_at), |beat| ("heartbeat_at", beat));
        return Ok(PreviousSession {
            state: SessionState::Crashed,
            ride_id: row.active_ride_id,
            ended_at: Some(epoch_to_timestamp(column, epoch)?),
        });
    };
    let state = if row.active_ride_id.is_some() {
        SessionState::RunningAtExit
    } else {
        SessionState::CleanQuit
    };
    Ok(PreviousSession {
        state,
        ride_id: row.active_ride_id,
        ended_at: Some(epoch_to_timestamp("closed_at", closed_at)?),
    })
}

/// Convert a stored epoch to an instant, naming the column on failure.
fn epoch_to_timestamp(column: &'static str, epoch: i64) -> Result<Timestamp, StoreError> {
    Timestamp::from_unix_timestamp(epoch).map_err(|_| StoreError::InvalidRow {
        table: "app_session",
        column,
        stored: epoch.to_string(),
    })
}
