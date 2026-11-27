// SPDX-License-Identifier: GPL-3.0-only

//! macOS window presentation the eframe viewport builder cannot express.

/// Stop `AppKit` from giving the root window a tab bar.
///
/// `NSWindow.tabbingMode` defaults to automatic, so a Mac whose tabbing preference is "always"
/// puts a tab strip above the content and contributes View ▸ Hide Tab Bar and Show All Tabs to the
/// menu. The app has one window and draws its own title row, so it has nothing to tab: refusing the
/// mode drops the strip and both menu rows with it.
pub(crate) fn disallow_window_tabs() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSWindowTabbingMode};

    // eframe runs the app on the main thread; AppKit is out of reach anywhere else.
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    for window in &NSApplication::sharedApplication(mtm).windows() {
        window.setTabbingMode(NSWindowTabbingMode::Disallowed);
    }
}
