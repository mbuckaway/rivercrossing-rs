// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the logo decoder.

use pretty_assertions::assert_eq;

use super::decode;

/// Write a `width` by `height` RGBA PNG of one colour to *path*.
fn write_png(path: &std::path::Path, width: u32, height: u32) {
    let file = std::fs::File::create(path).expect("a temporary file");
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("a png header");
    let pixels: Vec<u8> = (0..width * height)
        .flat_map(|_| [10, 20, 30, 255])
        .collect();
    writer.write_image_data(&pixels).expect("the pixel data");
}

#[test]
fn test_decode_reads_a_png_at_its_size() {
    // Arrange
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("logo.png");
    write_png(&path, 4, 3);

    // Act
    let image = decode(&path).expect("the png decodes");

    // Assert
    assert_eq!((image.width(), image.height()), (4, 3));
    assert_eq!(
        image.pixels[0],
        craft::egui::Color32::from_rgba_unmultiplied(10, 20, 30, 255)
    );
}

#[test]
fn test_decode_refuses_a_file_that_is_not_a_png() {
    // Arrange
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("logo.png");
    std::fs::write(&path, b"not a png").expect("the file writes");

    // Act
    let image = decode(&path);

    // Assert
    assert!(image.is_none());
}

#[test]
fn test_decode_refuses_a_missing_file() {
    // Act
    let image = decode(std::path::Path::new("/nonexistent/logo.png"));

    // Assert
    assert!(image.is_none());
}
