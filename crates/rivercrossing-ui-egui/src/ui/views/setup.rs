// SPDX-License-Identifier: GPL-3.0-only

//! The `ride_setup_dlg` form: the ride's fields, the entry and plate-model radios, the card
//! options and the card cap, collected into [`RideConfigParams`] on OK.
//!
//! Every field writes back into the open dialog's frozen field map, so the automation channel's
//! `ui.set` and the form's own widgets address the same value. The infobar shows the presenter's
//! refusal messages and the dialog stays open for the correction.

use std::path::PathBuf;

use craft::egui::{Color32, Key, Rect, Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo};
use craft::theme::Srgba;
use craft::theme::TypeRole;
use craft::theme::Variant;
use craft::theme::variant::Tone;
use craft::ui::buttons::Button;
use craft::ui::contract::{Props as _, Show as _, ShowContainer as _};
use craft::ui::display::{Badge, Shade, Text};
use craft::ui::runtime::env::{Env, Label};
use craft::ui::sections::Section;
use craft::ui::style::UiTokens;
use rivercrossing_cards::JokersMode;
use rivercrossing_ride::{EntryMode, PlateModel};

use super::dialogs::{
    field_caption, notice, pair, radio_group, stack_decimal_field, stack_select, stack_text_field,
    stack_text_field_wide, stack_whole_field, triple,
};
use crate::ui::state::AppState;

#[cfg(test)]
mod tests;

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

/// The default team size, matching the presenter's New Ride defaults.
const DEFAULT_TEAM: f64 = 4.0;

/// The default shoe size, matching the presenter's New Ride defaults.
const DEFAULT_DECKS: f64 = 8.0;

/// The default joker count, matching the presenter's New Ride defaults.
const DEFAULT_JOKERS: f64 = 1.0;

/// The persistent notice: which fields the ride's start locks.
const LOCK_NOTICE: &str =
    "Format, plate model, decks, jokers, card cap and tie-break lock once the ride starts.";

/// The logo status with no logo chosen (§15b's default word).
const STATUS_NO_LOGO: &str = "NO LOGO";

/// The logo status once a logo is picked.
const STATUS_READY: &str = "Ready";

/// The logo row's button label.
const BROWSE_LABEL: &str = "Browse…";

/// The focus ring's width, in points.
const FOCUS_RING_WIDTH: f32 = 2.0;

/// The gap between a chip and its focus ring, in points.
const FOCUS_RING_GAP: f32 = 1.0;

/// Show the ride-setup form.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState) {
    notice(ui, "setup_lock_notice", Tone::Info, LOCK_NOTICE);
    infobar(ui, state);
    let _ = Section::fixed("Ride").show(ui, |ui, _| ride_group(ui, state));
    let _ = Section::fixed("Format").show(ui, |ui, _| format_group(ui, state));
    let _ = Section::fixed("Cards").show(ui, |ui, _| cards_group(ui, state));
    let _ = Section::fixed("Tie-break").show(ui, |ui, _| tiebreak_group(ui, state));
    let _ = Section::fixed("Organizer logo").show(ui, |ui, _| logo_group(ui, state));
}

/// Show the event identity fields, two to a row as the design lays them out, and the short-lap
/// floor under them.
fn ride_group(ui: &mut Ui, state: &mut AppState) {
    pair(ui, |left, right| {
        stack_text_field(left, state, "name_input", "Ride name");
        // The mockup writes the date as `2026-06-13`, the field's own ISO spelling.
        stack_text_field(right, state, "date_picker", "Date");
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
    // The cap stands in the Cards group, where §15b places it, so the ride's last row runs full.
    stack_text_field_wide(ui, state, "min_lap_input", "Min lap (M:SS)");
}

/// Show the entry and plate-model choices and the card counts as the design's three-column grid.
fn format_group(ui: &mut Ui, state: &mut AppState) {
    triple(ui, |first, second, third| {
        entry_mode_radio(first, state);
        stack_whole_field(
            second,
            state,
            "team_size_spin",
            "Team size",
            DEFAULT_TEAM,
            2.0..=9.0,
        );
        plate_model_radio(third, state);
    });
    triple(ui, |first, second, third| {
        stack_whole_field(
            first,
            state,
            "decks_spin",
            "Decks",
            DEFAULT_DECKS,
            1.0..=20.0,
        );
        stack_whole_field(
            second,
            state,
            "jokers_spin",
            "Jokers",
            DEFAULT_JOKERS,
            0.0..=9.0,
        );
        jokers_radio(third, state);
    });
    short_laps_radio(ui, state);
}

/// Show the entry-mode radios over the live choice.
fn entry_mode_radio(ui: &mut Ui, state: &mut AppState) {
    let mixed = state.setup_radios.entry_mode == EntryMode::Mixed;
    let Some(mixed) = radio_pair(
        ui,
        "Entry mode",
        mixed,
        ("solo_radio", "Solo"),
        ("mixed_radio", "Mixed (teams)"),
    ) else {
        return;
    };
    state.setup_radios.entry_mode = if mixed {
        EntryMode::Mixed
    } else {
        EntryMode::Solo
    };
}

/// Show the plate-model radios over the live choice.
fn plate_model_radio(ui: &mut Ui, state: &mut AppState) {
    let relay = state.setup_radios.plate_model == PlateModel::TeamRelay;
    let Some(relay) = radio_pair(
        ui,
        "Plate model",
        relay,
        ("pooled_radio", "Rider plates (pooled)"),
        ("relay_radio", "Team relay"),
    ) else {
        return;
    };
    state.setup_radios.plate_model = if relay {
        PlateModel::TeamRelay
    } else {
        PlateModel::RiderPooled
    };
}

/// Show the short-lap radios over the live choice.
fn short_laps_radio(ui: &mut Ui, state: &mut AppState) {
    let always_deal = !state.setup_radios.hold_short_laps;
    let Some(always_deal) = radio_pair(
        ui,
        "Short laps",
        always_deal,
        ("hold_short_radio", "Hold short-lap cards for review"),
        ("always_deal_radio", "Always deal cards"),
    ) else {
        return;
    };
    state.setup_radios.hold_short_laps = !always_deal;
}

/// The tie-break choices, most important first as the design lists them.
const TIEBREAK_CHOICES: [(&str, &str); 3] = [
    ("Best hand", "high_card"),
    ("Laps", "laps"),
    ("Total time", "total_time"),
];

/// Show the tie-break order as the design's chips.
///
/// The order is one frozen field, `tiebreak_list`, holding the stored spellings joined by commas.
/// A chip dragged onto another's place moves there; a clicked chip, or a focused chip's Enter or
/// Space, moves one place earlier, wrapping to the end — the single-pointer and keyboard path
/// beside the drag (UX core §16).
fn tiebreak_group(ui: &mut Ui, state: &mut AppState) {
    let mut order = state.setup_tiebreak;
    field_caption(ui, "Tie-break order");
    let mut centres: Vec<f32> = Vec::new();
    let mut grabbed: Option<usize> = None;
    let mut drop_x: Option<f32> = None;
    let mut keyed: Option<(usize, usize)> = None;
    ui.horizontal_wrapped(|ui| {
        for (index, value) in order.iter().enumerate() {
            let label = tiebreak_label(value);
            let response = chip(ui, index, label);
            centres.push(response.rect.center().x);
            if response.drag_stopped() {
                // The chip that was dragged reports the stop itself, so its own place is the
                // source even though `dragged` is already false on this frame.
                grabbed = Some(index);
                drop_x = Some(
                    ui.ctx()
                        .pointer_interact_pos()
                        .map_or(response.rect.center().x, |at| at.x),
                );
            } else if response.clicked() || (response.has_focus() && activation_key(ui)) {
                keyed = Some((index, earlier(index, order.len())));
            }
        }
    });
    let target = match (grabbed, drop_x) {
        (Some(from), Some(x)) => Some((from, slot_at(&centres, x))),
        _ => keyed,
    };
    if let Some((from, to)) = target
        && from != to
    {
        order = reordered(order, from, to);
    }
    if order != state.setup_tiebreak {
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

/// Return whether the keyboard asks the focused chip to move: Enter or Space.
fn activation_key(ui: &Ui) -> bool {
    ui.input(|input| input.key_pressed(Key::Enter) || input.key_pressed(Key::Space))
}

/// Return where a chip at *at* goes when it moves one place earlier, the first to the end.
fn earlier(at: usize, len: usize) -> usize {
    if at == 0 {
        len.saturating_sub(1)
    } else {
        at - 1
    }
}

/// Return *order* with the chip at *from* moved to *to*, the rest keeping their order.
fn reordered(order: [&'static str; 3], from: usize, to: usize) -> [&'static str; 3] {
    let Some(taken) = order.get(from).copied() else {
        return order;
    };
    let rest: Vec<&'static str> = order
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(at, value)| (at != from).then_some(value))
        .collect();
    let at = to.min(rest.len());
    let moved = rest
        .iter()
        .copied()
        .take(at)
        .chain(std::iter::once(taken))
        .chain(rest.iter().copied().skip(at));
    let mut out = order;
    for (slot, value) in out.iter_mut().zip(moved) {
        *slot = value;
    }
    out
}

/// Return the chip slot a pointer at *x* names, given the chips' *centres*.
fn slot_at(centres: &[f32], x: f32) -> usize {
    centres
        .iter()
        .position(|centre| x <= *centre)
        .unwrap_or_else(|| centres.len().saturating_sub(1))
}

/// Show one tie-break chip; return the response that carries its click, focus and drag.
fn chip(ui: &mut Ui, index: usize, label: &'static str) -> Response {
    let id = ui.scope_id().with(("tiebreak_chip", index));
    let pill = pill(ui, label);
    let response = ui.interact(pill.rect, id, Sense::click_and_drag());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, label));
    if response.has_focus() {
        focus_ring(ui, response.rect);
    }
    response
}

/// Show a content-width chip: the theme's edge-only pill with its label in the caption role.
fn pill(ui: &mut Ui, label: impl Into<Label>) -> Response {
    Badge::new(label).variant(Variant::SECONDARY).show(ui)
}

/// Ring a focused chip in the theme's focus line.
fn focus_ring(ui: &Ui, rect: Rect) {
    let env = Env::of(ui);
    let corners = UiTokens::corner(env.tokens.shape.radius.pill);
    let ink = Color32::from(Srgba::from(env.tokens.color.line.focus));
    ui.painter().rect_stroke(
        rect.expand(FOCUS_RING_GAP),
        corners,
        Stroke::new(FOCUS_RING_WIDTH, ink),
        StrokeKind::Outside,
    );
}

/// Show the organizer logo row: the preview tile, the chosen file's chip, its status and Browse.
///
/// The frozen ids are the design's (`logo_preview_bmp`, `logo_status_lbl`, `logo_browse_btn`); the
/// path is kept in the dialog's state rather than its frozen field map, since the design names that
/// control outside the fields.
fn logo_group(ui: &mut Ui, state: &mut AppState) {
    let stored = state.setup_logo.clone();
    let picking = !stored.trim().is_empty();
    let path = picking.then(|| PathBuf::from(stored.trim()));
    ui.horizontal(|ui| {
        let tile_path = path.as_deref();
        ui.push_id("logo_preview_bmp", |ui| {
            crate::ui::logo::tile(ui, state, tile_path);
        });
        if let Some(name) = path
            .as_deref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
        {
            ui.add_space(Env::of(ui).tokens.space.pad.regular);
            let _ = pill(ui, name.to_owned());
        }
        ui.add_space(Env::of(ui).tokens.space.pad.regular);
        let status = if picking {
            STATUS_READY
        } else {
            STATUS_NO_LOGO
        };
        ui.push_id("logo_status_lbl", |ui| {
            let _ = Text::new(status)
                .role(TypeRole::Caption)
                .shade(Shade::Faint)
                .show(ui);
        });
        ui.add_space(Env::of(ui).tokens.space.pad.regular);
        ui.push_id("logo_browse_btn", |ui| {
            if Button::new(BROWSE_LABEL)
                .variant(Variant::SECONDARY)
                .show(ui)
                .clicked()
            {
                // The picker is the platform's, so the app raises it on its next logic pass.
                state.choose_logo = true;
            }
        });
    });
}

/// Show the card cap, the Cards group's one choice: the deck and joker counts stand in the Format
/// grid, where the mockup places them.
fn cards_group(ui: &mut Ui, state: &mut AppState) {
    stack_select(ui, state, "cap_choice", "Card cap", &CAP_CHOICES);
}

/// Show the jokers-counted radios over the live choice.
fn jokers_radio(ui: &mut Ui, state: &mut AppState) {
    let total = state.setup_radios.jokers_mode == JokersMode::Total;
    let Some(total) = radio_pair(
        ui,
        "Jokers counted",
        total,
        ("jokers_per_deck_radio", "Per deck"),
        ("jokers_total_radio", "Total"),
    ) else {
        return;
    };
    state.setup_radios.jokers_mode = if total {
        JokersMode::Total
    } else {
        JokersMode::PerDeck
    };
}

/// Show the dialog's refusal messages.
fn infobar(ui: &mut Ui, state: &AppState) {
    let Some(errors) = &state.setup_errors else {
        return;
    };
    notice(ui, "setup_infobar", Tone::Bad, errors.join("; "));
}

/// Show a pair of radios over two frozen controls; return the new choice when it changed.
///
/// The picked value comes from `AppState::setup_radios` because the frozen field map has no radio
/// kind; the frozen ids remain the widgets' ids.
fn radio_pair(
    ui: &mut Ui,
    group: &'static str,
    picked_second: bool,
    first: (&'static str, &'static str),
    second: (&'static str, &'static str),
) -> Option<bool> {
    let picked = u8::from(picked_second);
    let chosen = radio_group(
        ui,
        group,
        picked,
        &[(0, first.0, first.1), (1, second.0, second.1)],
    );
    (chosen != picked).then_some(chosen == 1)
}
