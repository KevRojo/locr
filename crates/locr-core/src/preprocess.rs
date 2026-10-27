//! Image decoding and pre-OCR normalization.

use std::borrow::Cow;
use std::io::Cursor;

use image::metadata::Orientation;
use image::{DynamicImage, GrayImage, ImageDecoder, ImageReader, RgbImage, RgbaImage};

use crate::{LocrError, Result, TransformUsed};

/// Decode encoded image bytes (PNG, JPEG, WEBP, BMP, TIFF, GIF, ...).
///
/// EXIF orientation is applied, so phone photos come out upright and
/// bounding boxes match what image viewers display.
pub fn decode_image(bytes: &[u8]) -> Result<DynamicImage> {
    let decode_err = |e: image::ImageError| LocrError::DecodeError(e.to_string());
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| LocrError::DecodeError(e.to_string()))?;
    let mut decoder = reader.into_decoder().map_err(decode_err)?;
    // Corrupt metadata must not make an otherwise readable image fail.
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder).map_err(decode_err)?;
    img.apply_orientation(orientation);
    Ok(img)
}

/// Wrap raw 8-bit pixels (row-major, no row padding) as an image.
///
/// The channel count is inferred from the buffer length: 1 = greyscale,
/// 3 = RGB, 4 = RGBA (e.g. an HTML canvas `ImageData`).
pub fn image_from_pixels(width: u32, height: u32, pixels: Vec<u8>) -> Result<DynamicImage> {
    let invalid = || {
        LocrError::InvalidInput(format!(
            "{} bytes is not a {width}x{height} greyscale, RGB or RGBA image",
            pixels.len()
        ))
    };
    let area = (width as usize)
        .checked_mul(height as usize)
        .filter(|&area| area > 0)
        .ok_or_else(invalid)?;
    if !pixels.len().is_multiple_of(area) {
        return Err(invalid());
    }
    let image = match pixels.len() / area {
        1 => GrayImage::from_raw(width, height, pixels).map(DynamicImage::ImageLuma8),
        3 => RgbImage::from_raw(width, height, pixels).map(DynamicImage::ImageRgb8),
        4 => RgbaImage::from_raw(width, height, pixels).map(DynamicImage::ImageRgba8),
        _ => None,
    };
    image.ok_or_else(|| LocrError::InvalidInput(format!("pixel buffer does not match {width}x{height}")))
}

/// Remove transparency by compositing onto a background that contrasts with
/// the visible content.
///
/// OCR engines read color channels only. Transparent pixels are usually
/// stored as black, so black text on a transparent background would vanish
/// into a black page. Dark content gets a white background, light content a
/// black one.
pub(crate) fn flatten_alpha(image: &DynamicImage) -> Cow<'_, DynamicImage> {
    if !image.color().has_alpha() {
        return Cow::Borrowed(image);
    }
    let rgba = match image {
        DynamicImage::ImageRgba8(rgba) => Cow::Borrowed(rgba),
        other => Cow::Owned(other.to_rgba8()),
    };

    let (mut ink, mut coverage) = (0.0f64, 0.0f64);
    for p in rgba.pixels() {
        let alpha = p[3] as f64 / 255.0;
        ink += alpha * luma(p[0], p[1], p[2]);
        coverage += alpha;
    }
    let ink = if coverage > 0.0 { ink / coverage } else { 0.0 };
    let bg: u32 = if ink < 128.0 { 255 } else { 0 };

    let mut rgb = RgbImage::new(rgba.width(), rgba.height());
    for (src, dst) in rgba.pixels().zip(rgb.pixels_mut()) {
        let a = src[3] as u32;
        for c in 0..3 {
            dst[c] = ((src[c] as u32 * a + bg * (255 - a) + 127) / 255) as u8;
        }
    }
    Cow::Owned(DynamicImage::ImageRgb8(rgb))
}

/// Rec. 601 luma, the weighting the ocrs models were trained with.
fn luma(r: u8, g: u8, b: u8) -> f64 {
    0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64
}

/// An image transform tried by the auto-enhance loop.
pub(crate) type Variant = fn(&RgbImage) -> DynamicImage;

/// Ordered list of (label, transform) candidates for the enhance loop.
pub(crate) fn enhance_candidates() -> Vec<(TransformUsed, Variant)> {
    use image::imageops::{brighten, contrast, grayscale};
    let candidates: [(TransformUsed, Variant); 8] = [
        (TransformUsed::Contrast(30.0), |img| DynamicImage::ImageRgb8(contrast(img, 30.0))),
        (TransformUsed::Contrast(55.0), |img| DynamicImage::ImageRgb8(contrast(img, 55.0))),
        (TransformUsed::Brightness(25), |img| DynamicImage::ImageRgb8(brighten(img, 25))),
        (TransformUsed::Brightness(-25), |img| DynamicImage::ImageRgb8(brighten(img, -25))),
        (TransformUsed::Saturation(1.6), |img| DynamicImage::ImageRgb8(adjust_saturation(img, 1.6))),
        (TransformUsed::GrayscaleContrast(40.0), |img| {
            DynamicImage::ImageLuma8(contrast(&grayscale(img), 40.0))
        }),
        (TransformUsed::Invert, |img| {
            let mut inverted = img.clone();
            image::imageops::invert(&mut inverted);
            DynamicImage::ImageRgb8(inverted)
        }),
        (TransformUsed::Combo("sat1.4+contrast40".into()), |img| {
            DynamicImage::ImageRgb8(contrast(&adjust_saturation(img, 1.4), 40.0))
        }),
    ];
    candidates.into()
}

/// Multiply saturation around the pixel's luma. factor=1.0 is identity, >1 more vivid.
fn adjust_saturation(img: &RgbImage, factor: f32) -> RgbImage {
    let mut out = img.clone();
    for p in out.pixels_mut() {
        let [r, g, b] = p.0.map(f32::from);
        // Luma (Rec. 601)
        let y = 0.299 * r + 0.587 * g + 0.114 * b;
        p.0 = [r, g, b].map(|c| (y + (c - y) * factor).clamp(0.0, 255.0) as u8);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgb, Rgba};

    fn encode(img: &DynamicImage, format: ImageFormat) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, format).unwrap();
        out.into_inner()
    }

    /// 3x2 image with a red pixel at the top-left corner.
    fn marker() -> DynamicImage {
        let mut img = RgbImage::from_pixel(3, 2, Rgb([255, 255, 255]));
        img.put_pixel(0, 0, Rgb([255, 0, 0]));
        DynamicImage::ImageRgb8(img)
    }

    /// Minimal little-endian TIFF/Exif blob with a single Orientation tag.
    fn exif_orientation(value: u16) -> Vec<u8> {
        let mut exif = b"II*\0".to_vec();
        exif.extend(8u32.to_le_bytes()); // offset of IFD0
        exif.extend(1u16.to_le_bytes()); // one entry
        exif.extend(0x0112u16.to_le_bytes()); // Orientation
        exif.extend(3u16.to_le_bytes()); // SHORT
        exif.extend(1u32.to_le_bytes()); // count
        exif.extend(value.to_le_bytes());
        exif.extend([0, 0]); // value padding
        exif.extend(0u32.to_le_bytes()); // no next IFD
        exif
    }

    /// Insert an `eXIf` chunk right after `IHDR`.
    fn png_with_exif(png: &[u8], exif: &[u8]) -> Vec<u8> {
        let ihdr_end = 8 + 4 + 4 + 13 + 4;
        let mut chunk = Vec::new();
        chunk.extend((exif.len() as u32).to_be_bytes());
        let mut body = b"eXIf".to_vec();
        body.extend(exif);
        chunk.extend(&body);
        chunk.extend(crc32(&body).to_be_bytes());
        [&png[..ihdr_end], &chunk, &png[ihdr_end..]].concat()
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &b in bytes {
            crc ^= b as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
            }
        }
        !crc
    }

    #[test]
    fn decodes_common_formats() {
        for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::Bmp, ImageFormat::Tiff] {
            let img = decode_image(&encode(&marker(), format)).unwrap();
            assert_eq!((img.width(), img.height()), (3, 2), "{format:?}");
        }
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(decode_image(b"not an image"), Err(LocrError::DecodeError(_))));
        assert!(matches!(decode_image(&[]), Err(LocrError::DecodeError(_))));
    }

    #[test]
    fn applies_exif_orientation() {
        let png = encode(&marker(), ImageFormat::Png);
        // 6 = "rotate 90° clockwise to display upright".
        let rotated = decode_image(&png_with_exif(&png, &exif_orientation(6))).unwrap();
        assert_eq!((rotated.width(), rotated.height()), (2, 3));
        let rgb = rotated.to_rgb8();
        assert_eq!(rgb.get_pixel(1, 0).0, [255, 0, 0], "top-left marker moves to top-right");

        let upright = decode_image(&png_with_exif(&png, &exif_orientation(1))).unwrap();
        assert_eq!(upright.to_rgb8().get_pixel(0, 0).0, [255, 0, 0]);
    }

    #[test]
    fn pixels_infer_channels() {
        assert!(image_from_pixels(2, 2, vec![0; 4]).unwrap().as_luma8().is_some());
        assert!(image_from_pixels(2, 2, vec![0; 12]).unwrap().as_rgb8().is_some());
        assert!(image_from_pixels(2, 2, vec![0; 16]).unwrap().as_rgba8().is_some());
        assert!(matches!(image_from_pixels(2, 2, vec![0; 8]), Err(LocrError::InvalidInput(_))));
        assert!(matches!(image_from_pixels(2, 2, vec![0; 5]), Err(LocrError::InvalidInput(_))));
        assert!(matches!(image_from_pixels(0, 2, vec![]), Err(LocrError::InvalidInput(_))));
    }

    #[test]
    fn opaque_images_are_untouched() {
        let img = marker();
        assert!(matches!(flatten_alpha(&img), Cow::Borrowed(_)));
    }

    #[test]
    fn dark_ink_on_transparent_gets_white_background() {
        let mut rgba = RgbaImage::from_pixel(4, 1, Rgba([0, 0, 0, 0]));
        rgba.put_pixel(0, 0, Rgba([10, 10, 10, 255]));
        rgba.put_pixel(1, 0, Rgba([0, 0, 0, 128])); // anti-aliased edge
        let flat = flatten_alpha(&DynamicImage::ImageRgba8(rgba)).to_rgb8();
        assert_eq!(flat.get_pixel(0, 0).0, [10, 10, 10]);
        assert_eq!(flat.get_pixel(1, 0).0, [127, 127, 127]);
        assert_eq!(flat.get_pixel(2, 0).0, [255, 255, 255]);
    }

    #[test]
    fn light_ink_on_transparent_gets_black_background() {
        // Hidden white color under alpha=0 must not leak through.
        let mut rgba = RgbaImage::from_pixel(4, 1, Rgba([255, 255, 255, 0]));
        rgba.put_pixel(0, 0, Rgba([250, 250, 250, 255]));
        let flat = flatten_alpha(&DynamicImage::ImageRgba8(rgba)).to_rgb8();
        assert_eq!(flat.get_pixel(0, 0).0, [250, 250, 250]);
        assert_eq!(flat.get_pixel(3, 0).0, [0, 0, 0]);
    }

    #[test]
    fn grey_alpha_is_flattened_too() {
        let mut la = image::GrayAlphaImage::from_pixel(2, 1, image::LumaA([0, 0]));
        la.put_pixel(0, 0, image::LumaA([0, 255]));
        let img = DynamicImage::ImageLumaA8(la);
        let flat = flatten_alpha(&img);
        assert_eq!(flat.as_rgb8().unwrap().get_pixel(1, 0).0, [255, 255, 255]);
    }

    #[test]
    fn enhance_candidates_preserve_size() {
        let rgb = marker().to_rgb8();
        for (transform, variant) in enhance_candidates() {
            let out = variant(&rgb);
            assert_eq!((out.width(), out.height()), (3, 2), "{transform}");
        }
    }

    #[test]
    fn saturation_identity() {
        let rgb = marker().to_rgb8();
        assert_eq!(adjust_saturation(&rgb, 1.0), rgb);
    }
}
