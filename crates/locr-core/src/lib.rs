use image::DynamicImage;
use std::io::Write;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_api_compiles() {
        let _loc = Locr::new(TesseractCliEngine::new());
    }
}
