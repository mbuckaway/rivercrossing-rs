// SPDX-License-Identifier: GPL-3.0-only

//! The `main_frame` console: the ride header, the clock columns, the plate entry, the crossings
//! feed with its two optional time columns (R-37), the counters and the two-tab review notebook
//! (R-84).
//!
//! Every control carries its frozen §15b id through `ui.push_id` and an explicit accessible name.
//! The stop light never carries meaning by colour alone: the state label beside it spells the ride
//! state. The console reads the presenter models; it makes no decision of its own.

use craft::egui::{self, Color32, Key, Stroke, TextEdit, Ui};
use rivercrossing_ride::RideStatus;
use rivercrossing_ride::RosterView;

use super::text;
use crate::ui::action::enable::RidePhase;
use crate::ui::action::id::{MI_START_RIDE, MI_STOP_RIDE, MI_UNDO_CROSSING};
use crate::ui::presenters::console::{self, ConsoleGates};
use crate::ui::presenters::data_source::{self, Counters, FeedRow, StopLight, format_duration};
use crate::ui::state::{AppState, ConsoleTab};

/// The status label's text when no ride is open.
const NO_RIDE: &str = "NO RIDE";

/// The plate field's hint while the entry is closed.
const ENTRY_CLOSED_HINT: &str = "start the ride to record";

/// The diameter of one clock dial, in points.
const DIAL_SIZE: f32 = 48.0;

/// Show the whole console.
pub(crate) fn show(ui: &mut Ui, state: &mut AppState) {
    ui.vertical(|ui| {
        header(ui, state);
        ui.add_space(4.0);
        entry_row(ui, state);
        ui.separator();
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| feed_panel(ui, state));
            ui.separator();
            ui.vertical(|ui| review_panel(ui, state));
        });
        ui.separator();
        status_bar(ui, state);
    });
}

// ----------------------------------------------------------------- header

/// Show the status light, the ride groups, the lap, the two clock columns and the run buttons.
fn header(ui: &mut Ui, state: &mut AppState) {
    let (status, stopped, lap, config) = match &state.ride {
        Some(ride) => (
            Some(ride.engine.state()),
            ride.engine.stopped(),
            data_source::current_lap(ride.engine.crossings()),
            Some(&ride.config),
        ),
        None => (None, false, 0, None),
    };
    let light = data_source::stop_light_mode(status, stopped);
    let status_text = status.map_or_else(
        || NO_RIDE.to_owned(),
        |status| data_source::status_text(status, stopped),
    );
    let (name, date, venue, organizer, scorer, lap_km) = match config {
        Some(config) => (
            config.name.clone(),
            format_date(config.event_date),
            config.venue.clone(),
            config.organizer.clone(),
            config.scorer.clone(),
            config.lap_km,
        ),
        None => (
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            0.0,
        ),
    };
    ui.horizontal(|ui| {
        ui.push_id("ride_status_panel", |ui| {
            ui.horizontal(|ui| {
                ui.push_id("ride_status_light", |ui| stop_light(ui, light));
                ui.push_id("ride_status_lbl", |ui| text(ui, &status_text));
            });
        });
        ui.separator();
        value(ui, "ride_name_value", "Ride", &name);
        value(ui, "ride_date_value", "Date", &date);
        value(ui, "ride_venue_value", "Venue", &venue);
        value(ui, "ride_organizer_value", "Organizer", &organizer);
        value(ui, "ride_scorer_value", "Scorer", &scorer);
        value(ui, "ride_lap_km_value", "Lap km", &format!("{lap_km:.1}"));
        ui.separator();
        ui.push_id("current_lap_lbl", |ui| text(ui, &format!("Lap {lap}")));
        ui.separator();
        clocks(ui, state);
        ui.separator();
        buttons(ui, state);
    });
}

/// Show one header value as its caption beside the value.
fn value(ui: &mut Ui, id: &str, caption: &str, value: &str) {
    ui.push_id(id, |ui| {
        ui.vertical(|ui| {
            text(ui, caption);
            text(ui, value);
        });
    });
}

/// Show the two clock columns, each a caption, a dial and its numeric readout.
fn clocks(ui: &mut Ui, state: &mut AppState) {
    let (elapsed, remaining, total) = match &state.ride {
        Some(ride) => {
            let seconds = data_source::clock_seconds(&ride.engine, state.clock_frozen);
            let total = f64::from(ride.config.planned_duration_s);
            if ride.engine.state() == RideStatus::Draft {
                (0.0, 0.0, total)
            } else {
                (seconds, (total - seconds).max(0.0), total)
            }
        }
        None => (0.0, 0.0, 0.0),
    };
    clock_column(
        ui,
        "elapsed_clock_panel",
        "elapsed_clock",
        "clock_elapsed_lbl",
        "Elapsed",
        data_source::clock_fraction(elapsed, total),
        &format_duration(elapsed),
    );
    clock_column(
        ui,
        "remaining_clock_panel",
        "remaining_clock",
        "clock_remaining_lbl",
        "Remaining",
        data_source::clock_fraction(remaining, total),
        &format_duration(remaining),
    );
}

/// Show one clock column.
fn clock_column(
    ui: &mut Ui,
    panel: &str,
    clock: &str,
    label: &str,
    caption: &str,
    fraction: f32,
    reading: &str,
) {
    ui.push_id(panel, |ui| {
        ui.vertical(|ui| {
            text(ui, caption);
            ui.push_id(clock, |ui| dial(ui, fraction));
            ui.push_id(label, |ui| text(ui, reading));
        });
    });
}

/// Paint a clock dial that fills toward *fraction*.
fn dial(ui: &mut Ui, fraction: f32) {
    let (rect, _response) =
        ui.allocate_exact_size(egui::vec2(DIAL_SIZE, DIAL_SIZE), egui::Sense::hover());
    let painter = ui.painter();
    let center = rect.center();
    let radius = rect.width() / 2.0 - 2.0;
    painter.circle_stroke(
        center,
        radius,
        Stroke::new(2.0, ui.visuals().weak_text_color()),
    );
    let angle = -std::f32::consts::FRAC_PI_2 + fraction * std::f32::consts::TAU;
    let tip = center + egui::vec2(radius * angle.cos(), radius * angle.sin());
    painter.line_segment(
        [center, tip],
        Stroke::new(3.0, ui.visuals().selection.stroke.color),
    );
}

/// Paint the ride-status stop light.
fn stop_light(ui: &mut Ui, mode: StopLight) {
    let colour = match mode {
        StopLight::Off => Color32::from_gray(140),
        StopLight::Green => Color32::from_rgb(46, 160, 67),
        StopLight::Yellow => Color32::from_rgb(220, 160, 20),
        StopLight::Red => Color32::from_rgb(200, 55, 55),
    };
    let (rect, _response) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 8.0, colour);
    ui.painter()
        .circle_stroke(rect.center(), 8.0, Stroke::new(1.0, Color32::from_gray(60)));
}

/// Show the start, stop and undo buttons, each dispatching its §15 action.
fn buttons(ui: &mut Ui, state: &mut AppState) {
    let gates = match &state.ride {
        Some(ride) => console::console_gates(&ride.engine),
        None => ConsoleGates {
            start: false,
            stop: false,
            undo: false,
        },
    };
    ui.push_id("start_btn", |ui| {
        if ui
            .add_enabled(gates.start, egui::Button::new("Start Ride"))
            .clicked()
        {
            let _ = state.act(MI_START_RIDE);
        }
    });
    ui.push_id("stop_btn", |ui| {
        if ui
            .add_enabled(gates.stop, egui::Button::new("Stop Ride"))
            .clicked()
        {
            let _ = state.act(MI_STOP_RIDE);
        }
    });
    ui.push_id("undo_btn", |ui| {
        if ui
            .add_enabled(gates.undo, egui::Button::new("Undo"))
            .clicked()
        {
            let _ = state.act(MI_UNDO_CROSSING);
        }
    });
}

// ------------------------------------------------------------ entry row

/// Show the plate field, the record button, the last-crossing label and the search field.
fn entry_row(ui: &mut Ui, state: &mut AppState) {
    let entry_open = state.phase() == RidePhase::Running;
    let hint = if entry_open {
        "Plate"
    } else {
        ENTRY_CLOSED_HINT
    };
    ui.horizontal(|ui| {
        let response = ui
            .push_id("plate_input", |ui| {
                ui.add_enabled(
                    entry_open,
                    TextEdit::singleline(&mut state.plate)
                        .hint_text(hint)
                        .desired_width(120.0),
                )
                .accessible_name("Plate")
            })
            .inner;
        let submitted = entry_open
            && state.dialogs.is_empty()
            && response.lost_focus()
            && ui.input(|input| input.key_pressed(Key::Enter));
        if submitted {
            state.submit_plate();
        }
        ui.push_id("record_btn", |ui| {
            if ui
                .add_enabled(entry_open, egui::Button::new("Record"))
                .clicked()
            {
                state.submit_plate();
            }
        });
        let last = match &state.ride {
            Some(ride) => data_source::feed_rows(&ride.engine, &ride.roster)
                .first()
                .map_or_else(|| "No crossings yet".to_owned(), last_crossing_text),
            None => "No crossings yet".to_owned(),
        };
        ui.push_id("last_crossing_lbl", |ui| text(ui, &last));
        ui.separator();
        ui.push_id("crossings_search", |ui| {
            ui.add(
                TextEdit::singleline(&mut state.search)
                    .hint_text("Search")
                    .desired_width(160.0),
            )
            .accessible_name("Search crossings");
        });
    });
}

/// Return one feed row's summary text.
fn last_crossing_text(row: &FeedRow) -> String {
    format!("Last: {} lap {}", row.plate, row.lap)
}

// ------------------------------------------------------------ feed panel

/// Show the counters and the crossings feed with its optional time columns.
fn feed_panel(ui: &mut Ui, state: &mut AppState) {
    let (rows, counters) = match &state.ride {
        Some(ride) => (
            data_source::feed_rows(&ride.engine, &ride.roster),
            data_source::counters(&ride.engine, &ride.roster),
        ),
        None => (Vec::new(), empty_counters()),
    };
    counters_row(ui, &counters);
    let visible = data_source::filter_rows(&rows, &state.search);
    let show_lap_time = state.settings.show_lap_time;
    let show_total = state.settings.show_total_times;
    ui.push_id("crossings_list", |ui| {
        egui::ScrollArea::vertical()
            .id_salt("crossings_list_scroll")
            .max_height(240.0)
            .show(ui, |ui| {
                egui::Grid::new("crossings_grid")
                    .num_columns(4 + usize::from(show_lap_time) + usize::from(show_total))
                    .striped(true)
                    .show(ui, |ui| {
                        text(ui, "Plate");
                        text(ui, "Lap");
                        text(ui, "Card");
                        if show_lap_time {
                            text(ui, "Lap time");
                        }
                        if show_total {
                            text(ui, "Total");
                        }
                        text(ui, "Rider");
                        ui.end_row();
                        for row in &visible {
                            text(ui, &row.plate);
                            text(ui, &row.lap.to_string());
                            text(ui, row.card.as_deref().unwrap_or(""));
                            if show_lap_time {
                                text(ui, &format_duration(row.lap_time));
                            }
                            if show_total {
                                text(ui, &format_duration(row.total_time));
                            }
                            text(ui, &row.name);
                            ui.end_row();
                        }
                    });
            });
    });
}

/// Show the console's counter chips.
fn counters_row(ui: &mut Ui, counters: &Counters) {
    ui.horizontal_wrapped(|ui| {
        ui.push_id("crossings_count_lbl", |ui| {
            text(ui, &format!("Crossings: {}", counters.crossings));
        });
        ui.push_id("cards_count_lbl", |ui| {
            text(ui, &format!("Cards: {}", counters.cards_dealt));
        });
        ui.push_id("on_course_lbl", |ui| {
            text(ui, &format!("On course: {}", counters.on_course));
        });
        ui.push_id("shoe_lbl", |ui| {
            text(
                ui,
                &format!("Shoe: {}/{}", counters.shoe_remaining, counters.shoe_total),
            );
        });
        ui.push_id("riders_count_lbl", |ui| {
            text(ui, &format!("Riders: {}", counters.riders));
        });
        ui.push_id("teams_count_lbl", |ui| {
            text(ui, &format!("Teams: {}", counters.teams));
        });
    });
}

/// Return the all-zero counters for the no-ride console.
fn empty_counters() -> Counters {
    Counters {
        crossings: 0,
        cards_dealt: 0,
        on_course: 0,
        shoe_remaining: 0,
        shoe_total: 0,
        riders: 0,
        teams: 0,
    }
}

// ---------------------------------------------------------- review panel

/// Show the two-tab review notebook: the live roster, and the crossings that need review.
fn review_panel(ui: &mut Ui, state: &mut AppState) {
    ui.push_id("review_notebook", |ui| {
        ui.horizontal(|ui| {
            if ui
                .selectable_label(state.console_tab == ConsoleTab::Riders, "Riders")
                .clicked()
            {
                state.console_tab = ConsoleTab::Riders;
            }
            if ui
                .selectable_label(state.console_tab == ConsoleTab::NeedsReview, "Needs Review")
                .clicked()
            {
                state.console_tab = ConsoleTab::NeedsReview;
            }
        });
    });
    match state.console_tab {
        ConsoleTab::Riders => riders_tab(ui, state),
        ConsoleTab::NeedsReview => review_tab(ui, state),
    }
}

/// Show the live roster: plate, name and team.
fn riders_tab(ui: &mut Ui, state: &mut AppState) {
    let entries = match &state.ride {
        Some(ride) => RosterView::entries(&ride.roster).clone(),
        None => Vec::new(),
    };
    ui.push_id("console_riders_list", |ui| {
        egui::ScrollArea::vertical()
            .id_salt("console_riders_scroll")
            .max_height(200.0)
            .show(ui, |ui| {
                egui::Grid::new("console_riders_grid")
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        text(ui, "Plate");
                        text(ui, "Name");
                        text(ui, "Team");
                        ui.end_row();
                        for entry in &entries {
                            let team = if entry.is_team {
                                entry.display_name.clone()
                            } else {
                                String::new()
                            };
                            for rider in &entry.riders {
                                let plate = rider
                                    .plate
                                    .clone()
                                    .filter(|plate| !plate.is_empty())
                                    .unwrap_or_else(|| entry.plate.clone());
                                text(ui, &plate);
                                text(ui, &rider.full_name);
                                text(ui, &team);
                                ui.end_row();
                            }
                        }
                    });
            });
    });
}

/// Show the "Needs Review" tab: the flagged rows, the held-only filter and the review button.
fn review_tab(ui: &mut Ui, state: &mut AppState) {
    let rows = match &state.ride {
        Some(ride) => data_source::review_rows(&ride.engine, &ride.roster, state.held_only),
        None => Vec::new(),
    };
    ui.horizontal(|ui| {
        ui.push_id("show_held_only_chk", |ui| {
            ui.checkbox(&mut state.held_only, "Show held only");
        });
        ui.push_id("review_btn", |ui| {
            ui.add_enabled(false, egui::Button::new("Review…"));
        });
    });
    ui.push_id("flagged_list", |ui| {
        egui::ScrollArea::vertical()
            .id_salt("flagged_scroll")
            .max_height(170.0)
            .show(ui, |ui| {
                egui::Grid::new("flagged_grid")
                    .num_columns(6)
                    .striped(true)
                    .show(ui, |ui| {
                        for heading in ["Issue", "Card", "Plate", "Lap", "Lap time", "Rider"] {
                            text(ui, heading);
                        }
                        ui.end_row();
                        for row in &rows {
                            text(ui, &row.issue);
                            text(ui, &row.card);
                            text(ui, &row.plate);
                            text(ui, &row.lap.to_string());
                            text(ui, &format_duration(row.lap_time));
                            text(ui, &row.rider);
                            ui.end_row();
                        }
                    });
            });
    });
}

// ----------------------------------------------------------- status bar

/// Show the status bar's notice.
fn status_bar(ui: &mut Ui, state: &AppState) {
    ui.push_id("main_statusbar", |ui| {
        text(ui, state.notice.as_deref().unwrap_or(""));
    });
}

/// Return *date* as the header's `YYYY-MM-DD` text.
fn format_date(date: time::Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}
