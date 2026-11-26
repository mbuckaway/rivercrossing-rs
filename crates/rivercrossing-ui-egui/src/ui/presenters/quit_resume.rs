// SPDX-License-Identifier: GPL-3.0-only

//! The quit-confirm and resume-dialog decision cores, with no rendering.
//!
//! The rules and the copy follow the Python oracle's `ui/quit_flow.py`,
//! `ui/resume_flow.py` and the launch flow in `ui/app.py`: quitting a RUNNING
//! ride asks through the wall-clock dialog, every other status asks through the
//! plain confirm, and a session that left a ride running opens the resume
//! dialog at the next launch.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the EPIC-12c views wire these decisions in a later phase; phase 1 unit-tests them"
    )
)]

use rivercrossing_ride::{RideStatus, Timestamp};
use rivercrossing_store::{PreviousSession, SessionState};

#[cfg(test)]
mod tests;

/// The plain quit confirm's frozen title.
pub(crate) const EXIT_CONFIRM_TITLE: &str = "Quit RiverCrossing?";

/// The plain quit confirm's frozen message.
pub(crate) const EXIT_CONFIRM_MESSAGE: &str = "Are you sure you want to quit? No ride is running.";

/// The plain quit confirm's affirmative label.
pub(crate) const EXIT_CONFIRM_OK_LABEL: &str = "Quit";

/// The plain quit confirm's safe label.
pub(crate) const EXIT_CONFIRM_CANCEL_LABEL: &str = "Cancel";

/// The confirm dialog that quitting asks through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuitDialog {
    /// The wall-clock warning for a RUNNING ride.
    Running,
    /// The plain no-ride-running confirm.
    Normal,
}

/// The button that ended a quit confirm.
///
/// [`QuitChoice::FinishFirst`] is reachable only from [`QuitDialog::Running`]:
/// the plain confirm carries no finish-first button, so its view never forms
/// that choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuitChoice {
    /// Quit now.
    Quit,
    /// Finish the ride first.
    FinishFirst,
    /// Stay in the app.
    Cancel,
}

/// What the app does after a quit confirm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuitOutcome {
    /// Quit now.
    Quit,
    /// Finish the ride first.
    FinishFirst,
    /// Stay in the app.
    Cancel,
}

/// The launch prompt that the previous session warrants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LaunchChoice {
    /// Show no prompt; the console stands as it is.
    Nothing,
    /// Show the resume dialog for the ride that was running.
    Resume,
    /// Open the ride library.
    Library,
}

/// Return the quit confirm that *status* asks through.
///
/// A RUNNING ride keeps its wall-clock warning whether or not the operator
/// stopped it: quitting does not stop the ride. `stopped` is the console's own
/// reading, kept in the signature so the view passes the state it renders; the
/// oracle's rule is the status alone. Every other status has no running ride
/// to protect and gets the plain confirm.
pub(crate) fn dialog_for_status(status: RideStatus, _stopped: bool) -> QuitDialog {
    if status == RideStatus::Running {
        QuitDialog::Running
    } else {
        QuitDialog::Normal
    }
}

/// Map the quit confirm's button to the app's outcome.
#[must_use]
pub(crate) const fn outcome_for(choice: QuitChoice) -> QuitOutcome {
    match choice {
        QuitChoice::Quit => QuitOutcome::Quit,
        QuitChoice::FinishFirst => QuitOutcome::FinishFirst,
        QuitChoice::Cancel => QuitOutcome::Cancel,
    }
}

/// Return the running quit dialog's message, which names the ride.
#[must_use]
pub(crate) fn running_exit_message(ride_name: &str) -> String {
    format!(
        "{ride_name} is still running. Quitting won't stop the ride — \
         it keeps timing on the wall clock, and you'll be asked to continue when you reopen."
    )
}

/// Return the launch prompt that *session* warrants.
///
/// A session that left a ride running resumes it: [`SessionState::RunningAtExit`]
/// always, and a crash only when it recorded a ride. Otherwise a store that
/// holds rides opens the library, and an empty store shows nothing.
#[must_use]
pub(crate) fn launch_choice(session: &PreviousSession, has_rides: bool) -> LaunchChoice {
    let running_at_exit = session.state == SessionState::RunningAtExit;
    let crashed_with_ride = session.state == SessionState::Crashed && session.ride_id.is_some();
    if running_at_exit || crashed_with_ride {
        return LaunchChoice::Resume;
    }
    if has_rides {
        return LaunchChoice::Library;
    }
    LaunchChoice::Nothing
}

/// Return the resume dialog's message for *`ride_name`*.
///
/// A clean quit has no running ride to resume, so wording one is a caller bug
/// and returns the refusal.
///
/// # Errors
///
/// Return the refusal when *state* is [`SessionState::CleanQuit`].
pub(crate) fn continue_message(
    ride_name: &str,
    state: SessionState,
    at: Timestamp,
) -> Result<String, String> {
    let time_text = format!("{:02}:{:02}", at.hour(), at.minute());
    match state {
        SessionState::RunningAtExit => Ok(format!(
            "\"{ride_name}\" is still running. You quit at {time_text} — \
             the ride kept running on the wall clock."
        )),
        SessionState::Crashed => Ok(format!(
            "\"{ride_name}\" is still running. The app closed unexpectedly at {time_text} — \
             the ride kept running on the wall clock."
        )),
        SessionState::CleanQuit => Err(format!("no resume copy for {}", state.as_str())),
    }
}

/// Return the error that a ride whose replay failed reports.
#[must_use]
pub(crate) fn replay_message(ride_name: &str, reason: &str) -> String {
    format!("cannot open ride {ride_name}: {reason}")
}
