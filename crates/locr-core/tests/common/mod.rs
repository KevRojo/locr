//! Image builders shared by the integration tests.
#![allow(dead_code)]

use std::io::Cursor;

use image::{DynamicImage, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};

pub const SYNTHETIC_MODELS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/synthetic");

pub fn hello_png() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/hello.png"
    ))
    .expect("hello.png fixture missing")
}

pub fn png(img: &DynamicImage) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png).unwrap();
    out.into_inner()
}

/// Encode `upright` the way phone cameras do: pixels stored rotated 90°
/// counter-clockwise, plus EXIF orientation 6 ("rotate 90° clockwise to view").
pub fn rotated_with_exif(upright: &DynamicImage) -> Vec<u8> {
    let stored = png(&upright.rotate270());
    let ihdr_end = 8 + 4 + 4 + 13 + 4;
    let mut exif = b"II*\0".to_vec();
    exif.extend(8u32.to_le_bytes());
    exif.extend(1u16.to_le_bytes());
    exif.extend(0x0112u16.to_le_bytes()); // Orientation
    exif.extend(3u16.to_le_bytes()); // SHORT
    exif.extend(1u32.to_le_bytes());
    exif.extend(6u16.to_le_bytes());
    exif.extend([0, 0]);
    exif.extend(0u32.to_le_bytes());

    let mut body = b"eXIf".to_vec();
    body.extend(&exif);
    let mut chunk = (exif.len() as u32).to_be_bytes().to_vec();
    chunk.extend(&body);
    chunk.extend(crc32(&body).to_be_bytes());
    [&stored[..ihdr_end], &chunk, &stored[ihdr_end..]].concat()
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Black ink on a transparent background, as exported by design tools:
/// coverage goes to alpha and the color channels are zero. Flattened onto
/// white this reproduces the greyscale of `img` exactly.
pub fn transparent_ink(img: &DynamicImage) -> DynamicImage {
    let grey = img.to_luma8();
    let mut out = RgbaImage::new(grey.width(), grey.height());
    for (src, dst) in grey.pixels().zip(out.pixels_mut()) {
        *dst = Rgba([0, 0, 0, 255 - src[0]]);
    }
    DynamicImage::ImageRgba8(out)
}

/// White page with one row of words per entry of `rows`; each number is the
/// glyph count of a word. Glyphs are dark grey blocks the synthetic models
/// detect as text.
pub fn synthetic_page(rows: &[&[u32]]) -> DynamicImage {
    let mut img = RgbImage::from_pixel(600, 40 + rows.len() as u32 * 70, Rgb([255, 255, 255]));
    for (r, words) in rows.iter().enumerate() {
        let y0 = 30 + r as u32 * 70;
        let mut x = 20;
        for (w, &glyphs) in words.iter().enumerate() {
            for g in 0..glyphs {
                let grey = 20 + ((r as u32 * 31 + w as u32 * 17 + g * 11) % 90) as u8;
                for y in y0..y0 + 22 {
                    for gx in x..x + 12 {
                        img.put_pixel(gx, y, Rgb([grey, grey, grey]));
                    }
                }
                x += 16;
            }
            x += 30;
        }
    }
    DynamicImage::ImageRgb8(img)
}
