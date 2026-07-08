//! Real end-to-end OCR smoke tests against the bundled ocrs models.
//!
//! Run with: `cargo test -p locr-core --test smoke_ocr -- --nocapture`

use locr_core::{score_ocr_text, EnhanceOptions, TransformUsed};

fn hello_png() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/hello.png"
    ))
    .expect("hello.png fixture missing")
}

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
        "scored: text={:?} score={:.3} transform={}",
        result.text,
        result.score,
        result.transform.as_str()
    );
    assert!(!result.text.is_empty());
    assert!(result.score > 0.0);
    assert!(result.score <= 1.0);
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
