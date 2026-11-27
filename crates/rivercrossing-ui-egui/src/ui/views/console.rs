// SPDX-License-Identifier: GPL-3.0-only

//! The `main_frame` console on the `ArtCraft` docked shell.
//!
//! The shell holds the title bar (with the in-window menu bar where no platform bar is installed), the options
//! bar (appearance, density and zoom), the tool rail, the workspace and the status bar. The workspace holds the
//! ride header, the two flattened clocks (`Progress` plus a numeric read-out), the plate-entry row, and the two
//! docked panels: the crossings feed and the Riders / Needs Review review notebook.
//!
//! Every control keeps its frozen §15b id through `ui.push_id` and its accessible name. The ride state never
//! reads by colour alone: the badge beside the state label spells the same word. The console reads the
//! presenter models; it makes no decision of its own.

use std::borrow::Cow;

use craft::behavior::{
    Axis, ColumnKey, ColumnWidth, DisplayOrder, IconId, ItemSource, Selection, SelectionChange,
};
use craft::egui::{Align, Color32, Layout, Sense, Ui, UiBuilder, vec2};
use craft::theme::variant::{Emphasis, Tone};
use craft::theme::{Srgba, Variant};
use craft::ui::bars::{Bar, BarKind};
use craft::ui::buttons::Button;
use craft::ui::choice::Checkbox;
use craft::ui::contract::{
    Bind, CollectionEvent, ColumnSpec, Entry, Props as _, Show as _, ShowCollection as _,
    ShowContainer as _, Sizing,
};
use craft::ui::display::{Badge, Divider, Progress, Shade, Text};
use craft::ui::docking::Splitter;
use craft::ui::list::ListRow;
use craft::ui::pickers::Select;
use craft::ui::primitives::IconSpec;
use craft::ui::runtime::env::{Env, WindowKey};
use craft::ui::shell::{AppShell, Title, TitleBar};
use craft::ui::style::UiTokens;
use craft::ui::table::Table;
use craft::ui::tabs::{Tab, TabStrip, TabStripOutput};
use craft::ui::text::TextField;
use rivercrossing_ride::{RideStatus, RosterView};

use super::menu;
use crate::ui::action::enable::{GateState, RidePhase, enabled};
use crate::ui::action::id::{
    ActionId, MI_AUDIT_TRAIL, MI_CHECK_RIDER_ISSUES, MI_OPEN_LIBRARY, MI_RIDER_EDITOR,
    MI_STANDINGS, MI_START_RIDE, MI_STOP_RIDE, MI_TEAM_EDITOR, MI_UNDO_CROSSING, WX_ID_PREFERENCES,
};
use crate::ui::presenters::console as labels;
use crate::ui::presenters::data_source::{
    self, Counters, FeedRow, ReviewRow, StopLight, format_duration,
};
use crate::ui::state::{AppState, ConsoleTab};
use crate::ui::window::APP_NAME;

/// The status label's text when no ride is open.
const NO_RIDE: &str = "NO RIDE";

/// The plate field's hint while the entry is closed.
const ENTRY_CLOSED_HINT: &str = "start the ride to record";

/// The plate field's hint while the entry is open.
const ENTRY_OPEN_HINT: &str = "Plate";

/// The search field's accessible name.
const SEARCH_LABEL: &str = "Search crossings";

/// The last-crossing line with no crossings yet.
const NO_CROSSINGS: &str = "No crossings yet";

/// The review dock's share of the workspace width; the feed takes the rest.
const REVIEW_DOCK_SHARE: f32 = 1.0 / 3.0;

/// The crossings feed's visible rows before it scrolls inside its panel.
const FEED_ROWS: u16 = 8;

/// The review tables' visible rows before they scroll inside their panel.
const REVIEW_ROWS: u16 = 8;

/// The System appearance choice.
const APPEARANCE_SYSTEM: u8 = 0;
/// The Light appearance choice.
const APPEARANCE_LIGHT: u8 = 1;
/// The Dark appearance choice.
const APPEARANCE_DARK: u8 = 2;

/// The Compact density choice.
const DENSITY_COMPACT: u8 = 0;
/// The Comfortable density choice.
const DENSITY_COMFORTABLE: u8 = 1;

/// The Start and Stop ride buttons, gated by and dispatching their §15 actions.
const RUN_BUTTONS: [(ActionId, &str, Variant); 2] = [
    (MI_START_RIDE, labels::START_RIDE_LABEL, Variant::PRIMARY),
    (MI_STOP_RIDE, labels::STOP_RIDE_LABEL, Variant::SECONDARY),
];

/// The tool rail: the seven navigation entries, each dispatching the same action as the menu.
///
/// The labels are the design's rail words (`design/mockups/index.html`, `.app-rail`), shorter than
/// the registry's own: the rail is a fixed-width column, and a full action label elides in it. The
/// icons are catalog meanings, so the set follows the theme's icon pack.
const RAIL: [(ActionId, &str, &str); 7] = [
    (MI_OPEN_LIBRARY, "Library", "folder_open"),
    (MI_RIDER_EDITOR, "Riders", "grid"),
    (MI_TEAM_EDITOR, "Teams", "layers"),
    (MI_CHECK_RIDER_ISSUES, "Issues", "warning"),
    (MI_STANDINGS, "Standings", "gauge"),
    (MI_AUDIT_TRAIL, "Audit", "clock"),
    (WX_ID_PREFERENCES, "Settings", "settings"),
];

/// The crossings feed's label and plate columns.
const NAME: ColumnKey = ColumnKey::new("name");
/// The plate column.
const PLATE: ColumnKey = ColumnKey::new("plate");
/// The dealt-card column.
const CARD: ColumnKey = ColumnKey::new("card");
/// The lap column.
const LAP: ColumnKey = ColumnKey::new("lap");
/// The lap-time column.
const LAP_TIME: ColumnKey = ColumnKey::new("lap_time");
/// The total-time column.
const TOTAL: ColumnKey = ColumnKey::new("total");
/// The team column.
const TEAM: ColumnKey = ColumnKey::new("team");
/// The review-issue column.
const ISSUE: ColumnKey = ColumnKey::new("issue");

/// Show the whole console on the docked shell.
pub(crate) fn show(ui: &mut Ui, state: &mut AppState, menu_bar: bool) {
    AppShell::new(WindowKey::main())
        .title_bar(move |ui, _, state: &mut AppState| {
            title_bar(ui, state, menu_bar);
        })
        .options_bar(true, |ui, _, state| options_bar(ui, state))
        .status(true, |ui, _, state| status_bar(ui, state))
        .body_label("Console")
        .body(|ui, _, state| body(ui, state))
        .show(ui, state);
}

// ----------------------------------------------------------------- title bar

/// Show the title bar: the in-window menu bar where no platform bar is installed, then the product and ride title.
///
/// The title is leading content, not the bar's own `Title`: a bar title sits in the gap, centred,
/// which is not the design's left-aligned `RiverCrossing — <ride>` (`design/mockups/index.html`,
/// `.app-titlebar`). With the frame's own title bar hidden, this is the window's only title row.
fn title_bar(ui: &mut Ui, state: &mut AppState, menu_bar: bool) {
    let _ = TitleBar::new().show_titled(
        ui,
        Title::new(""),
        |ui, _| {
            if menu_bar {
                menu::show(ui, state);
            }
            let _ = Text::new(ride_title(state)).show(ui);
        },
        |_, _| {},
    );
}

/// Return the title bar's text: the product name, with the ride's name when one is open.
fn ride_title(state: &AppState) -> String {
    match &state.ride {
        Some(ride) => format!("{APP_NAME} — {}", ride.config.name),
        None => APP_NAME.to_owned(),
    }
}

// ---------------------------------------------------------------- options bar

/// Show the options bar: the appearance and density choices and the zoom read-out.
fn options_bar(ui: &mut Ui, state: &mut AppState) {
    let _ = Bar::new("Options", BarKind::Options).show(ui, |ui, _| {
        appearance_choice(ui, state);
        density_choice(ui, state);
        let _ = Divider::vertical().show(ui);
        let _ = Text::new(format!("Zoom {}%", state.settings.zoom_percent)).show(ui);
    });
}

/// Show the appearance choice and persist a new mode.
fn appearance_choice(ui: &mut Ui, state: &mut AppState) {
    let entries = [
        Entry::new(APPEARANCE_SYSTEM, "System"),
        Entry::new(APPEARANCE_LIGHT, "Light"),
        Entry::new(APPEARANCE_DARK, "Dark"),
    ];
    let current = appearance_index(&state.settings.appearance);
    let mut picked = current;
    let _ = Select::new("Appearance", Bind::bound(&mut picked), &entries)
        .sizing(Sizing::Content)
        .show(ui);
    if picked != current {
        appearance_spelling(picked).clone_into(&mut state.settings.appearance);
        let _ = state.save_settings();
    }
}

/// Show the density choice and persist a new preset.
fn density_choice(ui: &mut Ui, state: &mut AppState) {
    let entries = [
        Entry::new(DENSITY_COMPACT, "Compact"),
        Entry::new(DENSITY_COMFORTABLE, "Comfortable"),
    ];
    let current = density_index(&state.settings.density);
    let mut picked = current;
    let _ = Select::new("Density", Bind::bound(&mut picked), &entries)
        .sizing(Sizing::Content)
        .show(ui);
    if picked != current {
        density_spelling(picked).clone_into(&mut state.settings.density);
        let _ = state.save_settings();
    }
}

/// Return the appearance choice for a stored spelling; an unknown spelling is System.
fn appearance_index(spelling: &str) -> u8 {
    match spelling {
        "light" => APPEARANCE_LIGHT,
        "dark" => APPEARANCE_DARK,
        _ => APPEARANCE_SYSTEM,
    }
}

/// Return the stored spelling for an appearance choice.
fn appearance_spelling(choice: u8) -> &'static str {
    match choice {
        APPEARANCE_LIGHT => "light",
        APPEARANCE_DARK => "dark",
        _ => "system",
    }
}

/// Return the density choice for a stored spelling; an unknown spelling is Compact.
fn density_index(spelling: &str) -> u8 {
    if spelling == "comfortable" {
        DENSITY_COMFORTABLE
    } else {
        DENSITY_COMPACT
    }
}

/// Return the stored spelling for a density choice.
fn density_spelling(choice: u8) -> &'static str {
    if choice == DENSITY_COMFORTABLE {
        "comfortable"
    } else {
        "compact"
    }
}

// -------------------------------------------------------------------- the rail

/// The rail's width: the approved mockup's 96-point labelled column (`design/mockups/index.html`,
/// `.app-rail`). The `ArtCraft` rail token is narrower because it is an icon-only rail; this app's
/// rail carries the design's words, so it sets its own width and lets the slot follow its content.
const RAIL_WIDTH: f32 = 118.0;

/// Show the tool rail: one navigation button per entry, each gated and dispatching its §15 action.
fn rail(ui: &mut Ui, state: &mut AppState) {
    let gate = state.gate();
    let column = vec2(RAIL_WIDTH, ui.available_height());
    ui.allocate_ui_with_layout(column, Layout::top_down(Align::Center), |ui| {
        for (id, label, icon) in RAIL {
            let on = state.implemented(id) && enabled(id, &gate);
            ui.push_id(id.as_str(), |ui| {
                // A fixed-width cell, so the label lays out at the rail's width instead of at
                // whatever the slot measured on the first pass (which elides it to an ellipsis).
                let row = vec2(RAIL_WIDTH, ui.spacing().interact_size.y);
                let clicked = ui
                    .allocate_ui_with_layout(row, Layout::top_down(Align::Center), |ui| {
                        Button::new(label)
                            .icon(IconSpec::svg(IconId::new(icon)))
                            .variant(Variant::GHOST)
                            .enabled(on)
                            .show(ui)
                            .clicked()
                    })
                    .inner;
                if clicked {
                    let _ = state.act(id);
                }
            });
        }
    });
}

// ----------------------------------------------------------------- status bar

/// Show the status bar: the six count chips and the ride or notice read-out.
fn status_bar(ui: &mut Ui, state: &AppState) {
    let counters = counters(state);
    let readout = status_readout(state);
    ui.push_id("main_statusbar", |ui| {
        let _ = Bar::new("Status", BarKind::Status).show(ui, |ui, _| {
            chip(
                ui,
                "crossings_count_lbl",
                format!("Crossings {}", counters.crossings),
            );
            chip(
                ui,
                "cards_count_lbl",
                format!("Cards {}", counters.cards_dealt),
            );
            chip(
                ui,
                "on_course_lbl",
                format!("On course {}", counters.on_course),
            );
            chip(
                ui,
                "shoe_lbl",
                format!("Shoe {}/{}", counters.shoe_remaining, counters.shoe_total),
            );
            chip(
                ui,
                "riders_count_lbl",
                format!("Riders {}", counters.riders),
            );
            chip(ui, "teams_count_lbl", format!("Teams {}", counters.teams));
            let _ = Divider::vertical().show(ui);
            let _ = Text::new(readout.clone()).show(ui);
        });
    });
}

/// Show one count chip under its frozen id.
fn chip(ui: &mut Ui, id: &'static str, label: String) {
    ui.push_id(id, |ui| {
        let _ = Badge::new(label).show(ui);
    });
}

/// Return the ride's counters, or the all-zero counters with no ride.
fn counters(state: &AppState) -> Counters {
    match &state.ride {
        Some(ride) => data_source::counters(&ride.engine, &ride.roster),
        None => empty_counters(),
    }
}

/// Return the status read-out: the last notice, else the ride's state word and elapsed time.
///
/// The design's bar reads `Shift 1 · RUNNING · 02:14:07` (`design/mockups/index.html`,
/// `.app-statusbar`); this app has no shift, so the word and the elapsed time carry it. No notice
/// carries a file path: the store's paths belong in the log.
fn status_readout(state: &AppState) -> String {
    if let Some(notice) = &state.notice {
        return notice.clone();
    }
    let word = status_word(state);
    if state.ride.is_none() {
        return word;
    }
    let (elapsed, _, _) = clock_readings(state);
    format!("{word} · {}", format_duration(elapsed))
}

/// Return the ride state's word, or the no-ride word.
fn status_word(state: &AppState) -> String {
    let (status, stopped) = match &state.ride {
        Some(ride) => (Some(ride.engine.state()), ride.engine.stopped()),
        None => (None, false),
    };
    status.map_or_else(
        || NO_RIDE.to_owned(),
        |status| data_source::status_text(status, stopped),
    )
}

// ------------------------------------------------------------------ workspace

/// Show the workspace: the header, the clocks, the entry row and the two docked panels.
fn body(ui: &mut Ui, state: &mut AppState) {
    // The design's workspace is a rail column beside everything else (`design/mockups/index.html`,
    // `.app-body`): the rail spans the header, the clocks, the entry row and the docks. It is body
    // content, not a shell slot, because the slot measures its extent from the content it clips —
    // a button elided to that measure would hold the rail at the ellipsis width for ever.
    ui.horizontal_top(|ui| {
        rail(ui, state);
        let _ = Divider::vertical().show(ui);
        // The workspace takes the rest of the body, and the docks take the rest of the workspace:
        // a nested `ui.vertical` sizes to its content, which leaves the docks a strip and the
        // window a dead band under them.
        let rest = vec2(ui.available_width(), ui.available_height());
        ui.allocate_ui_with_layout(rest, Layout::top_down(Align::Min), |ui| {
            header(ui, state);
            clocks_row(ui, state);
            entry_row(ui, state);
            docks(ui, state);
        });
    });
}

/// Show the header strip: the status badge and word, the ride identity and the current lap.
fn header(ui: &mut Ui, state: &mut AppState) {
    let (light, word, lap) = ride_identity(state);
    ui.push_id("ride_status_panel", |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.push_id("ride_status_light", |ui| {
                ride_badge(ui, light, word.clone());
            });
            ui.push_id("ride_status_lbl", |ui| {
                let _ = Text::new(word.clone()).show(ui);
            });
            ride_values(ui, state);
            ui.push_id("current_lap_lbl", |ui| {
                let _ = Text::new(format!("Lap {lap}")).show(ui);
            });
        });
    });
}

/// Return the stop-light mode, the state word and the current lap.
fn ride_identity(state: &AppState) -> (StopLight, String, u32) {
    let (status, stopped, lap) = match &state.ride {
        Some(ride) => (
            Some(ride.engine.state()),
            ride.engine.stopped(),
            data_source::current_lap(ride.engine.crossings()),
        ),
        None => (None, false, 0),
    };
    let light = data_source::stop_light_mode(status, stopped);
    let word = status.map_or_else(
        || NO_RIDE.to_owned(),
        |status| data_source::status_text(status, stopped),
    );
    (light, word, lap)
}

/// Show the ride status badge in its state tone.
fn ride_badge(ui: &mut Ui, light: StopLight, word: String) {
    let variant = match light {
        StopLight::Off => Variant::look(Emphasis::Soft, Tone::Neutral),
        StopLight::Green => Variant::look(Emphasis::Soft, Tone::Good),
        StopLight::Yellow => Variant::look(Emphasis::Soft, Tone::Warn),
        StopLight::Red => Variant::look(Emphasis::Soft, Tone::Bad),
    };
    let _ = Badge::new(word).variant(variant).show(ui);
}

/// Show the six ride identity read-outs.
fn ride_values(ui: &mut Ui, state: &AppState) {
    let (name, date, venue, organizer, scorer, lap_km) = match &state.ride {
        Some(ride) => (
            ride.config.name.clone(),
            format_date(ride.config.event_date),
            ride.config.venue.clone(),
            ride.config.organizer.clone(),
            ride.config.scorer.clone(),
            format!("{:.1}", ride.config.lap_km),
        ),
        None => (
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ),
    };
    value(ui, "ride_name_value", "Ride", name);
    value(ui, "ride_date_value", "Date", date);
    value(ui, "ride_venue_value", "Venue", venue);
    value(ui, "ride_organizer_value", "Organizer", organizer);
    value(ui, "ride_scorer_value", "Scorer", scorer);
    value(ui, "ride_lap_km_value", "Lap km", lap_km);
}

/// Show one header value as its caption beside the value.
fn value(ui: &mut Ui, id: &'static str, caption: &'static str, value: String) {
    ui.push_id(id, |ui| {
        ui.vertical(|ui| {
            let _ = Text::new(caption).shade(Shade::Muted).show(ui);
            let _ = Text::new(value).show(ui);
        });
    });
}

/// Show the two flattened clock columns.
fn clocks_row(ui: &mut Ui, state: &mut AppState) {
    let (elapsed, remaining, total) = clock_readings(state);
    ui.horizontal_top(|ui| {
        clock_column(
            ui,
            ClockView {
                panel: "elapsed_clock_panel",
                clock: "elapsed_clock",
                label: "clock_elapsed_lbl",
                caption: "Elapsed",
                fraction: data_source::clock_fraction(elapsed, total),
                reading: format_duration(elapsed),
            },
        );
        clock_column(
            ui,
            ClockView {
                panel: "remaining_clock_panel",
                clock: "remaining_clock",
                label: "clock_remaining_lbl",
                caption: "Remaining",
                fraction: data_source::clock_fraction(remaining, total),
                reading: format_duration(remaining),
            },
        );
    });
}

/// Return the elapsed, remaining and planned seconds the clocks read.
fn clock_readings(state: &AppState) -> (f64, f64, f64) {
    match &state.ride {
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
    }
}

/// One clock column's frozen ids, caption, progress and read-out.
struct ClockView {
    /// The `*_clock_panel` id.
    panel: &'static str,
    /// The `*_clock` id of the progress bar.
    clock: &'static str,
    /// The `clock_*_lbl` id of the numeric read-out.
    label: &'static str,
    /// The visible caption.
    caption: &'static str,
    /// The bar's filled fraction.
    fraction: f32,
    /// The numeric read-out.
    reading: String,
}

/// Show one clock column: the caption and read-out over the progress bar.
fn clock_column(ui: &mut Ui, view: ClockView) {
    let ClockView {
        panel,
        clock,
        label,
        caption,
        fraction,
        reading,
    } = view;
    ui.push_id(panel, |ui| {
        ui.horizontal(|ui| {
            let _ = Text::new(caption).shade(Shade::Muted).show(ui);
            ui.push_id(label, |ui| {
                let _ = Text::new(reading.clone()).show(ui);
            });
        });
        ui.push_id(clock, |ui| {
            let _ = Progress::new(fraction)
                .label(caption)
                .sizing(Sizing::FillWidth)
                .show(ui);
        });
    });
}

// ------------------------------------------------------------------ entry row

/// Show the Start and Stop buttons, the plate field, Record and Undo, and the last-crossing line.
fn entry_row(ui: &mut Ui, state: &mut AppState) {
    let entry_open = state.phase() == RidePhase::Running;
    let hint = if entry_open {
        ENTRY_OPEN_HINT
    } else {
        ENTRY_CLOSED_HINT
    };
    let last = last_crossing(state);
    let gate = state.gate();
    ui.horizontal_wrapped(|ui| {
        run_buttons(ui, state, gate);
        let _ = Divider::vertical().show(ui);
        let submitted = ui
            .push_id("plate_input", |ui| {
                TextField::new(labels::PLATE_LABEL, Bind::bound(&mut state.plate))
                    .placeholder(hint)
                    .submits()
                    .enabled(entry_open)
                    .show(ui)
                    .submitted
            })
            .inner;
        if submitted {
            state.submit_plate();
        }
        ui.push_id("record_btn", |ui| {
            let clicked = Button::new(labels::RECORD_LABEL)
                .variant(Variant::PRIMARY)
                .enabled(entry_open)
                .show(ui)
                .clicked();
            if clicked {
                state.submit_plate();
            }
        });
        ui.push_id("undo_btn", |ui| {
            let clicked = Button::new(labels::UNDO_LABEL)
                .variant(Variant::SECONDARY)
                .enabled(enabled(MI_UNDO_CROSSING, &gate))
                .show(ui)
                .clicked();
            if clicked {
                let _ = state.act(MI_UNDO_CROSSING);
            }
        });
        ui.push_id("last_crossing_lbl", |ui| {
            let _ = Text::new(last).shade(Shade::Muted).show(ui);
        });
    });
}

/// Show the Start and Stop buttons, each gated by and dispatching its §15 action.
fn run_buttons(ui: &mut Ui, state: &mut AppState, gate: GateState) {
    for (id, label, variant) in RUN_BUTTONS {
        ui.push_id(id.as_str(), |ui| {
            let clicked = Button::new(label)
                .variant(variant)
                .enabled(enabled(id, &gate))
                .show(ui)
                .clicked();
            if clicked {
                let _ = state.act(id);
            }
        });
    }
}

/// Return the last-crossing line's text.
fn last_crossing(state: &AppState) -> String {
    match &state.ride {
        Some(ride) => data_source::feed_rows(&ride.engine, &ride.roster)
            .first()
            .map_or_else(|| NO_CROSSINGS.to_owned(), last_crossing_text),
        None => NO_CROSSINGS.to_owned(),
    }
}

/// Return one feed row's summary text.
fn last_crossing_text(row: &FeedRow) -> String {
    format!("Last: {} lap {}", row.plate, row.lap)
}

// ------------------------------------------------------------------- the docks

/// Show the crossings feed and the review notebook as two docked panels split by the main splitter.
fn docks(ui: &mut Ui, state: &mut AppState) {
    let split = split_width(ui);
    let total = ui.available_width();
    let review = total * REVIEW_DOCK_SHARE;
    let feed = (total - review - split).max(0.0);
    // The room left in the workspace, measured from the allotted rect: `available_height` inside a
    // content-sized parent reports the content's own height, which leaves the docks a strip and a
    // dead band under them (`design/screenshots/current-ui/console-dark-window.jpg`, D13).
    let height = (ui.max_rect().bottom() - ui.cursor().top()).max(0.0);
    ui.horizontal_top(|ui| {
        dock(ui, feed, height, |ui| feed_panel(ui, state, height));
        ui.push_id("main_splitter", |ui| {
            let _ = Splitter::new(Axis::X).label("Main splitter").show(ui);
        });
        dock(ui, review, height, |ui| review_panel(ui, state, height));
    });
}

/// Show one dock at `width` by `height`: a filling stack that offers the panel a definite room.
///
/// A `Panel` fills the room its host gives it, and a scrolling body reports that room as unbounded —
/// so without this wrapper the panel takes its content's height and leaves a dead band under it.
fn dock(ui: &mut Ui, width: f32, height: f32, mut content: impl FnMut(&mut Ui)) {
    // An inline `Panel` sizes to its content and its scrolling body reports an unbounded room, so it
    // leaves the dead band the audit calls D13. The dock draws the panel surface at the rect it is
    // given and lays the content inside it, which is the design's card (`design/mockups/index.html`,
    // `.panel`).
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let env = Env::of(ui);
    let fill = Color32::from(Srgba::from(env.tokens.color.surface.panel));
    let corners = UiTokens::corner(env.tokens.shape.radius.panel);
    ui.painter().rect_filled(rect, corners, fill);
    let line = Color32::from(Srgba::from(env.tokens.color.line.border));
    ui.painter().rect_stroke(
        rect,
        corners,
        craft::egui::Stroke::new(1.0, line),
        craft::egui::StrokeKind::Inside,
    );
    let pad = env.tokens.space.pad.regular;
    let inside = UiBuilder::new()
        .max_rect(rect.shrink(pad))
        .layout(Layout::top_down(Align::Min));
    ui.scope_builder(inside, |ui| {
        content(ui);
    });
}

/// Return the splitter handle's width from the theme's pointer tolerance.
fn split_width(ui: &Ui) -> f32 {
    craft::ui::runtime::env::Env::of(ui).tokens.size.hit.target
}

/// Show the crossings feed panel: the search field over the feed table.
fn feed_panel(ui: &mut Ui, state: &mut AppState, room: f32) {
    let rows = feed_rows(state);
    let visible = data_source::filter_rows(&rows, &state.search);
    let show_lap_time = state.settings.show_lap_time;
    let show_total = state.settings.show_total_times;
    let columns = feed_columns(show_lap_time, show_total);
    let count = visible.len();
    dock(ui, ui.available_width(), room, |ui| {
        panel_heading(ui, "Crossings", Some(count));
        ui.push_id("crossings_search", |ui| {
            let _ = TextField::search(SEARCH_LABEL, Bind::bound(&mut state.search)).show(ui);
        });
        let mut selection = Selection::<usize>::default();
        let _ = Table::<usize>::new("Crossings", columns.clone())
            .striped(true)
            .max_rows(FEED_ROWS)
            .empty(NO_CROSSINGS)
            .selection(Bind::bound(&mut selection))
            .show_items(ui, &FeedSource(&visible), &mut |key, icx| {
                let mut row = ListRow::new(feed_name(&visible, *key))
                    .cells(icx.cells())
                    .striped(icx.row.striped);
                if let Some(feed) = visible.get(*key) {
                    row = row
                        .cell(PLATE, feed.plate.clone())
                        .cell(CARD, feed.card.clone().unwrap_or_default())
                        .cell(LAP, feed.lap.to_string());
                    if show_lap_time {
                        row = row.cell(LAP_TIME, format_duration(feed.lap_time));
                    }
                    if show_total {
                        row = row.cell(TOTAL, format_duration(feed.total_time));
                    }
                }
                row
            });
        open_crossing(state, &visible, selection.lead);
    });
}

/// Show a dock's heading band: its title, and the count the design puts beside it.
fn panel_heading(ui: &mut Ui, title: &'static str, count: Option<usize>) {
    ui.horizontal(|ui| {
        let _ = Text::new(title).show(ui);
        if let Some(count) = count {
            let _ = Text::new(format!("· {count}")).shade(Shade::Muted).show(ui);
        }
    });
}

/// Return the feed rows for the open ride, or none.
fn feed_rows(state: &AppState) -> Vec<FeedRow> {
    match &state.ride {
        Some(ride) => data_source::feed_rows(&ride.engine, &ride.roster),
        None => Vec::new(),
    }
}

/// Return the feed's row label: the rider's name, or an empty label for a key off the end.
fn feed_name(rows: &[FeedRow], key: usize) -> String {
    rows.get(key)
        .map_or_else(String::new, |row| row.name.clone())
}

/// Return the feed's columns, with the two optional time columns when their settings are on.
fn feed_columns(show_lap_time: bool, show_total: bool) -> Vec<ColumnSpec> {
    let mut columns = vec![
        ColumnSpec::new(NAME, "Name", ColumnWidth::Content),
        ColumnSpec::new(PLATE, "Plate", ColumnWidth::Content),
        ColumnSpec::new(CARD, "Card", ColumnWidth::Content),
        ColumnSpec::new(LAP, "Lap", ColumnWidth::Content),
    ];
    if show_lap_time {
        columns.push(ColumnSpec::new(LAP_TIME, "Lap time", ColumnWidth::Content));
    }
    if show_total {
        columns.push(ColumnSpec::new(TOTAL, "Total", ColumnWidth::Content));
    }
    columns
}

/// Open the selected crossing's detail, when the row names an entry.
fn open_crossing(state: &mut AppState, rows: &[FeedRow], lead: Option<usize>) {
    if let Some(feed) = lead.and_then(|key| rows.get(key))
        && !feed.entry.0.is_empty()
    {
        state.open_crossing_detail(&feed.entry, feed.lap);
    }
}

/// Show the review panel: the Riders / Needs Review notebook.
fn review_panel(ui: &mut Ui, state: &mut AppState, room: f32) {
    dock(ui, ui.available_width(), room, |ui| {
        let shown = tab_index(state.console_tab);
        let out = ui
            .push_id("review_notebook", |ui| {
                TabStrip::new("Review notebook").shown(shown).show_items(
                    ui,
                    &REVIEW_TABS,
                    &mut |key, _| Tab::new(tab_title(*key)).closable(false),
                )
            })
            .inner;
        if let Some(picked) = tab_pick(&out) {
            state.console_tab = tab_of(picked);
        }
        match state.console_tab {
            ConsoleTab::Riders => riders_tab(ui, state),
            ConsoleTab::NeedsReview => review_tab(ui, state),
        }
    });
}

/// Return the tab the user picked: a selection change or an activation, else the overflow menu's pick.
fn tab_pick(out: &TabStripOutput<u8>) -> Option<u8> {
    out.response
        .events
        .iter()
        .find_map(|event| match event {
            CollectionEvent::Selection(SelectionChange::Set(keys)) => keys.last().copied(),
            CollectionEvent::Activate(key) => Some(*key),
            _ => None,
        })
        .or(out.picked)
}

/// Return the tab key the displayed tab shows.
fn tab_index(tab: ConsoleTab) -> u8 {
    match tab {
        ConsoleTab::Riders => 0,
        ConsoleTab::NeedsReview => 1,
    }
}

/// Return the tab for a picked key.
fn tab_of(key: u8) -> ConsoleTab {
    if key == 0 {
        ConsoleTab::Riders
    } else {
        ConsoleTab::NeedsReview
    }
}

/// Return a tab's title.
fn tab_title(key: u8) -> &'static str {
    if key == 0 { "Riders" } else { "Needs Review" }
}

/// Show the live roster: name, plate and team.
fn riders_tab(ui: &mut Ui, state: &mut AppState) {
    let rows = rider_rows(state);
    let mut selection = Selection::<usize>::default();
    ui.push_id("console_riders_list", |ui| {
        let _ = Table::<usize>::new("Riders", rider_columns())
            .striped(true)
            .max_rows(REVIEW_ROWS)
            .empty("No riders")
            .selection(Bind::bound(&mut selection))
            .show_items(ui, &RiderSource(&rows), &mut |key, icx| {
                let mut row = ListRow::new(rider_name(&rows, *key))
                    .cells(icx.cells())
                    .striped(icx.row.striped);
                if let Some(rider) = rows.get(*key) {
                    row = row
                        .cell(PLATE, rider.plate.clone())
                        .cell(TEAM, rider.team.clone());
                }
                row
            });
    });
    if let Some(key) = selection.lead
        && let Some(rider) = rows.get(key)
    {
        state.open_rider_at(&rider.plate);
    }
}

/// One live roster row.
struct RiderRow {
    /// The rider's resolved plate.
    plate: String,
    /// The rider's full name.
    name: String,
    /// The team's display name, or empty for a solo entry.
    team: String,
}

/// Return the live roster's flattened rows.
fn rider_rows(state: &AppState) -> Vec<RiderRow> {
    let Some(ride) = &state.ride else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for entry in RosterView::entries(&ride.roster) {
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
            rows.push(RiderRow {
                plate,
                name: rider.full_name.clone(),
                team: team.clone(),
            });
        }
    }
    rows
}

/// Return the roster table's columns.
fn rider_columns() -> Vec<ColumnSpec> {
    vec![
        ColumnSpec::new(NAME, "Name", ColumnWidth::Content),
        ColumnSpec::new(PLATE, "Plate", ColumnWidth::Content),
        ColumnSpec::new(TEAM, "Team", ColumnWidth::Content),
    ]
}

/// Return the roster's row label: the rider's name, or an empty label for a key off the end.
fn rider_name(rows: &[RiderRow], key: usize) -> String {
    rows.get(key)
        .map_or_else(String::new, |row| row.name.clone())
}

/// Show the "Needs Review" tab: the held-only filter, the review button and the flagged rows.
fn review_tab(ui: &mut Ui, state: &mut AppState) {
    let rows = flagged_rows(state);
    let first_plate = rows.first().map(|row| row.plate.clone());
    let mut selection = Selection::<usize>::default();
    let mut open_plate = None;
    ui.horizontal(|ui| {
        ui.push_id("show_held_only_chk", |ui| {
            let _ = Checkbox::new(Bind::bound(&mut state.held_only))
                .label("Show held only")
                .show(ui);
        });
        let review = ui
            .push_id("review_btn", |ui| {
                Button::new("Review…")
                    .variant(Variant::SECONDARY)
                    .enabled(first_plate.is_some())
                    .show(ui)
                    .clicked()
            })
            .inner;
        if review && let Some(plate) = first_plate.clone() {
            open_plate = Some(plate);
        }
    });
    ui.push_id("flagged_list", |ui| {
        let _ = Table::<usize>::new("Needs review", flagged_columns())
            .striped(true)
            .max_rows(REVIEW_ROWS)
            .empty("No crossings need review")
            .selection(Bind::bound(&mut selection))
            .show_items(ui, &ReviewSource(&rows), &mut |key, icx| {
                let mut row = ListRow::new(flagged_name(&rows, *key))
                    .cells(icx.cells())
                    .striped(icx.row.striped);
                if let Some(review) = rows.get(*key) {
                    row = row
                        .cell(ISSUE, review.issue.clone())
                        .cell(CARD, review.card.clone())
                        .cell(PLATE, review.plate.clone())
                        .cell(LAP, review.lap.to_string())
                        .cell(LAP_TIME, format_duration(review.lap_time));
                }
                row
            });
    });
    if let Some(key) = selection.lead
        && let Some(review) = rows.get(key)
    {
        open_plate = Some(review.plate.clone());
    }
    if let Some(plate) = open_plate {
        state.open_rider_at(&plate);
    }
}

/// Return the flagged crossing rows for the open ride, or none.
fn flagged_rows(state: &AppState) -> Vec<ReviewRow> {
    match &state.ride {
        Some(ride) => data_source::review_rows(&ride.engine, &ride.roster, state.held_only),
        None => Vec::new(),
    }
}

/// Return the flagged table's columns.
fn flagged_columns() -> Vec<ColumnSpec> {
    vec![
        ColumnSpec::new(NAME, "Rider", ColumnWidth::Content),
        ColumnSpec::new(ISSUE, "Issue", ColumnWidth::Content),
        ColumnSpec::new(CARD, "Card", ColumnWidth::Content),
        ColumnSpec::new(PLATE, "Plate", ColumnWidth::Content),
        ColumnSpec::new(LAP, "Lap", ColumnWidth::Content),
        ColumnSpec::new(LAP_TIME, "Lap time", ColumnWidth::Content),
    ]
}

/// Return the flagged table's row label: the rider's name, or an empty label for a key off the end.
fn flagged_name(rows: &[ReviewRow], key: usize) -> String {
    rows.get(key)
        .map_or_else(String::new, |row| row.rider.clone())
}

// ------------------------------------------------------------------ the sources

/// The crossings feed as a keyed collection source.
struct FeedSource<'a>(&'a [FeedRow]);

impl DisplayOrder<usize> for FeedSource<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn key(&self, at: usize) -> usize {
        at
    }

    fn index_of(&self, key: &usize) -> Option<usize> {
        (*key < self.0.len()).then_some(*key)
    }

    fn text(&self, at: usize) -> Option<Cow<'_, str>> {
        self.0.get(at).map(|row| Cow::Borrowed(row.name.as_str()))
    }
}

impl ItemSource<usize> for FeedSource<'_> {}

/// The live roster as a keyed collection source.
struct RiderSource<'a>(&'a [RiderRow]);

impl DisplayOrder<usize> for RiderSource<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn key(&self, at: usize) -> usize {
        at
    }

    fn index_of(&self, key: &usize) -> Option<usize> {
        (*key < self.0.len()).then_some(*key)
    }

    fn text(&self, at: usize) -> Option<Cow<'_, str>> {
        self.0.get(at).map(|row| Cow::Borrowed(row.name.as_str()))
    }
}

impl ItemSource<usize> for RiderSource<'_> {}

/// The flagged crossings as a keyed collection source.
struct ReviewSource<'a>(&'a [ReviewRow]);

impl DisplayOrder<usize> for ReviewSource<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn key(&self, at: usize) -> usize {
        at
    }

    fn index_of(&self, key: &usize) -> Option<usize> {
        (*key < self.0.len()).then_some(*key)
    }

    fn text(&self, at: usize) -> Option<Cow<'_, str>> {
        self.0.get(at).map(|row| Cow::Borrowed(row.rider.as_str()))
    }
}

impl ItemSource<usize> for ReviewSource<'_> {}

/// The review notebook's two tabs as a keyed collection source.
#[derive(Clone, Copy, Debug, Default)]
struct ReviewTabs;

impl DisplayOrder<u8> for ReviewTabs {
    fn len(&self) -> usize {
        2
    }

    fn key(&self, at: usize) -> u8 {
        u8::from(at != 0)
    }

    fn index_of(&self, key: &u8) -> Option<usize> {
        match key {
            0 => Some(0),
            1 => Some(1),
            _ => None,
        }
    }

    fn text(&self, at: usize) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed(tab_title(u8::from(at != 0))))
    }
}

impl ItemSource<u8> for ReviewTabs {}

/// The shared review notebook source.
const REVIEW_TABS: ReviewTabs = ReviewTabs;

// --------------------------------------------------------------------- helpers

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

/// Return *date* as the header's `YYYY-MM-DD` text.
fn format_date(date: time::Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}
