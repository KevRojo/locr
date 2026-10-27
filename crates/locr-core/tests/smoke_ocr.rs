//! Real end-to-end OCR smoke tests against the bundled ocrs models.
//!
//! Run with: `cargo test -p locr-core --test smoke_ocr -- --nocapture`
#![cfg(feature = "bundle-models")]

mod common;

use common::{hello_png, png, rotated_with_exif, transparent_ink};
use locr_core::{decode_image, score_ocr_text, EnhanceOptions, OcrOptions, TransformUsed};

#[test]
fn ocr_reads_hello_locr() {
    let locr = locr_core::shared().expect("engine init");
    let text = locr.image_to_text(&hello_png()).expect("ocr failed");
    eprintln!("OCR text: {:?}", text);
    let upper = text.to_uppercase();
    assert!(
        upper.contains("HELLO") || upper.contains("LOCR"),
        "expected HELLO or LOCR in OCR output, got: {text:?}"
    );
}

#[test]
fn scored_ocr_returns_score() {
    let locr = locr_core::shared().expect("engine init");
    let result = locr.image_to_text_scored(&hello_png()).expect("scored ocr");
    eprintln!(
        "scored: text={:?} score={:.3} confidence={:?} transform={}",
        result.text,
        result.score,
        result.confidence,
        result.transform.as_str()
    );
    assert!(!result.text.is_empty());
    assert!(result.score > 0.0);
    assert!(result.score <= 1.0);
    assert!(result.confidence.is_some_and(|c| c > 0.0 && c <= 1.0));
    assert_eq!(result.transform, TransformUsed::None);
    assert_eq!(result.attempts, 1);
}

#[test]
fn auto_enhance_picks_best() {
    let locr = locr_core::shared().expect("engine init");
    // Force the enhance loop by setting min_score unrealistically high.
    let opts = EnhanceOptions {
        min_score: 0.99,
        max_attempts: 4,
    };
    let result = locr
        .image_to_text_auto(&hello_png(), opts)
        .expect("auto ocr");
    eprintln!(
        "auto: text={:?} score={:.3} transform={} attempts={}",
        result.text,
        result.score,
        result.transform.as_str(),
        result.attempts
    );
    assert!(result.attempts >= 1);
    assert!(result.score >= score_ocr_text(""));
    let upper = result.text.to_uppercase();
    assert!(
        upper.contains("HELLO") || upper.contains("LOCR") || !result.text.is_empty(),
        "auto enhance produced empty/garbage: {:?}",
        result.text
    );
}

#[test]
fn structured_output_has_boxes() {
    let locr = locr_core::shared().expect("engine init");
    let result = locr
        .image_to_data(&hello_png(), &OcrOptions::default())
        .expect("ocr");
    eprintln!("{:#?}", result.lines);
    assert_eq!((result.width, result.height), (400, 120));
    assert!(!result.lines.is_empty());
    for line in &result.lines {
        assert!(line.bbox.width > 0 && line.bbox.right() <= 400 && line.bbox.bottom() <= 120);
        for word in &line.words {
            assert!(word.bbox.x >= line.bbox.x && word.bbox.right() <= line.bbox.right());
            assert!(word.confidence.is_some_and(|c| c > 0.0 && c <= 1.0));
        }
    }
    let words: Vec<String> = result
        .lines
        .iter()
        .flat_map(|l| &l.words)
        .map(|w| w.text.to_uppercase())
        .collect();
    assert!(
        words
            .iter()
            .any(|w| w.contains("HELLO") || w.contains("LOCR")),
        "{words:?}"
    );
}

#[test]
fn transparent_png_reads_like_the_original() {
    // Black-on-transparent used to come out as black-on-black: no text at all.
    let locr = locr_core::shared().expect("engine init");
    let original = decode_image(&hello_png()).unwrap();
    let expected = locr.image_to_text(&hello_png()).unwrap();
    let transparent = locr
        .image_to_text(&png(&transparent_ink(&original)))
        .unwrap();
    assert!(!expected.is_empty());
    assert_eq!(transparent, expected);
}

#[test]
fn exif_rotated_photo_reads_like_the_original() {
    let locr = locr_core::shared().expect("engine init");
    let original = decode_image(&hello_png()).unwrap();
    let expected = locr.image_to_text(&hello_png()).unwrap();
    let rotated = locr.image_to_text(&rotated_with_exif(&original)).unwrap();
    assert_eq!(rotated, expected);
}

#[test]
fn allowed_chars_whitelist() {
    let locr = locr_core::shared().expect("engine init");
    let opts = OcrOptions {
        allowed_chars: Some("HELOCR".into()),
        ..Default::default()
    };
    let result = locr.image_to_data(&hello_png(), &opts).unwrap();
    eprintln!("whitelisted: {:?}", result.text);
    assert!(
        result
            .text
            .chars()
            .all(|c| "HELOCR".contains(c) || c.is_whitespace()),
        "{:?}",
        result.text
    );
}

#[test]
fn beam_search_reads_hello() {
    let locr = locr_core::shared().expect("engine init");
    let opts = OcrOptions {
        beam_width: 5,
        ..Default::default()
    };
    let text = locr
        .image_to_data(&hello_png(), &opts)
        .unwrap()
        .text
        .to_uppercase();
    assert!(text.contains("HELLO") || text.contains("LOCR"), "{text:?}");
}
