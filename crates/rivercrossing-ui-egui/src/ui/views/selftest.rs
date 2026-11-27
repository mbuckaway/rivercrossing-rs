// SPDX-License-Identifier: GPL-3.0-only

//! The evaluator self-test's read-only mono output in its own scrolling panel.

use craft::egui::{Align, Color32, Layout, Rect, ScrollArea, Sense, Ui, UiBuilder, vec2};
use craft::theme::Srgba;
use craft::ui::contract::Show as _;
use craft::ui::display::Text;
use craft::ui::runtime::env::Env;
use craft::ui::style::{TypeRole, UiTokens};

use crate::ui::state::AppState;

#[cfg(test)]
mod tests;

/// The report panel's height before it scrolls, as the design caps the report at 280 points
/// (`design/mockups/index.html`, `#s-selftest` `max-height:280px`).
pub(crate) const REPORT_HEIGHT: f32 = 280.0;

/// Show the self-test lines in a bounded panel that scrolls.
pub(crate) fn body(ui: &mut Ui, state: &mut AppState) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, REPORT_HEIGHT), Sense::hover());
    surface(ui, rect);
    let pad = Env::of(ui).tokens.space.pad.regular;
    let inside = UiBuilder::new()
        .max_rect(rect.shrink(pad))
        .layout(Layout::top_down(Align::Min));
    ui.scope_builder(inside, |ui| {
        ScrollArea::vertical()
            .id_salt("selftest_scroll")
            .show(ui, |ui| {
                ui.push_id("selftest_output", |ui| {
                    for line in &state.selftest_lines {
                        let _ = Text::new(line.clone()).role(TypeRole::Mono).show(ui);
                    }
                });
            });
    });
}

/// Paint the panel surface: the design's fill, hairline and corner radius.
fn surface(ui: &Ui, rect: Rect) {
    let env = Env::of(ui);
    let corners = UiTokens::corner(env.tokens.shape.radius.panel);
    let fill = Color32::from(Srgba::from(env.tokens.color.surface.panel));
    let line = Color32::from(Srgba::from(env.tokens.color.line.border));
    ui.painter().rect_filled(rect, corners, fill);
    ui.painter().rect_stroke(
        rect,
        corners,
        craft::egui::Stroke::new(1.0, line),
        craft::egui::StrokeKind::Inside,
    );
}
