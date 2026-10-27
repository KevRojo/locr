//! End-to-end tests through the real ocrs pipeline with tiny synthetic models
//! (`tests/fixtures/synthetic`), so they run offline. The recognized text is
//! meaningless; layout, geometry and invariants are not.
#![cfg(feature = "ocrs")]

mod common;

use common::*;
use locr_core::{
    image_from_pixels, EnhanceOptions, Locr, OcrOptions, OcrResult, OcrsEngine, TransformUsed,
};
use std::sync::OnceLock;

fn locr() -> &'static Locr<OcrsEngine> {
    static LOCR: OnceLock<Locr<OcrsEngine>> = OnceLock::new();
    LOCR.get_or_init(|| {
        Locr::new(OcrsEngine::from_models_dir(SYNTHETIC_MODELS).expect("synthetic models"))
    })
}

fn read(bytes: &[u8]) -> OcrResult {
    locr()
        .image_to_data(bytes, &OcrOptions::default())
        .expect("ocr")
}

#[test]
fn structured_output_follows_the_layout() {
    let page = synthetic_page(&[&[5, 3], &[4], &[2, 6, 3]]);
    let r = read(&png(&page));

    assert_eq!((r.width, r.height), (page.width(), page.height()));
    assert_eq!(r.lines.len(), 3, "{r:#?}");
    assert_eq!(
        r.text,
        r.lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!((0.0..=1.0).contains(&r.score));
    let confidence = r.confidence.expect("ocrs reports confidence");
    assert!(confidence > 0.0 && confidence <= 1.0);

    for (i, line) in r.lines.iter().enumerate() {
        // Rows are 70px apart starting at y=30, glyphs are 22px tall.
        let row_top = 30 + i as u32 * 70;
        assert!(
            line.bbox.y <= row_top && line.bbox.bottom() >= row_top + 22,
            "line {i}: {:?}",
            line.bbox
        );
        assert!(line.bbox.right() <= r.width && line.bbox.bottom() <= r.height);
        assert!(!line.words.is_empty());
        assert!(line.confidence.is_some());
        for word in &line.words {
            assert!(!word.text.is_empty() && !word.text.contains(' '));
            assert!(word.bbox.x >= line.bbox.x && word.bbox.right() <= line.bbox.right());
            assert!(word.confidence.is_some_and(|c| c > 0.0 && c <= 1.0));
        }
    }
}

#[test]
fn blank_page_reads_nothing() {
    let r = read(&png(&synthetic_page(&[])));
    assert!(r.text.is_empty() && r.lines.is_empty());
    assert_eq!((r.score, r.confidence), (0.0, None));
}

#[test]
fn exif_rotated_photo_reads_like_the_upright_one() {
    let page = synthetic_page(&[&[4, 2], &[3, 3]]);
    let upright = read(&png(&page));
    let rotated = read(&rotated_with_exif(&page));
    assert!(!upright.lines.is_empty());
    assert_eq!(upright, rotated);
}

#[test]
fn transparent_background_reads_like_white() {
    let page = synthetic_page(&[&[3, 4], &[5]]);
    let white = read(&png(&page));
    let transparent = read(&png(&transparent_ink(&page)));
    assert!(!white.lines.is_empty());
    assert_eq!(white, transparent);
}

#[test]
fn raw_pixels_read_like_encoded_images() {
    let page = synthetic_page(&[&[2, 2, 2]]);
    let rgba = page.to_rgba8();
    let (w, h) = rgba.dimensions();
    let from_pixels = locr()
        .recognize_with(
            &image_from_pixels(w, h, rgba.into_raw()).unwrap(),
            &OcrOptions::default(),
        )
        .unwrap();
    assert_eq!(read(&png(&page)), from_pixels);
}

#[test]
fn allowed_chars_restrict_the_output() {
    let page = png(&synthetic_page(&[&[6, 6], &[6, 6]]));
    let opts = OcrOptions {
        allowed_chars: Some("0123456789".into()),
        ..Default::default()
    };
    let r = locr().image_to_data(&page, &opts).unwrap();
    assert!(!r.text.is_empty());
    assert!(
        r.text
            .chars()
            .all(|c| c.is_ascii_digit() || c.is_whitespace()),
        "{:?}",
        r.text
    );
}

#[test]
fn beam_search_runs() {
    let page = png(&synthetic_page(&[&[4, 4]]));
    let r = locr()
        .image_to_data(
            &page,
            &OcrOptions {
                beam_width: 4,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(r.lines.len(), 1);
}

#[test]
fn enhance_loop_runs_through_the_pipeline() {
    let page = png(&synthetic_page(&[&[3, 3]]));
    let opts = EnhanceOptions {
        min_score: 1.1,
        max_attempts: 3,
    };
    let r = locr().image_to_text_auto(&page, opts).unwrap();
    assert_eq!(r.attempts, 3);
    assert!(!r.lines.is_empty());
    let plain = read(&page);
    assert!(r.score >= plain.score);
    if r.transform == TransformUsed::None {
        assert_eq!(r.text, plain.text);
    }
}

#[test]
fn engine_is_shareable_across_threads() {
    let page = png(&synthetic_page(&[&[3, 2], &[4]]));
    let expected = read(&page);
    std::thread::scope(|s| {
        let handles: Vec<_> = (0..4).map(|_| s.spawn(|| read(&page))).collect();
        for h in handles {
            assert_eq!(h.join().unwrap(), expected);
        }
    });
}

#[cfg(feature = "json")]
#[test]
fn json_schema_round_trip() {
    let r = read(&png(&synthetic_page(&[&[3, 2]])));
    let json: serde_json::Value = serde_json::from_str(&r.to_json()).unwrap();
    assert_eq!(json["version"], locr_core::JSON_SCHEMA_VERSION);
    assert_eq!(json["text"], r.text.as_str());
    assert_eq!(json["lines"].as_array().unwrap().len(), r.lines.len());
    let lines: Vec<locr_core::Line> = serde_json::from_value(json["lines"].clone()).unwrap();
    assert_eq!(lines, r.lines);
}
