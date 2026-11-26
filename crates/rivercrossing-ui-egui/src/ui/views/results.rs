// SPDX-License-Identifier: GPL-3.0-only

//! The results window: the Teams/Solo notebook and the stale-export infobar.

use craft::egui::{self, Ui};

use super::text;
use crate::ui::presenters::results::StandingsRowModel;
use crate::ui::state::AppState;
use crate::ui::state::exports::ResultsTab;

/// Show the standings notebook.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState) {
    let headline = state.results_headline();
    text(ui, &headline);
    if let Some(error) = state.results.error.clone() {
        ui.push_id("results_error", |ui| text(ui, &error));
    }
    let Some(model) = state.results.model.clone() else {
        return;
    };
    let solo_only = state
        .ride
        .as_ref()
        .is_some_and(|ride| ride.config.entry_mode == rivercrossing_ride::EntryMode::Solo);
    ui.push_id("results_notebook", |ui| {
        if solo_only {
            ui.push_id("standings_list", |ui| {
                grid(ui, &model.solo, "standings_grid");
            });
            return;
        }
        ui.horizontal(|ui| {
            let tab = &mut state.results.tab;
            ui.selectable_value(tab, ResultsTab::Teams, "Teams")
                .on_hover_text("Team standings");
            ui.selectable_value(tab, ResultsTab::Solo, "Solo")
                .on_hover_text("Solo standings");
        });
        match state.results.tab {
            ResultsTab::Teams => {
                ui.push_id("teams_standings_list", |ui| {
                    grid(ui, &model.teams, "teams_standings_grid");
                });
            }
            ResultsTab::Solo => {
                ui.push_id("solo_standings_list", |ui| {
                    grid(ui, &model.solo, "solo_standings_grid");
                });
            }
        }
    });
    if model.stale {
        ui.push_id("stale_infobar", |ui| {
            text(
                ui,
                "A correction landed after the last export; export again.",
            );
        });
    }
}

/// Show one section's rows.
fn grid(ui: &mut Ui, rows: &[StandingsRowModel], salt: &str) {
    egui::ScrollArea::vertical()
        .id_salt(format!("{salt}_scroll"))
        .max_height(320.0)
        .show(ui, |ui| {
            egui::Grid::new(salt)
                .num_columns(9)
                .striped(true)
                .show(ui, |ui| {
                    for heading in [
                        "Place", "Plate", "Entry", "Type", "Sex", "Laps", "Hand", "Draw", "Total",
                    ] {
                        text(ui, heading);
                    }
                    ui.end_row();
                    for row in rows {
                        text(ui, &place_text(row));
                        text(ui, &row.plate);
                        text(ui, &row.name);
                        text(ui, kind_text(row));
                        text(ui, row.sex.as_deref().unwrap_or(""));
                        text(ui, &row.laps.to_string());
                        text(ui, &row.hand);
                        text(ui, &row.draw);
                        text(ui, &duration_text(row.total_time));
                        ui.end_row();
                    }
                });
        });
}

/// Return the Place cell: the number, or the DNS marker.
fn place_text(row: &StandingsRowModel) -> String {
    row.place
        .map_or_else(|| "DNS".to_owned(), |place| place.to_string())
}

/// Return the Type cell.
fn kind_text(row: &StandingsRowModel) -> &'static str {
    match row.kind {
        rivercrossing_ride::SnapshotKind::Team => "Team",
        rivercrossing_ride::SnapshotKind::Solo => "Solo",
    }
}

/// Return *seconds* as `H:MM:SS`, zero when no crossing credited a time.
fn duration_text(seconds: f64) -> String {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the board shows whole seconds of a non-negative ride clock"
    )]
    let whole = seconds.max(0.0) as u64;
    format!(
        "{}:{:02}:{:02}",
        whole / 3600,
        whole % 3600 / 60,
        whole % 60
    )
}
