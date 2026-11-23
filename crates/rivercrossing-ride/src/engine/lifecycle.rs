// SPDX-License-Identifier: GPL-3.0-only

//! The lifecycle transitions and the setup-config update.

use serde_json::{Value, json};

use super::{RideEngine, format_timestamp, illegal, shoe_structure, start_blocked};
use crate::clock::{Clock, Timestamp};
use crate::config::{RideConfig, setup_minimum_violations};
use crate::error::RideError;
use crate::event::{Event, EventAction};
use crate::format::format_elapsed;
use crate::roster_view::RosterView;
use crate::status::RideStatus;

impl<V: RosterView, C: Clock> RideEngine<V, C> {
    /// Start the ride, or continue a ride that stopped or reopened.
    ///
    /// From DRAFT the three readiness gates run in order: the roster holds at
    /// least one entry, the setup clears its minimum rule, and the roster
    /// clears its start violations. From RUNNING or REOPENED the ride
    /// continues with `actual_start` unchanged.
    ///
    /// # Errors
    ///
    /// Return [`RideError::StartBlocked`] when a DRAFT ride is not ready, and
    /// [`RideError::IllegalState`] when the ride is FINISHED.
    pub fn start(&mut self, at: Option<Timestamp>) -> Result<Event, RideError> {
        if self.state == RideStatus::Running || self.state == RideStatus::Reopened {
            return self.continue_ride();
        }
        if self.state != RideStatus::Draft {
            return Err(illegal(format!(
                "cannot start from {}",
                self.state.as_str()
            )));
        }
        self.check_start_ready()?;
        let started_at = at.unwrap_or_else(|| self.clock.now());
        Ok(self.begin_start(started_at))
    }

    /// Continue a RUNNING or REOPENED ride without moving the gun.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the ride has no start instant.
    fn continue_ride(&mut self) -> Result<Event, RideError> {
        let started_at = self.require_actual_start()?;
        self.state = RideStatus::Running;
        self.roster.set_ride_status(RideStatus::Running);
        self.stopped = false;
        self.finished_at = None;
        self.tiebreak.clear();
        Ok(self.append(Event {
            action: EventAction::Continue,
            payload: json!({
                "actual_start": format_timestamp(started_at),
                "reason": format_elapsed(0.0),
            }),
        }))
    }

    /// Check the three DRAFT readiness gates in order.
    ///
    /// # Errors
    ///
    /// Return [`RideError::StartBlocked`] with one reason per blocking issue.
    fn check_start_ready(&self) -> Result<(), RideError> {
        let entries = self.roster.entries();
        if entries.is_empty() {
            return Err(start_blocked(
                "roster has no riders".to_owned(),
                vec!["roster has no riders".to_owned()],
            ));
        }
        let setup = setup_minimum_violations(&self.config);
        if !setup.is_empty() {
            let message = format!("ride setup is incomplete: {}", setup.join("; "));
            return Err(start_blocked(message, setup));
        }
        let violations = self.roster.start_violations();
        if !violations.is_empty() {
            let reasons: Vec<String> = violations
                .iter()
                .map(|violation| format!("{}: {}", violation.plate, violation.reason))
                .collect();
            let message = format!("roster is not ready to start: {}", reasons.join("; "));
            return Err(start_blocked(message, reasons));
        }
        Ok(())
    }

    /// Record the DRAFT to RUNNING transition.
    ///
    /// The replay seam calls this directly, so a persisted `start` row is
    /// rebuilt without re-judging the readiness gates that it cleared live.
    pub(super) fn begin_start(&mut self, started_at: Timestamp) -> Event {
        self.actual_start = Some(started_at);
        self.state = RideStatus::Running;
        self.roster.set_ride_status(RideStatus::Running);
        self.stopped = false;
        self.append(Event {
            action: EventAction::Start,
            payload: json!({
                "actual_start": format_timestamp(started_at),
                "reason": format_elapsed(0.0),
            }),
        })
    }

    /// Lock plate entry while the ride stays RUNNING.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the ride is not RUNNING, is
    /// already stopped, or has no start instant.
    pub fn stop(&mut self) -> Result<Event, RideError> {
        if self.state != RideStatus::Running {
            return Err(illegal(format!(
                "cannot stop a {} ride",
                self.state.as_str()
            )));
        }
        if self.stopped {
            return Err(illegal("ride is already stopped"));
        }
        let stopped_at = self.clock.now();
        let start = self.require_actual_start()?;
        self.stopped = true;
        self.stopped_at = Some(stopped_at);
        Ok(self.append(Event {
            action: EventAction::Stop,
            payload: json!({
                "stopped_at": format_timestamp(stopped_at),
                "reason": format_elapsed((stopped_at - start).as_seconds_f64()),
            }),
        }))
    }

    /// Close a RUNNING or REOPENED ride and close the shoe.
    ///
    /// A non-empty `self_test_failed_checks` records the failed evaluator
    /// checks on the finish event and marks the results unverified. Once the
    /// finish instant is recorded, the venue's high-card draw runs for every
    /// equal-hand group.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the ride is DRAFT or FINISHED,
    /// or has no start instant.
    pub fn finish(&mut self, self_test_failed_checks: &[String]) -> Result<Event, RideError> {
        if self.state != RideStatus::Running && self.state != RideStatus::Reopened {
            return Err(illegal(format!(
                "cannot finish from {}",
                self.state.as_str()
            )));
        }
        let finished_at = self.clock.now();
        let event = self.finish_at(finished_at, self_test_failed_checks)?;
        self.record_tiebreak_draws();
        Ok(event)
    }

    /// Move to FINISHED at `finished_at` and append the finish event.
    ///
    /// The replay seam re-applies a persisted finish through this method
    /// alone, so a reloaded ride keeps its recorded instant and never draws
    /// the tie-break twice: the draw arrives as its own audit row.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the ride has no start instant.
    pub(super) fn finish_at(
        &mut self,
        finished_at: Timestamp,
        self_test_failed_checks: &[String],
    ) -> Result<Event, RideError> {
        let start = self.require_actual_start()?;
        self.state = RideStatus::Finished;
        self.roster.set_ride_status(RideStatus::Finished);
        self.stopped = false;
        self.finished_at = Some(finished_at);
        self.self_test_failed_checks = self_test_failed_checks.to_vec();
        self.shoe.close();
        let mut payload = serde_json::Map::new();
        payload.insert(
            "finished_at".to_owned(),
            json!(format_timestamp(finished_at)),
        );
        payload.insert(
            "reason".to_owned(),
            json!(format_elapsed((finished_at - start).as_seconds_f64())),
        );
        if !self_test_failed_checks.is_empty() {
            payload.insert(
                "self_test_failed_checks".to_owned(),
                json!(self_test_failed_checks),
            );
        }
        Ok(self.append(Event {
            action: EventAction::Finish,
            payload: Value::Object(payload),
        }))
    }

    /// Reopen a FINISHED ride for corrections and reopen the shoe.
    ///
    /// The recorded finish instant is preserved, and the recorded tie-break
    /// draws are cleared so the corrected ride redraws at its next finish.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the ride is not FINISHED, or
    /// has no start instant.
    pub fn reopen(&mut self) -> Result<Event, RideError> {
        if self.state != RideStatus::Finished {
            return Err(illegal(format!(
                "cannot reopen from {}",
                self.state.as_str()
            )));
        }
        let reopened_at = self.clock.now();
        self.reopen_at(reopened_at)
    }

    /// Move to REOPENED at `reopened_at` and append the reopen event.
    ///
    /// The replay seam re-applies a persisted reopen through this method,
    /// keeping the recorded instant and the finish instant it measured from.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the ride has no start instant.
    pub(super) fn reopen_at(&mut self, reopened_at: Timestamp) -> Result<Event, RideError> {
        let start = self.require_actual_start()?;
        self.state = RideStatus::Reopened;
        self.roster.set_ride_status(RideStatus::Reopened);
        self.stopped = false;
        self.tiebreak.clear();
        self.shoe.reopen();
        Ok(self.append(Event {
            action: EventAction::Reopen,
            payload: json!({
                "reopened_at": format_timestamp(reopened_at),
                "reason": format_elapsed((reopened_at - start).as_seconds_f64()),
            }),
        }))
    }

    /// Back-date the gun and log how far it moved.
    ///
    /// Lap 1 derives its time from the gun, so this correction recomputes
    /// lap 1 alone.
    ///
    /// # Errors
    ///
    /// Return [`RideError::IllegalState`] when the ride is not RUNNING, or has
    /// no start instant.
    pub fn set_start_time(&mut self, at: Timestamp) -> Result<Event, RideError> {
        if self.state != RideStatus::Running {
            return Err(illegal(format!(
                "cannot set start time from {}",
                self.state.as_str()
            )));
        }
        let previous = self.require_actual_start()?;
        self.actual_start = Some(at);
        Ok(self.append(Event {
            action: EventAction::SetStartTime,
            payload: json!({
                "actual_start": format_timestamp(at),
                "previous_start": format_timestamp(previous),
                "reason": format_elapsed((at - previous).as_seconds_f64()),
            }),
        }))
    }

    /// Replace the ride's setup config.
    ///
    /// A DRAFT ride whose deck count, joker count, or jokers mode changed
    /// rebuilds the shoe from its stored seed. Past DRAFT the shoe is never
    /// touched.
    ///
    /// # Errors
    ///
    /// Return [`RideError::Config`] when a shoe count is above 255, and
    /// [`RideError::IllegalState`] when the shoe refuses the rebuild.
    pub fn update_config(&mut self, config: RideConfig) -> Result<(), RideError> {
        let rebuild = self.state == RideStatus::Draft
            && shoe_structure(&config) != shoe_structure(&self.config);
        if !rebuild {
            self.config = config;
            return Ok(());
        }
        let decks = shoe_count("deck_count", config.deck_count)?;
        let jokers = shoe_count("jokers_per_deck", config.jokers_per_deck)?;
        self.shoe
            .reconfigure(decks, jokers, config.jokers_mode)
            .map_err(|error| illegal(error.to_string()))?;
        self.config = config;
        Ok(())
    }
}

/// Convert one shoe count to the shoe's `u8` width.
///
/// # Errors
///
/// Return [`RideError::Config`] when the count is above 255.
fn shoe_count(field: &str, value: u32) -> Result<u8, RideError> {
    u8::try_from(value).map_err(|_| RideError::Config {
        message: format!("{field} must be at most 255, got {value}"),
    })
}
