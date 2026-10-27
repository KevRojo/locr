//! Universal local OCR engine.
//!
//! `locr-core` decodes an image (honoring EXIF orientation), flattens
//! transparency onto a contrasting background, and runs a pure-Rust OCR
//! engine that returns text, lines, words, bounding boxes and per-character
//! confidence. Nothing leaves the device.
//!
//! ```no_run
//! let bytes = std::fs::read("invoice.png")?;
//! let locr = locr_core::shared()?;
//! println!("{}", locr.image_to_text(&bytes)?);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::fmt;

use image::DynamicImage;
use thiserror::Error;

mod preprocess;
pub use preprocess::{decode_image, image_from_pixels};

#[cfg(feature = "json")]
mod json;
#[cfg(feature = "json")]
pub use json::JSON_SCHEMA_VERSION;

#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum LocrError {
    #[error("failed to decode image: {0}")]
    DecodeError(String),
    #[error("ocr engine failed: {0}")]
    EngineError(String),
    #[error("tesseract binary not found. install it: https://tesseract-ocr.github.io/tessdoc/Installation.html")]
    TesseractNotFound,
    #[error("no ocrs models available; enable the `bundle-models` feature, set LOCR_MODELS_DIR, or load model files explicitly")]
    ModelsNotFound,
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

pub type Result<T> = std::result::Result<T, LocrError>;

/// Axis-aligned box in image pixel coordinates (origin at the top-left).
///
/// Coordinates refer to the image as displayed, i.e. after EXIF orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BoundingBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl BoundingBox {
    /// Exclusive right edge.
    pub fn right(&self) -> u32 {
        self.x + self.width
    }

    /// Exclusive bottom edge.
    pub fn bottom(&self) -> u32 {
        self.y + self.height
    }

    /// Smallest box containing both `self` and `other`.
    pub fn union(&self, other: &BoundingBox) -> BoundingBox {
        let (x, y) = (self.x.min(other.x), self.y.min(other.y));
        BoundingBox {
            x,
            y,
            width: self.right().max(other.right()) - x,
            height: self.bottom().max(other.bottom()) - y,
        }
    }
}

/// A recognized word: a run of non-space characters.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Word {
    pub text: String,
    /// Mean recognition probability of the word's characters, in `[0.0, 1.0]`.
    /// `None` when the engine does not report confidence.
    pub confidence: Option<f32>,
    pub bbox: BoundingBox,
}

/// A recognized line of text, with its words in reading order.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Line {
    pub text: String,
    /// Mean recognition probability of the line's non-space characters.
    pub confidence: Option<f32>,
    pub bbox: BoundingBox,
    pub words: Vec<Word>,
}

/// Abstraction over any local OCR engine.
pub trait OcrEngine: Send + Sync {
    /// Detect and recognize text, returning lines in reading order.
    ///
    /// [`Locr`] calls this with an image that is already EXIF-oriented and
    /// has no transparency. Engines honor `options.allowed_chars` and
    /// `options.beam_width` when they can; `options.enhance` is handled by
    /// [`Locr`].
    fn recognize_lines(&self, image: &DynamicImage, options: &OcrOptions) -> Result<Vec<Line>>;
}

/// Which image transform produced the winning OCR result.
#[derive(Debug, Clone, PartialEq)]
pub enum TransformUsed {
    /// Original image, no enhancement.
    None,
    /// Contrast adjusted by the given amount (image crate units, ~ -100..100).
    Contrast(f32),
    /// Brightness adjusted by the given amount (image crate units, ~ -255..255).
    Brightness(i32),
    /// Saturation multiplied by factor (1.0 = unchanged, >1 = more vivid).
    Saturation(f32),
    /// Converted to grayscale then contrast-boosted.
    GrayscaleContrast(f32),
    /// Color inverted (great for white-on-black / dark UI screenshots).
    Invert,
    /// Combo of transforms applied in order (described for diagnostics).
    Combo(String),
}

impl TransformUsed {
    pub fn as_str(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for TransformUsed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransformUsed::None => f.write_str("none"),
            TransformUsed::Contrast(v) => write!(f, "contrast:{v}"),
            TransformUsed::Brightness(v) => write!(f, "brightness:{v}"),
            TransformUsed::Saturation(v) => write!(f, "saturation:{v}"),
            TransformUsed::GrayscaleContrast(v) => write!(f, "grayscale+contrast:{v}"),
            TransformUsed::Invert => f.write_str("invert"),
            TransformUsed::Combo(s) => write!(f, "combo:{s}"),
        }
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for TransformUsed {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// OCR result: text, layout, quality score and the transform that won.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct OcrResult {
    /// Recognized text: one line per detected text line, trimmed.
    pub text: String,
    /// Quality score in `[0.0, 1.0]`. Higher = more trustworthy.
    ///
    /// Blends the model's recognition confidence with the [`score_ocr_text`]
    /// heuristic (`0.6 * confidence + 0.4 * heuristic`); heuristic only when
    /// the engine reports no confidence.
    pub score: f32,
    /// Mean recognition probability over all non-space characters, if the
    /// engine reports it.
    pub confidence: Option<f32>,
    /// Image transform that produced this result.
    pub transform: TransformUsed,
    /// How many OCR passes were attempted (1 = no enhance loop).
    pub attempts: u32,
    /// Width of the analyzed image (after EXIF orientation).
    pub width: u32,
    /// Height of the analyzed image (after EXIF orientation).
    pub height: u32,
    /// Text lines in reading order, with words and bounding boxes.
    pub lines: Vec<Line>,
}

impl OcrResult {
    fn from_lines(
        lines: Vec<Line>,
        width: u32,
        height: u32,
        transform: TransformUsed,
        attempts: u32,
    ) -> Self {
        let text = lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        // Weight each line by its character count so the page confidence is
        // the mean over characters, not over lines.
        let (mut sum, mut weight) = (0.0f64, 0.0f64);
        for line in &lines {
            if let Some(c) = line.confidence.filter(|c| c.is_finite()) {
                let n = line.text.chars().filter(|c| !c.is_whitespace()).count() as f64;
                sum += c as f64 * n;
                weight += n;
            }
        }
        let confidence = (weight > 0.0).then(|| (sum / weight) as f32);

        OcrResult {
            score: quality_score(&text, confidence),
            text,
            confidence,
            transform,
            attempts,
            width,
            height,
            lines,
        }
    }
}

/// Options for the auto-enhance superpower.
#[derive(Debug, Clone, PartialEq)]
pub struct EnhanceOptions {
    /// If the first pass scores below this, try image transforms.
    /// Default: `0.55`.
    pub min_score: f32,
    /// Max OCR passes including the original. Default: `7`.
    pub max_attempts: u32,
}

impl Default for EnhanceOptions {
    fn default() -> Self {
        Self {
            min_score: 0.55,
            max_attempts: 7,
        }
    }
}

/// Per-call recognition options. `OcrOptions::default()` = plain OCR.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OcrOptions {
    /// Only emit these characters, e.g. `"0123456789"` for meter readings or
    /// invoice totals. Spaces are always allowed so words stay separated.
    /// `None` = the full model alphabet.
    pub allowed_chars: Option<String>,
    /// CTC beam search width. `0` or `1` = greedy decoding (fastest).
    /// Wider beams can fix ambiguous characters at some CPU cost.
    pub beam_width: u32,
    /// Retry with contrast/brightness/saturation/... transforms when the
    /// first pass scores low. `None` = single pass.
    pub enhance: Option<EnhanceOptions>,
}

/// Heuristic quality score for OCR text in `[0.0, 1.0]`.
///
/// Scores the *output*: length, alphanumeric ratio, word structure, and
/// symbol noise. [`OcrResult::score`] blends it with the model confidence.
///
/// This is intentionally simple and deterministic — good enough to decide
/// whether to retry with contrast/brightness/saturation boosts.
pub fn score_ocr_text(text: &str) -> f32 {
    let text = text.trim();
    if text.is_empty() {
        return 0.0;
    }

    let chars: Vec<char> = text.chars().collect();
    let n = chars.len() as f32;

    // 1) Length signal — empty is 0, short is weak, 12+ chars is solid.
    let length_score = (n / 12.0).clamp(0.0, 1.0);

    // 2) Alphanumeric / space ratio (letters+digits+space are "good").
    let good = chars
        .iter()
        .filter(|c| {
            c.is_alphanumeric()
                || c.is_whitespace()
                || matches!(c, '.' | ',' | ':' | '-' | '/' | '%' | '$' | '#')
        })
        .count() as f32;
    let alnum_score = good / n;

    // 3) Word structure — prefer 2+ tokens of length >= 2.
    let words: Vec<&str> = text.split_whitespace().collect();
    let real_words = words.iter().filter(|w| w.chars().count() >= 2).count();
    let word_score = match real_words {
        0 => 0.1,
        1 => 0.55,
        2 => 0.8,
        _ => 1.0,
    };

    // 4) Noise penalty — too many weird symbols tanks the score.
    let weird = chars
        .iter()
        .filter(|c| {
            !c.is_alphanumeric()
                && !c.is_whitespace()
                && !matches!(
                    c,
                    '.' | ','
                        | ':'
                        | ';'
                        | '-'
                        | '/'
                        | '%'
                        | '$'
                        | '#'
                        | '('
                        | ')'
                        | '['
                        | ']'
                        | '"'
                        | '\''
                        | '!'
                        | '?'
                )
        })
        .count() as f32;
    let noise_penalty = (weird / n).clamp(0.0, 0.6);

    // Weighted blend.
    let raw =
        0.25 * length_score + 0.35 * alnum_score + 0.30 * word_score + 0.10 * (1.0 - noise_penalty);
    (raw - noise_penalty * 0.4).clamp(0.0, 1.0)
}

/// [`OcrResult::score`]: model confidence blended with the text heuristic.
fn quality_score(text: &str, confidence: Option<f32>) -> f32 {
    let heuristic = score_ocr_text(text);
    match confidence {
        Some(c) if heuristic > 0.0 => (0.6 * c + 0.4 * heuristic).clamp(0.0, 1.0),
        _ => heuristic,
    }
}

/// Universal local OCR handle.
pub struct Locr<E: OcrEngine> {
    engine: E,
}

impl<E: OcrEngine> Locr<E> {
    pub fn new(engine: E) -> Self {
        Self { engine }
    }

    /// The underlying engine.
    pub fn engine(&self) -> &E {
        &self.engine
    }

    /// Decode bytes (PNG, JPEG, WEBP, BMP, TIFF, etc.) and run OCR.
    pub fn image_to_text(&self, bytes: &[u8]) -> Result<String> {
        Ok(self.image_to_data(bytes, &OcrOptions::default())?.text)
    }

    /// Run OCR on an already-decoded image.
    pub fn recognize(&self, image: &DynamicImage) -> Result<String> {
        Ok(self.recognize_with(image, &OcrOptions::default())?.text)
    }

    /// OCR with a quality score (single pass, no enhance loop).
    pub fn image_to_text_scored(&self, bytes: &[u8]) -> Result<OcrResult> {
        self.image_to_data(bytes, &OcrOptions::default())
    }

    /// OCR with a quality score on a decoded image.
    pub fn recognize_scored(&self, image: &DynamicImage) -> Result<OcrResult> {
        self.recognize_with(image, &OcrOptions::default())
    }

    /// Superpower: OCR + auto-enhance when the score is low.
    ///
    /// 1. Run OCR on the original image and score it.
    /// 2. If score < `opts.min_score`, try contrast / brightness / saturation /
    ///    grayscale / invert transforms and keep the best scoring result.
    ///
    /// Perfect for washed-out photos, dark UI screenshots, low-contrast scans.
    pub fn image_to_text_auto(&self, bytes: &[u8], opts: EnhanceOptions) -> Result<OcrResult> {
        self.image_to_data(
            bytes,
            &OcrOptions {
                enhance: Some(opts),
                ..OcrOptions::default()
            },
        )
    }

    /// Same as [`Self::image_to_text_auto`] but on a decoded image.
    pub fn recognize_auto(&self, image: &DynamicImage, opts: EnhanceOptions) -> Result<OcrResult> {
        self.recognize_with(
            image,
            &OcrOptions {
                enhance: Some(opts),
                ..OcrOptions::default()
            },
        )
    }

    /// Full result — text, lines, words, boxes, confidence — with options.
    ///
    /// ```no_run
    /// use locr_core::OcrOptions;
    ///
    /// let bytes = std::fs::read("meter.jpg")?;
    /// let opts = OcrOptions { allowed_chars: Some("0123456789.".into()), ..Default::default() };
    /// let result = locr_core::shared()?.image_to_data(&bytes, &opts)?;
    /// for line in &result.lines {
    ///     println!("{} {:?} {:?}", line.text, line.bbox, line.confidence);
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn image_to_data(&self, bytes: &[u8], options: &OcrOptions) -> Result<OcrResult> {
        let img = decode_image(bytes)?;
        self.recognize_with(&img, options)
    }

    /// Same as [`Self::image_to_data`] but on a decoded image.
    pub fn recognize_with(&self, image: &DynamicImage, options: &OcrOptions) -> Result<OcrResult> {
        let image = preprocess::flatten_alpha(image);
        let mut best = self.run_pass(&image, options, TransformUsed::None, 1)?;
        let Some(enhance) = &options.enhance else {
            return Ok(best);
        };
        if best.score >= enhance.min_score || enhance.max_attempts <= 1 {
            return Ok(best);
        }

        let rgb = image.to_rgb8();
        let mut attempts = 1u32;

        for (transform, variant) in preprocess::enhance_candidates() {
            if attempts >= enhance.max_attempts {
                break;
            }
            attempts += 1;
            let candidate = match self.run_pass(&variant(&rgb), options, transform, attempts) {
                Ok(c) => c,
                Err(_) => continue,
            };
            if candidate.score > best.score {
                best = candidate;
                if best.score >= enhance.min_score {
                    break;
                }
            }
        }

        best.attempts = attempts;
        Ok(best)
    }

    fn run_pass(
        &self,
        image: &DynamicImage,
        options: &OcrOptions,
        transform: TransformUsed,
        attempts: u32,
    ) -> Result<OcrResult> {
        let (width, height) = (image.width(), image.height());
        let lines = if width == 0 || height == 0 {
            Vec::new()
        } else {
            self.engine.recognize_lines(image, options)?
        };
        Ok(OcrResult::from_lines(
            lines, width, height, transform, attempts,
        ))
    }
}

/// Default engine for the current feature set.
#[cfg(feature = "ocrs")]
pub type DefaultEngine = OcrsEngine;

#[cfg(all(not(feature = "ocrs"), feature = "tesseract-cli"))]
pub type DefaultEngine = TesseractCliEngine;

/// Build a `locr` handle with the default engine configured by crate features.
#[cfg(any(feature = "ocrs", feature = "tesseract-cli"))]
pub fn default() -> Result<Locr<DefaultEngine>> {
    #[cfg(feature = "ocrs")]
    {
        Ok(Locr::new(DefaultEngine::new()?))
    }
    #[cfg(all(not(feature = "ocrs"), feature = "tesseract-cli"))]
    {
        Ok(Locr::new(DefaultEngine::new()))
    }
}

/// Shared default engine (models loaded once). Safe for concurrent use.
///
/// Prefer this over [`default`] in hot paths (FFI, WASM, batch OCR) so the
/// expensive model load does not run on every call. The first outcome is
/// cached for the life of the process, including a failure to find models.
#[cfg(any(feature = "ocrs", feature = "tesseract-cli"))]
pub fn shared() -> Result<&'static Locr<DefaultEngine>> {
    use std::sync::OnceLock;
    static ENGINE: OnceLock<Result<Locr<DefaultEngine>>> = OnceLock::new();
    ENGINE.get_or_init(default).as_ref().map_err(Clone::clone)
}

#[cfg(feature = "ocrs")]
mod ocrs_engine;
#[cfg(feature = "ocrs")]
pub use ocrs_engine::OcrsEngine;

#[cfg(feature = "tesseract-cli")]
mod tesseract_engine;
#[cfg(feature = "tesseract-cli")]
pub use tesseract_engine::TesseractCliEngine;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn score_empty_is_zero() {
        assert_eq!(score_ocr_text(""), 0.0);
        assert_eq!(score_ocr_text("   "), 0.0);
    }

    #[test]
    fn score_clean_text_is_high() {
        let s = score_ocr_text("HELLO LOCR");
        assert!(s > 0.6, "expected high score for clean text, got {s}");
    }

    #[test]
    fn score_garbage_is_low() {
        let s = score_ocr_text("@@@###~~~^^^");
        assert!(s < 0.45, "expected low score for garbage, got {s}");
    }

    #[test]
    fn confidence_pulls_the_score() {
        let text = "HELLO LOCR";
        assert_eq!(quality_score(text, None), score_ocr_text(text));
        assert!(quality_score(text, Some(0.99)) > quality_score(text, Some(0.2)));
        assert_eq!(quality_score("", Some(0.99)), 0.0);
    }

    #[test]
    fn bbox_union() {
        let a = BoundingBox {
            x: 10,
            y: 5,
            width: 10,
            height: 10,
        };
        let b = BoundingBox {
            x: 30,
            y: 0,
            width: 5,
            height: 5,
        };
        assert_eq!(
            a.union(&b),
            BoundingBox {
                x: 10,
                y: 0,
                width: 25,
                height: 15
            }
        );
    }

    #[test]
    fn transform_display_matches_as_str() {
        assert_eq!(TransformUsed::Contrast(30.0).to_string(), "contrast:30");
        assert_eq!(TransformUsed::Invert.as_str(), "invert");
    }

    fn line(text: &str, confidence: Option<f32>) -> Line {
        Line {
            text: text.into(),
            confidence,
            bbox: BoundingBox::default(),
            words: Vec::new(),
        }
    }

    #[test]
    fn page_confidence_is_weighted_by_characters() {
        let lines = vec![line("aaaaaaaa", Some(1.0)), line("bb", Some(0.5))];
        let r = OcrResult::from_lines(lines, 10, 10, TransformUsed::None, 1);
        assert_eq!(r.text, "aaaaaaaa\nbb");
        assert!((r.confidence.unwrap() - 0.9).abs() < 1e-6);
        let r = OcrResult::from_lines(vec![line("x", None)], 1, 1, TransformUsed::None, 1);
        assert_eq!(r.confidence, None);
    }

    /// Scripted engine: returns canned outputs in order and records the
    /// images it was asked to read.
    struct Scripted {
        outputs: Mutex<Vec<Result<Vec<Line>>>>,
        seen: Mutex<Vec<DynamicImage>>,
    }

    impl Scripted {
        fn new(outputs: Vec<Result<Vec<Line>>>) -> Self {
            Self {
                outputs: Mutex::new(outputs),
                seen: Mutex::new(Vec::new()),
            }
        }
    }

    impl OcrEngine for Scripted {
        fn recognize_lines(&self, image: &DynamicImage, _: &OcrOptions) -> Result<Vec<Line>> {
            self.seen.lock().unwrap().push(image.clone());
            let mut outputs = self.outputs.lock().unwrap();
            if outputs.is_empty() {
                Ok(Vec::new())
            } else {
                outputs.remove(0)
            }
        }
    }

    fn white(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            w,
            h,
            image::Rgb([255, 255, 255]),
        ))
    }

    #[test]
    fn single_pass_without_enhance() {
        let locr = Locr::new(Scripted::new(vec![Ok(vec![line("@@", Some(0.1))])]));
        let r = locr.recognize_scored(&white(4, 4)).unwrap();
        assert_eq!((r.attempts, r.transform.clone()), (1, TransformUsed::None));
        assert_eq!((r.width, r.height), (4, 4));
        assert_eq!(locr.engine().seen.lock().unwrap().len(), 1);
    }

    #[test]
    fn enhance_keeps_the_best_and_stops_early() {
        let locr = Locr::new(Scripted::new(vec![
            Ok(vec![line("@#", Some(0.2))]),
            Err(LocrError::EngineError("boom".into())),
            Ok(vec![line("HELLO LOCR again", Some(0.95))]),
            Ok(vec![line("never reached", Some(0.99))]),
        ]));
        let r = locr
            .recognize_auto(&white(4, 4), EnhanceOptions::default())
            .unwrap();
        assert_eq!(r.text, "HELLO LOCR again");
        assert_eq!(r.transform, TransformUsed::Contrast(55.0));
        assert_eq!(r.attempts, 3);
        assert_eq!(locr.engine().seen.lock().unwrap().len(), 3);
    }

    #[test]
    fn enhance_respects_max_attempts() {
        let locr = Locr::new(Scripted::new(vec![]));
        let opts = EnhanceOptions {
            min_score: 0.99,
            max_attempts: 4,
        };
        let r = locr.recognize_auto(&white(4, 4), opts).unwrap();
        assert_eq!(r.attempts, 4);
        assert_eq!(r.transform, TransformUsed::None);
        assert_eq!(locr.engine().seen.lock().unwrap().len(), 4);
    }

    #[test]
    fn empty_image_skips_the_engine() {
        let locr = Locr::new(Scripted::new(vec![Err(LocrError::EngineError(
            "unreachable".into(),
        ))]));
        let r = locr.recognize_scored(&white(0, 0)).unwrap();
        assert!(r.text.is_empty() && r.lines.is_empty());
        assert!(locr.engine().seen.lock().unwrap().is_empty());
    }

    #[test]
    fn engine_sees_flattened_image() {
        let mut rgba = image::RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 0, 0]));
        rgba.put_pixel(1, 1, image::Rgba([0, 0, 0, 255]));
        let locr = Locr::new(Scripted::new(vec![]));
        locr.recognize(&DynamicImage::ImageRgba8(rgba)).unwrap();
        let seen = locr.engine().seen.lock().unwrap();
        let rgb = seen[0].as_rgb8().expect("flattened to RGB");
        assert_eq!(rgb.get_pixel(0, 0).0, [255, 255, 255]);
        assert_eq!(rgb.get_pixel(1, 1).0, [0, 0, 0]);
    }

    #[cfg(feature = "ocrs")]
    #[test]
    fn ocrs_engine_initializes() {
        let engine = OcrsEngine::new();
        assert!(
            engine.is_ok() || matches!(engine, Err(LocrError::ModelsNotFound)),
            "unexpected init error: {:?}",
            engine.err()
        );
    }

    #[cfg(feature = "ocrs")]
    #[test]
    fn shared_engine_is_reusable() {
        let first = shared().map(|l| l as *const _);
        let second = shared().map(|l| l as *const _);
        assert_eq!(first, second);
    }

    #[cfg(feature = "tesseract-cli")]
    #[test]
    fn tesseract_trait_api_compiles() {
        let _loc = Locr::new(TesseractCliEngine::new());
    }
}
