// SPDX-License-Identifier: GPL-3.0-only

//! The `ride_setup_dlg` form: the ride's fields, the entry and plate-model radios, the shoe
//! options and the card cap, collected into [`RideConfigParams`] on OK.
//!
//! Every field writes back into the open dialog's frozen field map, so the automation channel's
//! `ui.set` and the form's own widgets address the same value. The infobar shows the presenter's
//! refusal messages and the dialog stays open for the correction.

use craft::egui::Ui;
use craft::theme::TypeRole;
use craft::theme::Variant;
use craft::theme::variant::Tone;
use craft::ui::buttons::Button;
use craft::ui::contract::{Bind, Props as _, Show as _, ShowContainer as _};
use craft::ui::display::{Shade, Text};
use craft::ui::runtime::env::Env;
use craft::ui::sections::Section;
use craft::ui::text::TextField;

use super::dialogs::{
    field_caption, notice, radio_group, stack_date_field, stack_decimal_field, stack_select,
    stack_text_field, stack_whole_field,
};
use crate::ui::state::AppState;

/// The `cap_choice` items: no cap, then one per cap in 5..=20.
const CAP_CHOICES: [(&str, &str); 17] = [
    ("Disabled", "Disabled"),
    ("5", "5"),
    ("6", "6"),
    ("7", "7"),
    ("8", "8"),
    ("9", "9"),
    ("10", "10"),
    ("11", "11"),
    ("12", "12"),
    ("13", "13"),
    ("14", "14"),
    ("15", "15"),
    ("16", "16"),
    ("17", "17"),
    ("18", "18"),
    ("19", "19"),
    ("20", "20"),
];

/// The default lap length in kilometres, matching the presenter's New Ride defaults.
const DEFAULT_LAP_KM: f64 = 8.0;

/// The default shoe size, matching the presenter's New Ride defaults.
const DEFAULT_DECKS: f64 = 8.0;

/// The default joker count, matching the presenter's New Ride defaults.
const DEFAULT_JOKERS: f64 = 1.0;

/// Show the ride-setup form.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState) {
    infobar(ui, state);
    let _ = Section::fixed("Ride").show(ui, |ui, _| ride_group(ui, state));
    let _ = Section::fixed("Format").show(ui, |ui, _| format_group(ui, state));
    let _ = Section::fixed("Tie-break").show(ui, |ui, _| tiebreak_group(ui, state));
    let _ = Section::fixed("Organizer logo").show(ui, |ui, _| logo_group(ui, state));
    let _ = Section::fixed("Shoe").show(ui, |ui, _| shoe_group(ui, state));
}

/// Show the event identity fields, two to a row as the design lays them out.
fn ride_group(ui: &mut Ui, state: &mut AppState) {
    pair(ui, |left, right| {
        stack_text_field(left, state, "name_input", "Ride name");
        stack_date_field(right, state, "date_picker", "Date");
    });
    pair(ui, |left, right| {
        stack_text_field(left, state, "start_time_picker", "Start time");
        stack_text_field(right, state, "venue_input", "Venue");
    });
    pair(ui, |left, right| {
        stack_text_field(left, state, "organizer_input", "Organizer");
        stack_text_field(right, state, "scorer_input", "Scorer");
    });
    pair(ui, |left, right| {
        stack_decimal_field(
            left,
            state,
            "lap_km_spin",
            "Lap length (km)",
            DEFAULT_LAP_KM,
            0.1..=100.0,
        );
        stack_text_field(right, state, "duration_input", "Duration (H:MM)");
    });
    pair(ui, |left, right| {
        stack_text_field(left, state, "min_lap_input", "Min lap (M:SS)");
        stack_select(right, state, "cap_choice", "Hand cap", &CAP_CHOICES);
    });
}

/// Show two fields side by side: the design's two-column form (`design/mockups/index.html`,
/// `.grid2`).
fn pair(ui: &mut Ui, content: impl FnOnce(&mut Ui, &mut Ui)) {
    ui.columns(2, |columns| {
        let (left, right) = columns.split_at_mut(1);
        if let (Some(left), Some(right)) = (left.first_mut(), right.first_mut()) {
            content(left, right);
        }
    });
}

/// Show the entry and plate-model choices, two to a row.
fn format_group(ui: &mut Ui, state: &mut AppState) {
    pair(ui, |left, right| {
        radio_pair(
            left,
            state,
            "Entry mode",
            ("solo_radio", "Solo"),
            ("mixed_radio", "Mixed (teams)"),
        );
        stack_whole_field(
            right,
            state,
            "team_size_spin",
            "Largest team",
            4.0,
            2.0..=9.0,
        );
    });
    radio_pair(
        ui,
        state,
        "Plate model",
        ("pooled_radio", "Rider plates (pooled)"),
        ("relay_radio", "Team relay"),
    );
    radio_pair(
        ui,
        state,
        "Short laps",
        ("hold_short_radio", "Hold short-lap cards for review"),
        ("always_deal_radio", "Always deal cards"),
    );
}

/// The tie-break choices, most important first as the design lists them.
const TIEBREAK_CHOICES: [(&str, &str); 3] = [
    ("Best hand", "high_card"),
    ("Laps", "laps"),
    ("Total time", "total_time"),
];

/// Show the tie-break order as the design's chips, dragged into place.
///
/// The order is one frozen field, `tiebreak_list`, holding the stored spellings joined by commas.
/// A chip dragged onto another's place moves there, and the whole list is written back.
fn tiebreak_group(ui: &mut Ui, state: &mut AppState) {
    let mut order = state.setup_tiebreak;
    let before = order;
    field_caption(ui, "Tie-break order");
    let mut grabbed: Option<usize> = None;
    let mut dropped: Option<usize> = None;
    let mut row_left = 0.0;
    ui.horizontal(|ui| {
        row_left = ui.cursor().left();
        for (index, value) in order.iter().enumerate() {
            let label = tiebreak_label(value);
            let (rect, response) = ui.allocate_exact_size(
                craft::egui::vec2(CHIP_WIDTH, CHIP_HEIGHT),
                craft::egui::Sense::click_and_drag(),
            );
            let active = response.hovered() || response.dragged();
            chip(ui, rect, label, active);
            response.widget_info(|| {
                craft::egui::WidgetInfo::labeled(craft::egui::Role::Button, true, label)
            });
            if response.drag_stopped() {
                // The chip that was dragged reports the stop itself, so its own place is the
                // source even though `dragged` is already false on this frame.
                grabbed = Some(index);
                let x = ui
                    .ctx()
                    .pointer_interact_pos()
                    .map_or(rect.center().x, |at| at.x);
                dropped = Some(slot_at(row_left, x));
            }
            ui.add_space(CHIP_GAP);
        }
    });
    if let (Some(from), Some(to)) = (grabbed, dropped)
        && from != to
    {
        let mut moved: Vec<&'static str> = order.to_vec();
        let taken = moved.remove(from);
        moved.insert(to.min(moved.len()), taken);
        order.copy_from_slice(&moved);
    }
    if order != before {
        state.setup_tiebreak = order;
    }
}

/// Return the chip's word for a stored tie-break spelling.
fn tiebreak_label(value: &'static str) -> &'static str {
    TIEBREAK_CHOICES
        .iter()
        .find(|(_, stored)| *stored == value)
        .map_or(value, |(name, _)| *name)
}

/// Return the chip slot a pointer at *x* names, given the row's left edge.
fn slot_at(row_left: f32, x: f32) -> usize {
    let stride = CHIP_WIDTH + CHIP_GAP;
    let slot = ((x - row_left) / stride).round();
    if slot <= 0.0 {
        return 0;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a chip slot is a small count of three, and the negative case returned above"
    )]
    let index = slot as usize;
    index.min(2)
}

/// Paint one chip: the design's pill, filled while it is hovered or dragged.
fn chip(ui: &mut Ui, rect: craft::egui::Rect, label: &'static str, active: bool) {
    let env = Env::of(ui);
    let corners = craft::ui::style::UiTokens::corner(env.tokens.shape.radius.control);
    let fill = if active {
        craft::egui::Color32::from(craft::theme::Srgba::from(env.tokens.color.state.hover))
    } else {
        craft::egui::Color32::from(craft::theme::Srgba::from(env.tokens.color.surface.panel))
    };
    let line = craft::egui::Color32::from(craft::theme::Srgba::from(env.tokens.color.line.border));
    ui.painter().rect_filled(rect, corners, fill);
    ui.painter().rect_stroke(
        rect,
        corners,
        craft::egui::Stroke::new(1.0, line),
        craft::egui::StrokeKind::Inside,
    );
    // Painted at the chip's centre: a text widget in a fixed cell keeps to its start edge.
    let ink = craft::egui::Color32::from(craft::theme::Srgba::from(env.tokens.color.text.normal));
    ui.painter().text(
        rect.center(),
        craft::egui::Align2::CENTER_CENTER,
        label,
        craft::egui::FontId::proportional(13.0),
        ink,
    );
}

/// One tie-break chip's width.
const CHIP_WIDTH: f32 = 140.0;

/// One tie-break chip's height.
const CHIP_HEIGHT: f32 = 28.0;

/// The gap between two chips.
const CHIP_GAP: f32 = 8.0;

/// Show the organizer logo row: the chosen file, its preview and state, and the browse button.
///
/// The frozen ids are the design's (`logo_preview_bmp`, `logo_status_lbl`, `logo_browse_btn`); the
/// path is kept in the dialog's state rather than its frozen field map, since the design names that
/// control outside the fields.
fn logo_group(ui: &mut Ui, state: &mut AppState) {
    logo_path_field(ui, state);
    ui.horizontal(|ui| {
        let stored = state.setup_logo.clone();
        let path = std::path::PathBuf::from(stored.trim());
        let chosen = (!stored.trim().is_empty()).then_some(path.as_path());
        crate::ui::logo::tile(ui, state, chosen);
        ui.add_space(Env::of(ui).tokens.space.pad.regular);
        let status = if stored.trim().is_empty() {
            "No logo chosen"
        } else {
            "Ready"
        };
        ui.push_id("logo_status_lbl", |ui| {
            let _ = Text::new(status.to_owned())
                .role(TypeRole::Caption)
                .shade(Shade::Muted)
                .show(ui);
        });
        ui.push_id("logo_browse_btn", |ui| {
            if Button::new("Browse…")
                .variant(Variant::SECONDARY)
                .show(ui)
                .clicked()
            {
                state.note(crate::ui::state::FILE_PICKER_UNAVAILABLE);
            }
        });
    });
}

/// Show the logo path under its caption, reading and writing the dialog's state.
fn logo_path_field(ui: &mut Ui, state: &mut AppState) {
    let mut value = state.setup_logo.clone();
    ui.push_id("logo_preview_bmp", |ui| {
        field_caption(ui, "Logo file");
        ui.set_max_width(420.0);
        let _ = TextField::new("Logo file", Bind::bound(&mut value)).show(ui);
    });
    if value != state.setup_logo {
        state.setup_logo = value;
    }
}

/// Show the shoe's options, two to a row where the design pairs them.
fn shoe_group(ui: &mut Ui, state: &mut AppState) {
    pair(ui, |left, right| {
        stack_whole_field(
            left,
            state,
            "decks_spin",
            "Decks",
            DEFAULT_DECKS,
            1.0..=20.0,
        );
        stack_whole_field(
            right,
            state,
            "jokers_spin",
            "Jokers",
            DEFAULT_JOKERS,
            0.0..=9.0,
        );
    });
    radio_pair(
        ui,
        state,
        "Jokers counted",
        ("jokers_per_deck_radio", "Jokers per deck"),
        ("jokers_total_radio", "Jokers total"),
    );
    stack_select(ui, state, "cap_choice", "Card cap", &CAP_CHOICES);
}

/// Show the dialog's refusal messages.
fn infobar(ui: &mut Ui, state: &AppState) {
    let Some(errors) = &state.setup_errors else {
        return;
    };
    notice(ui, "setup_infobar", Tone::Bad, errors.join("; "));
}

/// Show a pair of radios over two frozen controls; picking one clears the other.
fn radio_pair(
    ui: &mut Ui,
    state: &mut AppState,
    group: &'static str,
    first: (&'static str, &'static str),
    second: (&'static str, &'static str),
) {
    let picked = u8::from(state.dialogs.field(second.0) == Some("true"));
    let chosen = radio_group(
        ui,
        group,
        picked,
        &[(0, first.0, first.1), (1, second.0, second.1)],
    );
    if chosen == picked {
        return;
    }
    let _ = state.dialogs.set_field(first.0, bool_word(chosen == 0));
    let _ = state.dialogs.set_field(second.0, bool_word(chosen == 1));
}

/// Return the stored word of a flag.
fn bool_word(on: bool) -> &'static str {
    if on { "true" } else { "false" }
}
