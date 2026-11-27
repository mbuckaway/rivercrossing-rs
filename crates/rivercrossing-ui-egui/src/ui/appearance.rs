// SPDX-License-Identifier: GPL-3.0-only

//! The `ArtCraft` `RootEnv` that the persisted appearance settings describe.
//!
//! The mode, contrast, density, text size and theme are the persisted settings; the frozen
//! `mi_zoom_*` actions stay a separate zoom axis on `ctx.set_zoom_factor`.

use craft::theme::{ContrastChoice, ModeChoice, Size, TextSize, ThemeId};
use craft::ui::runtime::env::RootEnv;
use rivercrossing_store::settings::AppSettings;

#[cfg(test)]
mod tests;

/// Return the root env that *settings* describes.
#[must_use]
pub(crate) fn root_env(settings: &AppSettings) -> RootEnv {
    let mut root = RootEnv::new()
        .mode(mode(&settings.appearance))
        .contrast(contrast(&settings.contrast))
        .density(density(&settings.density))
        .theme(theme(&settings.theme));
    root.text_size = text_size(&settings.text_size);
    root
}

/// Return the mode choice for a stored spelling; an unknown spelling follows the host.
#[must_use]
pub(crate) fn mode(spelling: &str) -> ModeChoice {
    match spelling {
        "light" => ModeChoice::Light,
        "dark" => ModeChoice::Dark,
        _ => ModeChoice::System,
    }
}

/// Return the contrast choice for a stored spelling; an unknown spelling follows the host.
#[must_use]
pub(crate) fn contrast(spelling: &str) -> ContrastChoice {
    match spelling {
        "standard" => ContrastChoice::Standard,
        "high" => ContrastChoice::High,
        _ => ContrastChoice::System,
    }
}

/// Return the density preset for a stored spelling; an unknown spelling is compact.
#[must_use]
pub(crate) fn density(spelling: &str) -> Size {
    match spelling {
        "comfortable" => Size::Comfortable,
        _ => Size::Compact,
    }
}

/// Return the text-size step for a stored spelling; an unknown spelling is medium.
#[must_use]
pub(crate) fn text_size(spelling: &str) -> TextSize {
    match spelling {
        "tiny" => TextSize::Tiny,
        "small" => TextSize::Small,
        "large" => TextSize::Large,
        _ => TextSize::Medium,
    }
}

/// Return the theme id for a stored id; an id that cannot parse is the suite default.
#[must_use]
pub(crate) fn theme(id: &str) -> ThemeId {
    ThemeId::parse(id).unwrap_or_else(|_| ThemeId::suite_default())
}
