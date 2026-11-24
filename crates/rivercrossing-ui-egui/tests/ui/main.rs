// SPDX-License-Identifier: GPL-3.0-only

//! The `ui` snapshot target.
//!
//! PR B replaces this harness smoke with the committed shell golden. Keeping a real
//! render pass here proves the `egui_kittest` target and the `-P ui` profile work
//! before the golden policy lands.

#[test]
fn kittest_harness_settles_a_frame() {
    let mut harness = egui_kittest::Harness::new_ui(|ui| {
        ui.label("RiverCrossing");
    });

    assert!(harness.run_ok().is_some(), "the harness settles");
}
