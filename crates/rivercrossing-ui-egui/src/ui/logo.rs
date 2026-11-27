// SPDX-License-Identifier: GPL-3.0-only

//! The organizer logo's decoded texture: one decode per file version, shared by the header tile
//! and the setup dialog's preview.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use craft::egui::{
    Align, Color32, ColorImage, Context, Layout, Rect, Sense, TextureHandle, TextureOptions, Ui,
    UiBuilder,
};
use craft::theme::Srgba;
use craft::theme::TypeRole;
use craft::ui::contract::Show as _;
use craft::ui::display::{Shade, Text};
use craft::ui::runtime::env::Env;
use craft::ui::style::UiTokens;

use super::state::AppState;

/// The tile's side: the design's square beside the ride's statistics.
pub(crate) const TILE_SIDE: f32 = 40.0;

/// The largest file the decoder reads, so a mis-picked file cannot stall a frame.
const MAX_BYTES: u64 = 4 * 1024 * 1024;

/// The largest side the decoder accepts; a bigger image is refused rather than downsampled.
const MAX_SIDE: u32 = 1024;

/// The decoded logo textures, by path and file version.
#[derive(Default)]
pub(crate) struct LogoTextures {
    loaded: RefCell<HashMap<PathBuf, (SystemTime, TextureHandle)>>,
}

impl std::fmt::Debug for LogoTextures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogoTextures")
            .field("loaded", &self.loaded.borrow().len())
            .finish()
    }
}

impl LogoTextures {
    /// Return the texture for the logo at *path*, decoding it the first time and after it changes.
    ///
    /// A missing, unreadable, oversized or undecodable file gives `None`, so the caller keeps its
    /// placeholder.
    pub(crate) fn texture(&self, ctx: &Context, path: &Path) -> Option<TextureHandle> {
        let modified = std::fs::metadata(path).ok()?.modified().ok()?;
        if let Some((stamp, handle)) = self.loaded.borrow().get(path)
            && *stamp == modified
        {
            return Some(handle.clone());
        }
        let image = decode(path)?;
        let name = format!("logo:{}", path.display());
        let handle = ctx.load_texture(name, image, TextureOptions::LINEAR);
        self.loaded
            .borrow_mut()
            .insert(path.to_path_buf(), (modified, handle.clone()));
        Some(handle)
    }
}

/// Decode the PNG at *path* into a colour image, or `None` when it cannot be read.
fn decode(path: &Path) -> Option<ColorImage> {
    let bytes = std::fs::read(path).ok()?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_BYTES {
        return None;
    }
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).ok()?;
    if info.width > MAX_SIDE || info.height > MAX_SIDE {
        return None;
    }
    // A palette or grey image still arrives as RGBA once the reader expands it.
    let data = buffer.get(..info.buffer_size())?;
    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect(),
        _ => return None,
    };
    let size = [
        usize::try_from(info.width).ok()?,
        usize::try_from(info.height).ok()?,
    ];
    Some(ColorImage::from_rgba_unmultiplied(size, &rgba))
}

#[cfg(test)]
mod tests;

/// Show the organizer-logo tile: the decoded image, or the design's placeholder with *caption*.
///
/// The tile is `side` points square. A path that cannot be decoded keeps the placeholder, so a
/// moved or corrupt file never leaves a hole in the header.
pub(crate) fn tile(ui: &mut Ui, state: &AppState, path: Option<&Path>) {
    let (rect, _) = ui.allocate_exact_size(craft::egui::vec2(TILE_SIDE, TILE_SIDE), Sense::hover());
    let env = Env::of(ui);
    let fill = Color32::from(Srgba::from(env.tokens.color.surface.sunken));
    let line = Color32::from(Srgba::from(env.tokens.color.line.border));
    let corners = UiTokens::corner(env.tokens.shape.radius.control);
    ui.painter().rect_filled(rect, corners, fill);
    ui.painter().rect_stroke(
        rect,
        corners,
        craft::egui::Stroke::new(1.0, line),
        craft::egui::StrokeKind::Inside,
    );
    if let Some(handle) = path.and_then(|path| state.logos.texture(ui.ctx(), path)) {
        let uv = Rect::from_min_max(craft::egui::pos2(0.0, 0.0), craft::egui::pos2(1.0, 1.0));
        ui.painter()
            .image(handle.id(), rect.shrink(1.0), uv, Color32::WHITE);
        return;
    }
    let caption = path
        .and_then(|path| path.file_stem())
        .and_then(|stem| stem.to_str())
        .unwrap_or("LOGO");
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect.shrink(2.0))
            .layout(Layout::top_down(Align::Center)),
        |ui| {
            let _ = Text::new(caption.to_owned())
                .role(TypeRole::Caption)
                .elide(craft::ui::display::Elide::Middle)
                .shade(Shade::Muted)
                .show(ui);
        },
    );
}
