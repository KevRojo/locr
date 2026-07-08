use image::DynamicImage;
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LocrError {
    #[error("failed to decode image: {0}")]
    DecodeError(String),
    #[error("ocr engine failed: {0}")]
    EngineError(String),
    #[error("tesseract binary not found. install it: https://tesseract-ocr.github.io/tessdoc/Installation.html")]
    TesseractNotFound,
    #[error("no ocrs models available; enable the `bundle-models` feature or provide model files")]
    ModelsNotFound,
}

pub type Result<T> = std::result::Result<T, LocrError>;

/// Abstraction over any local OCR engine.
pub trait OcrEngine: Send + Sync {
    fn recognize(&self, image: &DynamicImage) -> Result<String>;
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
        match self {
            TransformUsed::None => "none".into(),
            TransformUsed::Contrast(v) => format!("contrast:{v}"),
            TransformUsed::Brightness(v) => format!("brightness:{v}"),
            TransformUsed::Saturation(v) => format!("saturation:{v}"),
            TransformUsed::GrayscaleContrast(v) => format!("grayscale+contrast:{v}"),
            TransformUsed::Invert => "invert".into(),
            TransformUsed::Combo(s) => format!("combo:{s}"),
        }
    }
}

/// OCR result with a quality score and the transform that won.
#[derive(Debug, Clone)]
pub struct OcrResult {
    /// Recognized text (trimmed).
    pub text: String,
    /// Quality score in `[0.0, 1.0]`. Higher = more trustworthy.
    pub score: f32,
    /// Image transform that produced this result.
    pub transform: TransformUsed,
    /// How many OCR passes were attempted (1 = no enhance loop).
    pub attempts: u32,
}

/// Options for the auto-enhance superpower.
#[derive(Debug, Clone)]
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

/// Heuristic quality score for OCR text in `[0.0, 1.0]`.
///
/// ocrs does not expose per-char confidence, so we score the *output*:
/// length, alphanumeric ratio, word structure, and symbol noise.
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

    // 1) Length signal — empty is 0, short is weak, 8+ chars is solid.
    let length_score = (n / 12.0).clamp(0.0, 1.0);

    // 2) Alphanumeric / space ratio (letters+digits+space are "good").
    let good = chars
        .iter()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || matches!(c, '.' | ',' | ':' | '-' | '/' | '%' | '$' | '#'))
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
                && !matches!(c, '.' | ',' | ':' | ';' | '-' | '/' | '%' | '$' | '#' | '(' | ')' | '[' | ']' | '"' | '\'' | '!' | '?')
        })
        .count() as f32;
    let noise_penalty = (weird / n).clamp(0.0, 0.6);

    // Weighted blend.
    let raw = 0.25 * length_score + 0.35 * alnum_score + 0.30 * word_score + 0.10 * (1.0 - noise_penalty);
    (raw - noise_penalty * 0.4).clamp(0.0, 1.0)
}

/// Universal local OCR handle.
pub struct Locr<E: OcrEngine> {
    engine: E,
}

impl<E: OcrEngine> Locr<E> {
    pub fn new(engine: E) -> Self {
        Self { engine }
    }

    /// Decode bytes (PNG, JPEG, WEBP, BMP, TIFF, etc.) and run OCR.
    pub fn image_to_text(&self, bytes: &[u8]) -> Result<String> {
        let img = decode(bytes)?;
        self.engine.recognize(&img)
    }

    /// Run OCR on an already-decoded image.
    pub fn recognize(&self, image: &DynamicImage) -> Result<String> {
        self.engine.recognize(image)
    }

    /// OCR with a quality score (single pass, no enhance loop).
    pub fn image_to_text_scored(&self, bytes: &[u8]) -> Result<OcrResult> {
        let img = decode(bytes)?;
        self.recognize_scored(&img)
    }

    /// OCR with a quality score on a decoded image.
    pub fn recognize_scored(&self, image: &DynamicImage) -> Result<OcrResult> {
        let text = self.engine.recognize(image)?;
        Ok(OcrResult {
            score: score_ocr_text(&text),
            text,
            transform: TransformUsed::None,
            attempts: 1,
        })
    }

    /// Superpower: OCR + auto-enhance when the score is low.
    ///
    /// 1. Run OCR on the original image and score it.
    /// 2. If score < `opts.min_score`, try contrast / brightness / saturation /
    ///    grayscale / invert transforms and keep the best scoring result.
    ///
    /// Perfect for washed-out photos, dark UI screenshots, low-contrast scans.
    pub fn image_to_text_auto(
        &self,
        bytes: &[u8],
        opts: EnhanceOptions,
    ) -> Result<OcrResult> {
        let img = decode(bytes)?;
        self.recognize_auto(&img, opts)
    }

    /// Same as [`Self::image_to_text_auto`] but on a decoded image.
    pub fn recognize_auto(
        &self,
        image: &DynamicImage,
        opts: EnhanceOptions,
    ) -> Result<OcrResult> {
        let mut best = self.recognize_scored(image)?;
        if best.score >= opts.min_score || opts.max_attempts <= 1 {
            return Ok(best);
        }

        let candidates = enhance_candidates();
        let mut attempts = 1u32;

        for (transform, variant) in candidates {
            if attempts >= opts.max_attempts {
                break;
            }
            attempts += 1;
            let text = match self.engine.recognize(&variant(image)) {
                Ok(t) => t,
                Err(_) => continue,
            };
            let score = score_ocr_text(&text);
            if score > best.score {
                best = OcrResult {
                    text,
                    score,
                    transform,
                    attempts,
                };
                if best.score >= opts.min_score {
                    break;
                }
            } else {
                best.attempts = attempts;
            }
        }

        best.attempts = attempts;
        Ok(best)
    }
}

fn decode(bytes: &[u8]) -> Result<DynamicImage> {
    image::load_from_memory(bytes).map_err(|e| LocrError::DecodeError(e.to_string()))
}

/// Ordered list of (label, transform) candidates for the enhance loop.
fn enhance_candidates() -> Vec<(TransformUsed, Box<dyn Fn(&DynamicImage) -> DynamicImage>)> {
    vec![
        (
            TransformUsed::Contrast(30.0),
            Box::new(|img| DynamicImage::ImageRgba8(image::imageops::contrast(img, 30.0))),
        ),
        (
            TransformUsed::Contrast(55.0),
            Box::new(|img| DynamicImage::ImageRgba8(image::imageops::contrast(img, 55.0))),
        ),
        (
            TransformUsed::Brightness(25),
            Box::new(|img| DynamicImage::ImageRgba8(image::imageops::brighten(img, 25))),
        ),
        (
            TransformUsed::Brightness(-25),
            Box::new(|img| DynamicImage::ImageRgba8(image::imageops::brighten(img, -25))),
        ),
        (
            TransformUsed::Saturation(1.6),
            Box::new(|img| adjust_saturation(img, 1.6)),
        ),
        (
            TransformUsed::GrayscaleContrast(40.0),
            Box::new(|img| {
                let gray = DynamicImage::ImageLuma8(image::imageops::grayscale(img));
                DynamicImage::ImageRgba8(image::imageops::contrast(&gray, 40.0))
            }),
        ),
        (
            TransformUsed::Invert,
            Box::new(|img| {
                let mut rgba = img.to_rgba8();
                for p in rgba.pixels_mut() {
                    p[0] = 255 - p[0];
                    p[1] = 255 - p[1];
                    p[2] = 255 - p[2];
                }
                DynamicImage::ImageRgba8(rgba)
            }),
        ),
        (
            TransformUsed::Combo("sat1.4+contrast40".into()),
            Box::new(|img| {
                let sat = adjust_saturation(img, 1.4);
                DynamicImage::ImageRgba8(image::imageops::contrast(&sat, 40.0))
            }),
        ),
    ]
}

/// Multiply saturation around mid-gray. factor=1.0 is identity, >1 more vivid.
fn adjust_saturation(img: &DynamicImage, factor: f32) -> DynamicImage {
    use image::Rgba;
    let mut rgba = img.to_rgba8();
    for p in rgba.pixels_mut() {
        let r = p[0] as f32;
        let g = p[1] as f32;
        let b = p[2] as f32;
        // Luma (Rec. 601)
        let y = 0.299 * r + 0.587 * g + 0.114 * b;
        let nr = (y + (r - y) * factor).clamp(0.0, 255.0) as u8;
        let ng = (y + (g - y) * factor).clamp(0.0, 255.0) as u8;
        let nb = (y + (b - y) * factor).clamp(0.0, 255.0) as u8;
        *p = Rgba([nr, ng, nb, p[3]]);
    }
    DynamicImage::ImageRgba8(rgba)
}

/// Default engine for the current feature set.
#[cfg(feature = "ocrs")]
pub type DefaultEngine = OcrsEngine;

#[cfg(all(not(feature = "ocrs"), feature = "tesseract-cli"))]
pub type DefaultEngine = TesseractCliEngine;

/// Build a `locr` handle with the default engine configured by crate features.
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
/// expensive model load does not run on every call.
pub fn shared() -> Result<&'static Locr<DefaultEngine>> {
    use std::sync::OnceLock;
    // Cache the engine (or the error string) so we only pay model-load cost once.
    static ENGINE: OnceLock<std::result::Result<Locr<DefaultEngine>, String>> = OnceLock::new();
    let slot = ENGINE.get_or_init(|| default().map_err(|e| e.to_string()));
    match slot {
        Ok(engine) => Ok(engine),
        Err(msg) => {
            if msg.contains("models") {
                Err(LocrError::ModelsNotFound)
            } else if msg.contains("tesseract") {
                Err(LocrError::TesseractNotFound)
            } else {
                Err(LocrError::EngineError(msg.clone()))
            }
        }
    }
}

#[cfg(feature = "ocrs")]
mod ocrs_engine {
    use super::{DynamicImage, LocrError, OcrEngine, Result};
    use ocrs::{ImageSource, OcrEngine as OcrsInner, OcrEngineParams};
    use rten::Model;

    /// Pure-Rust OCR engine powered by [ocrs](https://github.com/robertknight/ocrs) and RTen.
    ///
    /// Models are embedded at build time via `include_bytes!` when the `bundle-models` feature
    /// is enabled. No runtime downloads, no cloud calls, no native binaries to install.
    pub struct OcrsEngine {
        engine: OcrsInner,
    }

    impl std::fmt::Debug for OcrsEngine {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("OcrsEngine").finish_non_exhaustive()
        }
    }

    impl OcrsEngine {
        pub fn new() -> Result<Self> {
            #[cfg(feature = "bundle-models")]
            {
                let detection_model = Model::load_static_slice(include_bytes!(concat!(
                    env!("OUT_DIR"),
                    "/text-detection.rten"
                )))
                .map_err(|e| LocrError::EngineError(format!("failed to load detection model: {}", e)))?;

                let recognition_model = Model::load_static_slice(include_bytes!(concat!(
                    env!("OUT_DIR"),
                    "/text-recognition.rten"
                )))
                .map_err(|e| LocrError::EngineError(format!("failed to load recognition model: {}", e)))?;

                let engine = OcrsInner::new(OcrEngineParams {
                    detection_model: Some(detection_model),
                    recognition_model: Some(recognition_model),
                    ..Default::default()
                })
                .map_err(|e| LocrError::EngineError(format!("failed to build ocrs engine: {}", e)))?;

                Ok(Self { engine })
            }

            #[cfg(not(feature = "bundle-models"))]
            {
                Err(LocrError::ModelsNotFound)
            }
        }

        /// Fallible constructor — prefer this over panicking defaults.
        pub fn try_new() -> Result<Self> {
            Self::new()
        }
    }

    impl OcrEngine for OcrsEngine {
        fn recognize(&self, image: &DynamicImage) -> Result<String> {
            let rgb = image.to_rgb8();
            let source = ImageSource::from_bytes(rgb.as_raw(), rgb.dimensions())
                .map_err(|e| LocrError::EngineError(format!("image preprocessing failed: {}", e)))?;
            let input = self
                .engine
                .prepare_input(source)
                .map_err(|e| LocrError::EngineError(format!("ocrs prepare_input failed: {}", e)))?;
            let text = self
                .engine
                .get_text(&input)
                .map_err(|e| LocrError::EngineError(format!("ocrs get_text failed: {}", e)))?;
            Ok(text.trim().to_string())
        }
    }
}

#[cfg(feature = "ocrs")]
pub use ocrs_engine::OcrsEngine;

#[cfg(feature = "tesseract-cli")]
mod tesseract_engine {
    use super::{DynamicImage, LocrError, OcrEngine, Result};

    /// Default engine using the local `tesseract` CLI.
    #[derive(Debug, Default, Clone, Copy)]
    pub struct TesseractCliEngine;

    impl TesseractCliEngine {
        pub fn new() -> Self {
            Self
        }
    }

    impl OcrEngine for TesseractCliEngine {
        fn recognize(&self, image: &DynamicImage) -> Result<String> {
            let mut temp = tempfile::NamedTempFile::with_suffix(".png")
                .map_err(|e| LocrError::EngineError(e.to_string()))?;
            image
                .write_to(&mut temp, image::ImageFormat::Png)
                .map_err(|e| LocrError::EngineError(e.to_string()))?;

            let output = std::process::Command::new("tesseract")
                .arg(temp.path())
                .arg("stdout")
                .arg("-l")
                .arg("eng")
                .output()
                .map_err(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        LocrError::TesseractNotFound
                    } else {
                        LocrError::EngineError(e.to_string())
                    }
                })?;

            if !output.status.success() {
                return Err(LocrError::EngineError(
                    String::from_utf8_lossy(&output.stderr).to_string(),
                ));
            }

            String::from_utf8(output.stdout)
                .map(|s| s.trim().to_string())
                .map_err(|e| LocrError::EngineError(e.to_string()))
        }
    }
}

#[cfg(feature = "tesseract-cli")]
pub use tesseract_engine::TesseractCliEngine;

#[cfg(test)]
mod tests {
    use super::*;

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
        let _ = shared();
        let _ = shared();
    }

    #[cfg(feature = "tesseract-cli")]
    #[test]
    fn tesseract_trait_api_compiles() {
        let _loc = Locr::new(TesseractCliEngine::new());
    }
}
