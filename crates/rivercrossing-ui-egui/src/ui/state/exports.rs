// SPDX-License-Identifier: GPL-3.0-only

//! The results window, the five file exports, the `WordPress` publish form and the simulator form.
//!
//! The app's views read the models here; the app's job threads take the [`ExportJob`],
//! [`PublishStart`] and [`SimStart`] values and run the slow work off the UI thread.

use std::path::{Path, PathBuf};

use rivercrossing_htmlexport::types::ExportInput;
use rivercrossing_ride::EntryMode;
use time::OffsetDateTime;

use super::AppState;
use crate::ui::action::spec::CommandId;
use crate::ui::presenters::{export_input, publish_wordpress, results, simulator};

/// The results notebook's selected tab.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ResultsTab {
    /// The team standings.
    #[default]
    Teams,
    /// The solo standings.
    Solo,
}

/// The results window's cached state.
#[derive(Debug, Default)]
pub(crate) struct ResultsState {
    /// The ranked rows, or `None` when the model refused.
    pub(crate) model: Option<results::StandingsModel>,
    /// The model's refusal, shown in the window's infobar.
    pub(crate) error: Option<String>,
    /// The event count captured at the last successful export.
    pub(crate) watermark: results::ExportWatermark,
    /// The notebook's selected tab.
    pub(crate) tab: ResultsTab,
}

/// The five file exports the Results menu commands start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExportKind {
    /// The HTML results page.
    Html,
    /// The PDF results report.
    Pdf,
    /// The podium poster PDF.
    Poster,
    /// The podium poster HTML page.
    PosterHtml,
    /// The standings CSV.
    Csv,
}

impl ExportKind {
    /// Return the stable key that the launch log and the picker carry.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Pdf => "pdf",
            Self::Poster => "poster",
            Self::PosterHtml => "poster_html",
            Self::Csv => "csv",
        }
    }

    /// Return the action id whose command starts this export.
    #[must_use]
    pub(crate) const fn command(self) -> CommandId {
        match self {
            Self::Html => CommandId::ExportHtml,
            Self::Pdf => CommandId::ExportPdf,
            Self::Poster => CommandId::ExportPoster,
            Self::PosterHtml => CommandId::ExportPosterHtml,
            Self::Csv => CommandId::ExportCsv,
        }
    }

    /// Return the export that *command* starts, when it starts one.
    #[must_use]
    pub(crate) fn from_command(command: CommandId) -> Option<Self> {
        [
            Self::Html,
            Self::Pdf,
            Self::Poster,
            Self::PosterHtml,
            Self::Csv,
        ]
        .into_iter()
        .find(|kind| kind.command() == command)
    }

    /// Return the file extension the save picker offers.
    #[must_use]
    pub(crate) const fn extension(self) -> &'static str {
        match self {
            Self::Html | Self::PosterHtml => "html",
            Self::Pdf | Self::Poster => "pdf",
            Self::Csv => "csv",
        }
    }

    /// Return the save picker's suggested file name for *ride*.
    #[must_use]
    pub(crate) fn default_file_name(self, ride: &str) -> String {
        let slug = publish_wordpress::slugify(ride);
        let tail = match self {
            Self::Html | Self::Pdf => "results",
            Self::Poster | Self::PosterHtml => "podium",
            Self::Csv => "standings",
        };
        format!("{slug}-{tail}.{}", self.extension())
    }
}

/// One render-and-write job for the store worker.
#[derive(Debug, Clone)]
pub(crate) struct ExportJob {
    /// Which file to write.
    pub(crate) kind: ExportKind,
    /// Where to write it.
    pub(crate) path: PathBuf,
    /// The composed payload.
    pub(crate) input: Box<ExportInput>,
    /// The PDF layout's pinned creation instant.
    pub(crate) created_at: OffsetDateTime,
}

/// The values a confirmed publish hands to its job thread.
#[derive(Debug, Clone)]
pub(crate) struct PublishStart {
    /// The validated form.
    pub(crate) form: publish_wordpress::PublishForm,
    /// The composed payload; the job renders the full or podium fragment.
    pub(crate) input: Box<ExportInput>,
}

/// The values a confirmed simulation hands to its job thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SimStart {
    /// The run's scripted settings.
    pub(crate) settings: simulator::SimulationSettings,
    /// The engine event count before the run, so only new events persist.
    pub(crate) pre_events: usize,
}

/// Return *n* as the numeric label a count choice renders.
#[must_use]
pub(crate) fn count_label(n: i64) -> String {
    n.to_string()
}

/// Return the count a label spells, or zero when it is not a whole number.
#[must_use]
pub(crate) fn count_value(label: &str) -> i64 {
    label.trim().parse().unwrap_or(0)
}

/// Return the label that a choice value renders, defaulting to *fallback*.
#[must_use]
fn label_for(choices: &[(&'static str, &'static str)], value: &str, fallback: &str) -> String {
    choices
        .iter()
        .find(|(_label, stored)| *stored == value)
        .map_or_else(
            || fallback.to_owned(),
            |(label, _stored)| (*label).to_owned(),
        )
}

impl AppState {
    // ----------------------------------------------------------- results

    /// Open the results window on the engine's current ranking.
    pub(crate) fn open_results(&mut self) {
        self.refresh_results();
        let _ = self.dialogs.open("results_dlg", None);
    }

    /// Rebuild the results model from the engine.
    pub(crate) fn refresh_results(&mut self) {
        let watermark = self.results.watermark.value();
        let built = match self.ride.as_ref() {
            Some(ride) => results::standings_model(
                &ride.engine,
                ride.config.tiebreak_order,
                self.settings.show_dns_riders,
                watermark,
            ),
            None => Ok(results::StandingsModel {
                teams: Vec::new(),
                solo: Vec::new(),
                stale: false,
                self_test_unverified: false,
            }),
        };
        match built {
            Ok(model) => {
                self.results.model = Some(model);
                self.results.error = None;
            }
            Err(message) => {
                self.results.model = None;
                self.results.error = Some(message);
            }
        }
    }

    /// Return the results window's headline: the ride's name and status.
    #[must_use]
    pub(crate) fn results_headline(&self) -> String {
        let Some(ride) = self.ride.as_ref() else {
            return "Standings".to_owned();
        };
        let status = ride.engine.state().as_str().to_uppercase();
        format!("Standings — {} ({status})", ride.config.name)
    }

    /// Mark the ride's current event count as the last export.
    pub(crate) fn mark_exported(&mut self) {
        let Some(ride) = self.ride.as_ref() else {
            return;
        };
        let watermark = ride.engine.events().len();
        self.results.watermark.mark_exported(watermark);
        self.refresh_results();
    }

    // ----------------------------------------------------------- exports

    /// Compose the export payload from the open ride and the publish options.
    ///
    /// # Errors
    ///
    /// Return the no-ride refusal and the standings refusal.
    pub(crate) fn build_export(&self) -> Result<ExportInput, String> {
        let Some(ride) = self.ride.as_ref() else {
            return Err("Open a ride first".to_owned());
        };
        export_input::build(
            &ride.engine,
            &ride.roster,
            &ride.config,
            &self.settings,
            &generated_stamp(),
        )
    }

    /// Return the save picker's suggested name for *kind*.
    #[must_use]
    pub(crate) fn export_file_name(&self, kind: ExportKind) -> String {
        let ride = self
            .ride
            .as_ref()
            .map_or("results", |ride| ride.config.name.as_str());
        kind.default_file_name(ride)
    }

    /// Queue *kind*'s render-and-write job for *path*.
    ///
    /// # Errors
    ///
    /// Return the no-ride and standings refusals; nothing is queued then.
    pub(crate) fn queue_export(&mut self, kind: ExportKind, path: &Path) -> Result<(), String> {
        let input = self.build_export()?;
        self.pending_export = Some(ExportJob {
            kind,
            path: path.to_path_buf(),
            input: Box::new(input),
            created_at: OffsetDateTime::now_utc(),
        });
        Ok(())
    }

    /// Record a finished export: mark its presence, its preview path and the watermark.
    pub(crate) fn record_export(&mut self, kind: ExportKind, path: &Path) {
        match kind {
            ExportKind::Html => {
                self.exports.html = true;
                self.exports.last_html = Some(path.to_path_buf());
            }
            ExportKind::PosterHtml => {
                self.exports.poster_html = true;
                self.exports.last_poster_html = Some(path.to_path_buf());
            }
            ExportKind::Pdf => {
                self.exports.pdf = true;
                self.exports.last_pdf = Some(path.to_path_buf());
            }
            ExportKind::Poster | ExportKind::Csv => {}
        }
        self.mark_exported();
        self.note(format!("Exported {}", path.display()));
    }

    /// Return the last file that a preview row opens.
    ///
    /// # Errors
    ///
    /// Return the not-yet-exported refusal when nothing was written this session.
    pub(crate) fn preview_target(&self, kind: ExportKind) -> Result<PathBuf, String> {
        let path = match kind {
            ExportKind::Html => self.exports.last_html.clone(),
            ExportKind::PosterHtml => self.exports.last_poster_html.clone(),
            ExportKind::Pdf => self.exports.last_pdf.clone(),
            ExportKind::Poster | ExportKind::Csv => None,
        };
        path.ok_or_else(|| "Export the file first".to_owned())
    }

    // ---------------------------------------------------------- publish

    /// Open the publish form, prefilled from the settings and the live ride.
    pub(crate) fn open_publish(&mut self) {
        let name = self
            .ride
            .as_ref()
            .map_or("results", |ride| ride.config.name.as_str());
        let kind = self.settings.wp_kind.clone();
        let kind_label = label_for(
            &publish_wordpress::PUBLISH_KIND_CHOICES,
            &kind,
            "Full results",
        );
        let status_label = label_for(
            &publish_wordpress::STATUS_CHOICES,
            &self.settings.wp_status,
            "Draft",
        );
        let title = publish_wordpress::default_title(name, &kind);
        let slug = publish_wordpress::default_slug(name, &kind);
        self.publish_errors = None;
        let _ = self.dialogs.open("publish_wordpress_dlg", None);
        for (field, value) in [
            ("wp_kind_choice", kind_label),
            ("wp_url_input", self.settings.wp_url.clone()),
            ("wp_username_input", self.settings.wp_username.clone()),
            ("wp_password_input", self.settings.wp_password.clone()),
            ("wp_title_input", title),
            ("wp_slug_input", slug),
            ("wp_parent_input", self.settings.wp_parent.clone()),
            ("wp_status_choice", status_label),
        ] {
            let _ = self.dialogs.set_field(field, value);
        }
    }

    /// Return the form that the publish dialog's fields hold.
    #[must_use]
    pub(crate) fn publish_form(&self) -> publish_wordpress::PublishForm {
        let field = |id: &str| self.dialogs.field(id).unwrap_or_default().to_owned();
        let kind_label = field("wp_kind_choice");
        let status_label = field("wp_status_choice");
        publish_wordpress::PublishForm {
            base_url: field("wp_url_input"),
            username: field("wp_username_input"),
            password: field("wp_password_input"),
            title: field("wp_title_input"),
            slug: field("wp_slug_input"),
            parent: field("wp_parent_input"),
            status: publish_wordpress::status_value(&status_label).to_owned(),
            kind: publish_wordpress::resolved_kind(&kind_label).to_owned(),
        }
    }

    /// Validate the form and, when it passes, persist it and start the publish.
    ///
    /// # Errors
    ///
    /// Return the validation messages joined; the dialog stays open on them.
    pub(crate) fn submit_publish(&mut self) -> Result<(), String> {
        let form = self.publish_form();
        if !form.is_valid() {
            let errors = form.errors();
            self.publish_errors = Some(errors.clone());
            return Err(errors.join("; "));
        }
        self.publish_errors = None;
        self.settings.wp_url.clone_from(&form.base_url);
        self.settings.wp_username.clone_from(&form.username);
        self.settings.wp_password.clone_from(&form.password);
        self.settings.wp_parent.clone_from(&form.parent);
        self.settings.wp_status.clone_from(&form.status);
        self.settings.wp_kind.clone_from(&form.kind);
        self.persist_settings();
        let input = self.build_export()?;
        self.publish_form = Some(form.clone());
        "Publishing…".clone_into(&mut self.publish_status);
        let _ = self.dialogs.cancel();
        let _ = self.dialogs.open("publish_running_dlg", None);
        self.pending_publish = Some(PublishStart {
            form,
            input: Box::new(input),
        });
        Ok(())
    }

    // -------------------------------------------------------- simulator

    /// Open the simulator form, prefilled from the saved `sim_*` settings.
    pub(crate) fn open_simulation(&mut self) {
        let labels = [
            ("short_lap_choice", self.settings.sim_short_laps),
            ("lapped_choice", self.settings.sim_lapped),
            ("team_stop_choice", self.settings.sim_team_stop),
        ];
        self.sim_message = None;
        let lap_km = self.ride.as_ref().map_or(8.0, |ride| ride.config.lap_km);
        let interval = if self.settings.sim_interval > 0 {
            i64::from(self.settings.sim_interval)
        } else {
            simulator::default_interval_minutes(
                lap_km,
                self.settings.avg_speed_kmh,
                simulator::INTERVAL_MIN_MINUTES,
                simulator::INTERVAL_MAX_MINUTES,
            )
        };
        let _ = self.dialogs.open("simulation_dlg", None);
        for (field, value) in [
            (
                "riders_spin",
                i64::from(self.settings.sim_riders).to_string(),
            ),
            ("teams_spin", i64::from(self.settings.sim_teams).to_string()),
            ("solo_spin", i64::from(self.settings.sim_solo).to_string()),
            ("laps_spin", i64::from(self.settings.sim_laps).to_string()),
            ("interval_spin", interval.to_string()),
        ] {
            let _ = self.dialogs.set_field(field, value);
        }
        for (field, value) in labels {
            let _ = self.dialogs.set_field(field, count_label(i64::from(value)));
        }
    }

    /// Return the simulator's five numeric fields and three count choices.
    #[must_use]
    pub(crate) fn simulation_counts(&self) -> (i64, i64, i64, i64, i64, i64, i64, i64) {
        let spin = |id: &str| {
            self.dialogs
                .field(id)
                .map_or(0, |value| value.trim().parse().unwrap_or(0))
        };
        (
            spin("riders_spin"),
            spin("teams_spin"),
            spin("solo_spin"),
            spin("laps_spin"),
            spin("interval_spin"),
            count_value(self.dialogs.field("short_lap_choice").unwrap_or("0")),
            count_value(self.dialogs.field("lapped_choice").unwrap_or("0")),
            count_value(self.dialogs.field("team_stop_choice").unwrap_or("0")),
        )
    }

    /// Generate the simulator's riders into the open draft ride.
    ///
    /// # Errors
    ///
    /// Return the no-ride refusal and the generator's or roster's own refusal.
    pub(crate) fn generate_sim_riders(&mut self) -> Result<(), String> {
        let (riders, teams, solo, ..) = self.simulation_counts();
        let seed = super::new_ride_seed();
        let Some(ride) = self.ride.as_mut() else {
            return Err("Open a ride first".to_owned());
        };
        let generated = if ride.config.entry_mode == EntryMode::Solo {
            simulator::generate_solo_riders(&mut ride.roster, riders, seed)
        } else {
            simulator::generate_riders(
                &mut ride.roster,
                &simulator::GeneratorInput {
                    total: riders,
                    teams,
                    solo,
                    seed,
                },
            )
        };
        generated?;
        self.persist_roster();
        self.note(format!("Generated {riders} riders"));
        Ok(())
    }

    /// Fill the simulator's infobar with the Check button's explanation.
    pub(crate) fn check_simulation(&mut self) {
        let (riders, teams, ..) = self.simulation_counts();
        let (min_team, max_team) = self.ride.as_ref().map_or(
            (
                i64::from(rivercrossing_roster::roster::MIN_TEAM_SIZE),
                simulator::DEFAULT_MAX_TEAM_SIZE,
            ),
            |ride| {
                (
                    i64::from(rivercrossing_roster::roster::MIN_TEAM_SIZE),
                    i64::from(ride.roster.max_team_size()),
                )
            },
        );
        self.sim_message = Some(simulator::check_message(riders, teams, min_team, max_team));
    }

    /// Validate the run, persist the `sim_*` settings and hand the run to the app.
    ///
    /// # Errors
    ///
    /// Return the no-ride refusal and the laps/interval validation message.
    pub(crate) fn submit_simulation(&mut self) -> Result<(), String> {
        if self.ride.is_none() {
            return Err("Open a ride first".to_owned());
        }
        let (_riders, _teams, _solo, laps, interval, short_laps, lapped, team_stop) =
            self.simulation_counts();
        if let Some(problem) = simulator::validate(laps, interval) {
            self.sim_message = Some(problem.clone());
            return Err(problem);
        }
        self.persist_sim_settings();
        let pre_events = self
            .ride
            .as_ref()
            .map_or(0, |ride| ride.engine.events().len());
        "Simulating…".clone_into(&mut self.sim_status);
        self.sim_progress = 0.0;
        self.sim_message = None;
        let _ = self.dialogs.cancel();
        let _ = self.dialogs.open("sim_running_dlg", None);
        self.pending_sim = Some(SimStart {
            settings: simulator::SimulationSettings {
                laps,
                interval_minutes: interval,
                short_laps,
                lapped,
                team_stop,
                seed: super::new_ride_seed(),
            },
            pre_events,
        });
        Ok(())
    }

    /// Store one progress fraction for the running simulation.
    pub(crate) fn set_sim_progress(&mut self, fraction: f32) {
        self.sim_progress = fraction.clamp(0.0, 1.0);
        self.sim_status = format!(
            "Simulated {:.0}% of crossings",
            f64::from(self.sim_progress) * 100.0
        );
    }

    /// Restore the simulated ride, persist its new events and close the progress window.
    #[expect(
        clippy::too_many_arguments,
        reason = "the finish handshake takes the ride, its outcome and the persisted-event watermark"
    )]
    pub(crate) fn finish_simulation(
        &mut self,
        ride: super::LoadedRide,
        outcome: simulator::SimOutcome,
        pre_events: usize,
    ) {
        let id = ride.ride_id;
        let new_events: Vec<rivercrossing_ride::Event> = ride
            .engine
            .events()
            .iter()
            .skip(pre_events)
            .cloned()
            .collect();
        for event in new_events {
            self.pending_store.push(super::StoreCommand::Append {
                id,
                event: Box::new(event),
            });
        }
        self.ride = Some(ride);
        self.pending_sim = None;
        let _ = self.dialogs.cancel();
        self.refresh_models();
        if outcome.cancelled {
            self.note(format!(
                "Simulation cancelled after {} crossings",
                outcome.recorded
            ));
        } else if let Some(blocked) = outcome.blocked {
            self.note(blocked.join("; "));
        } else {
            self.note(format!("Simulated {} crossings", outcome.recorded));
        }
    }

    /// Rebuild the last publish from its kept form and queue it again.
    ///
    /// # Errors
    ///
    /// Return the no-form refusal and the export composition's refusal.
    pub(crate) fn retry_publish(&mut self) -> Result<(), String> {
        let Some(form) = self.publish_form.clone() else {
            return Err("There is no publish to retry".to_owned());
        };
        let input = self.build_export()?;
        "Publishing…".clone_into(&mut self.publish_status);
        let _ = self.dialogs.open("publish_running_dlg", None);
        self.pending_publish = Some(PublishStart {
            form,
            input: Box::new(input),
        });
        Ok(())
    }

    /// Record the operator's cancel request for the running publish.
    pub(crate) fn request_publish_cancel(&mut self) {
        self.publish_cancel = true;
        "Cancelling…".clone_into(&mut self.publish_status);
    }

    /// Record the operator's cancel request for the running simulation.
    pub(crate) fn request_sim_cancel(&mut self) {
        self.sim_cancel = true;
        "Cancelling…".clone_into(&mut self.sim_status);
    }

    /// Save the simulator's fields without starting a run (its close path).
    pub(crate) fn persist_sim_settings(&mut self) {
        let (riders, teams, solo, laps, interval, short_laps, lapped, team_stop) =
            self.simulation_counts();
        self.settings.sim_riders = i32::try_from(riders).unwrap_or(0);
        self.settings.sim_teams = i32::try_from(teams).unwrap_or(0);
        self.settings.sim_solo = i32::try_from(solo).unwrap_or(0);
        self.settings.sim_laps = i32::try_from(laps).unwrap_or(0);
        self.settings.sim_interval = i32::try_from(interval).unwrap_or(0);
        self.settings.sim_short_laps = i32::try_from(short_laps).unwrap_or(0);
        self.settings.sim_lapped = i32::try_from(lapped).unwrap_or(0);
        self.settings.sim_team_stop = i32::try_from(team_stop).unwrap_or(0);
        self.persist_settings();
    }

    /// Re-seed the publish title and slug for *kind* and store the choice.
    pub(crate) fn set_publish_kind(&mut self, kind: &str) {
        let name = self
            .ride
            .as_ref()
            .map_or("results", |ride| ride.config.name.as_str())
            .to_owned();
        let title = publish_wordpress::default_title(&name, kind);
        let slug = publish_wordpress::default_slug(&name, kind);
        let _ = self.dialogs.set_field("wp_title_input", title);
        let _ = self.dialogs.set_field("wp_slug_input", slug);
        let _ = self.dialogs.set_field("wp_kind_choice", kind);
    }
}

/// Return the export footer's stamp.
fn generated_stamp() -> String {
    format!("Generated {}", OffsetDateTime::now_utc())
}
