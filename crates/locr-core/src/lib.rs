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
        let img = image::load_from_memory(bytes)
            .map_err(|e| LocrError::DecodeError(e.to_string()))?;
        self.engine.recognize(&img)
    }

    /// Run OCR on an already-decoded image.
    pub fn recognize(&self, image: &DynamicImage) -> Result<String> {
        self.engine.recognize(image)
    }
}

/// Default engine for the current feature set.
#[cfg(feature = "ocrs")]
pub type DefaultEngine = OcrsEngine;

#[cfg(all(not(feature = "ocrs"), feature = "tesseract-cli"))]
pub type DefaultEngine = TesseractCliEngine;

/// Build a `locr` handle with the default engine configured by crate features.
pub fn default() -> Result<Locr<DefaultEngine>> {
    Ok(Locr::new(DefaultEngine::new()?))
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
    }

    impl Default for OcrsEngine {
        fn default() -> Self {
            Self::new().expect("ocrs engine failed to initialize")
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
    use std::io::Write;

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
            image.write_to(&mut temp, image::ImageFormat::Png)
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

    #[cfg(feature = "ocrs")]
    #[test]
    fn ocrs_engine_initializes() {
        let _ = OcrsEngine::new();
    }

    #[cfg(feature = "tesseract-cli")]
    #[test]
    fn tesseract_trait_api_compiles() {
        let _loc = Locr::new(TesseractCliEngine::new());
    }
}
