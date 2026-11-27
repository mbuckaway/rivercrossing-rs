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

/// Rename the application menu to `name`.
///
/// `AppKit` names the first menu after `NSProcessInfo.processName` — the executable,
/// `rivercrossing` — and overwrites the title a menu bar is installed with. The app is
/// `RiverCrossing`, so the title is set back after installation. Returns whether the title differed
/// and was corrected; the driver calls it once per pass, which is two cheap `AppKit` reads when the
/// title already matches.
pub(crate) fn retitle_app_menu(name: &str) -> bool {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::NSString;

    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let Some(menu) = NSApplication::sharedApplication(mtm).mainMenu() else {
        return false;
    };
    let Some(item) = menu.itemAtIndex(0) else {
        return false;
    };
    if item.title().to_string() == name {
        return false;
    }
    item.setTitle(&NSString::from_str(name));
    true
}
