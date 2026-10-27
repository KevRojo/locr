//! Pure-Rust OCR engine powered by [ocrs](https://github.com/robertknight/ocrs) and RTen.
//!
//! ocrs handles detection, layout analysis and line-image preparation. The
//! recognition model is run here so its character probabilities survive as
//! confidence scores (ocrs discards them). Batching, CTC decoding and the
//! mapping back to image coordinates mirror ocrs'
//! `TextRecognizer::recognize_text_lines`; the differential tests below hold
//! this module to ocrs' output.

use std::path::Path;

use image::DynamicImage;
use ocrs::{ImageSource, OcrEngine as OcrsInner, OcrEngineParams, OcrInput};
use rayon::prelude::*;
use rten::ctc::{CtcDecoder, CtcHypothesis};
use rten::{Dimension, Model};
use rten_imageproc::{
    bounding_rect, BoundingRect, Line as Segment, LineF, Point, PointF, Polygon, Rect, RotatedRect,
};
use rten_tensor::prelude::*;
use rten_tensor::{NdTensor, NdTensorView, Tensor};

use crate::{BoundingBox, Line, LocrError, OcrEngine, OcrOptions, Result, Word};

/// Characters of the bundled recognition model, in label order (label 0 is
/// the CTC blank). Same as ocrs' default alphabet, except that label 44 is
/// decoded as the '€' the model was trained on (ocrs prints an 'E').
const DEFAULT_ALPHABET: &str = " 0123456789!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~€ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Normalized value of black pixels in ocrs' input tensors; also the padding value.
const BLACK_VALUE: f32 = -0.5;

/// Lines per recognition batch (matches ocrs).
const MAX_LINES_PER_BATCH: usize = 20;

/// Pure-Rust OCR engine powered by [ocrs](https://github.com/robertknight/ocrs) and RTen.
///
/// Models are embedded at build time via `include_bytes!` when the `bundle-models` feature
/// is enabled. No runtime downloads, no cloud calls, no native binaries to install.
pub struct OcrsEngine {
    /// Detection, layout analysis and line preparation.
    engine: OcrsInner,
    /// Second handle on the recognition model, run directly to keep probabilities.
    recognizer: Model,
    /// Height of line images expected by `recognizer`.
    rec_height: usize,
    alphabet: Vec<char>,
}

impl std::fmt::Debug for OcrsEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OcrsEngine").finish_non_exhaustive()
    }
}

impl OcrsEngine {
    /// Engine with the default models: embedded at build time with the
    /// `bundle-models` feature, otherwise loaded from `$LOCR_MODELS_DIR`.
    pub fn new() -> Result<Self> {
        #[cfg(feature = "bundle-models")]
        {
            // 64-byte aligned so RTen can use the weights in place instead of
            // copying every tensor out of an unaligned `include_bytes!` slice.
            #[repr(C, align(64))]
            struct Aligned<T: ?Sized>(T);
            static DETECTION: &Aligned<[u8]> = &Aligned(*include_bytes!(concat!(
                env!("OUT_DIR"),
                "/text-detection.rten"
            )));
            static RECOGNITION: &Aligned<[u8]> = &Aligned(*include_bytes!(concat!(
                env!("OUT_DIR"),
                "/text-recognition.rten"
            )));

            let load = |bytes: &'static [u8], which| {
                Model::load_static_slice(bytes).map_err(load_err(which))
            };
            Self::from_models(
                load(&DETECTION.0, "detection")?,
                load(&RECOGNITION.0, "recognition")?,
                load(&RECOGNITION.0, "recognition")?,
            )
        }

        #[cfg(not(feature = "bundle-models"))]
        {
            match std::env::var_os("LOCR_MODELS_DIR") {
                Some(dir) => Self::from_models_dir(dir),
                None => Err(LocrError::ModelsNotFound),
            }
        }
    }

    /// Fallible constructor — prefer this over panicking defaults.
    pub fn try_new() -> Result<Self> {
        Self::new()
    }

    /// Engine from `.rten` model bytes, e.g. fetched once and cached by the host app.
    pub fn from_model_bytes(detection: Vec<u8>, recognition: Vec<u8>) -> Result<Self> {
        let detection = Model::load(detection).map_err(load_err("detection"))?;
        let layout = Model::load(recognition.clone()).map_err(load_err("recognition"))?;
        let recognition = Model::load(recognition).map_err(load_err("recognition"))?;
        Self::from_models(detection, layout, recognition)
    }

    /// Engine from `.rten` model files.
    pub fn from_model_files(
        detection: impl AsRef<Path>,
        recognition: impl AsRef<Path>,
    ) -> Result<Self> {
        let read = |path: &Path, which: &str| {
            std::fs::read(path).map_err(|e| {
                LocrError::EngineError(format!(
                    "failed to read {which} model {}: {e}",
                    path.display()
                ))
            })
        };
        Self::from_model_bytes(
            read(detection.as_ref(), "detection")?,
            read(recognition.as_ref(), "recognition")?,
        )
    }

    /// Engine from a directory containing `text-detection.rten` and `text-recognition.rten`.
    pub fn from_models_dir(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        Self::from_model_files(
            dir.join("text-detection.rten"),
            dir.join("text-recognition.rten"),
        )
    }

    /// Decode with a custom alphabet, for recognition models trained on other
    /// character sets. The model must output `alphabet.chars().count() + 1` classes.
    pub fn with_alphabet(mut self, alphabet: &str) -> Self {
        self.alphabet = alphabet.chars().collect();
        self
    }

    fn from_models(detection: Model, layout: Model, recognition: Model) -> Result<Self> {
        let engine = OcrsInner::new(OcrEngineParams {
            detection_model: Some(detection),
            recognition_model: Some(layout),
            ..Default::default()
        })
        .map_err(|e| LocrError::EngineError(format!("failed to build ocrs engine: {}", e)))?;

        Ok(Self {
            engine,
            rec_height: input_height(&recognition),
            recognizer: recognition,
            alphabet: DEFAULT_ALPHABET.chars().collect(),
        })
    }

    fn prepare(&self, image: &DynamicImage) -> Result<OcrInput> {
        let converted;
        let (bytes, dimensions) = match image {
            DynamicImage::ImageLuma8(img) => (img.as_raw().as_slice(), img.dimensions()),
            DynamicImage::ImageRgb8(img) => (img.as_raw().as_slice(), img.dimensions()),
            other => {
                converted = other.to_rgb8();
                (converted.as_raw().as_slice(), converted.dimensions())
            }
        };
        let source = ImageSource::from_bytes(bytes, dimensions)
            .map_err(|e| LocrError::EngineError(format!("image preprocessing failed: {}", e)))?;
        self.engine
            .prepare_input(source)
            .map_err(engine_err("ocrs prepare_input"))
    }

    /// Recognize `lines` (word rects from layout analysis), keeping per-character
    /// probabilities. Entries are `None` for lines without recognized text.
    fn recognize_text(
        &self,
        input: &OcrInput,
        lines: &[Vec<RotatedRect>],
        options: &OcrOptions,
    ) -> Result<Vec<Option<Vec<RecChar>>>> {
        // Batch lines with similar widths, as ocrs does: every line in a batch
        // is padded to the batch width, and the padding influences the model.
        let mut order: Vec<(usize, usize)> = lines
            .iter()
            .enumerate()
            .map(|(index, words)| (self.group_width(words), index))
            .collect();
        order.sort_unstable();
        let mut batches: Vec<(usize, Vec<usize>)> = Vec::new();
        for (width, index) in order {
            match batches.last_mut() {
                Some((batch_width, batch))
                    if *batch_width == width && batch.len() < MAX_LINES_PER_BATCH =>
                {
                    batch.push(index)
                }
                _ => batches.push((width, vec![index])),
            }
        }

        let excluded = excluded_labels(&self.alphabet, options.allowed_chars.as_deref());
        let results = rten::thread_pool().run(|| {
            batches
                .into_par_iter()
                .map(|(width, batch)| {
                    self.recognize_batch(input, lines, width, &batch, &excluded, options.beam_width)
                })
                .collect::<Result<Vec<_>>>()
        })?;

        let mut recognized: Vec<Option<Vec<RecChar>>> = (0..lines.len()).map(|_| None).collect();
        for (index, chars) in results.into_iter().flatten() {
            recognized[index] = chars;
        }
        Ok(recognized)
    }

    /// Width a line is padded to in its recognition batch (ocrs' grouping).
    fn group_width(&self, words: &[RotatedRect]) -> usize {
        let (width, height) = bounding_rect(words.iter())
            .map(|r| r.integral_bounding_rect())
            .map_or((0, 0), |r| (r.width(), r.height()));
        resized_line_width(width, height, self.rec_height as i32).next_multiple_of(50) as usize
    }

    fn recognize_batch(
        &self,
        input: &OcrInput,
        lines: &[Vec<RotatedRect>],
        group_width: usize,
        batch: &[usize],
        excluded: &[usize],
        beam_width: u32,
    ) -> Result<Vec<(usize, Option<Vec<RecChar>>)>> {
        let lines = batch
            .iter()
            .map(|&index| {
                let words = &lines[index];
                let image = self
                    .engine
                    .prepare_recognition_input(input, words)
                    .map_err(engine_err("line preparation"))?;
                Ok(PendingLine {
                    index,
                    image,
                    region: Polygon::new(line_polygon(words)),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        // ocrs sizes line images with the same formula as `group_width`; never
        // let a disagreement overflow the batch.
        let group_width = lines
            .iter()
            .map(|l| l.image.size(1))
            .fold(group_width, usize::max);
        let height = lines[0].image.size(0);
        let mut batch = NdTensor::full([lines.len(), 1, height, group_width], BLACK_VALUE);
        for (i, line) in lines.iter().enumerate() {
            batch
                .slice_mut((i, 0, .., ..line.image.size(1)))
                .copy_from(&line.image);
        }

        let input: Tensor<f32> = batch.into();
        let output = self
            .recognizer
            .run_one(input.view().into(), None)
            .map_err(engine_err("text recognition"))?;
        let output: Tensor<f32> = output
            .try_into()
            .map_err(|_| wrong_output("expected a float tensor".into()))?;
        let ndim = output.ndim();
        let mut output: NdTensor<f32, 3> = output
            .try_into()
            .map_err(|_| wrong_output(format!("expected 3 dims but got {ndim}")))?;
        // [seq, batch, class] => [batch, seq, class]
        output.permute([1, 0, 2]);

        if output.size(2) != self.alphabet.len() + 1 {
            return Err(wrong_output(format!(
                "model has {} classes but the alphabet has {} characters (+1 blank)",
                output.size(2),
                self.alphabet.len()
            )));
        }
        let seq_len = output.size(1);
        if seq_len == 0 {
            return Ok(lines.into_iter().map(|line| (line.index, None)).collect());
        }
        // How much the model downsamples the line width (an integer factor, or close).
        let downsample = (group_width as f32 / seq_len as f32).round() as u32;

        let decoder = CtcDecoder::new();
        Ok(lines
            .into_iter()
            .enumerate()
            .map(|(i, line)| {
                let mut log_probs = output.slice([i]).to_tensor();
                for t in 0..seq_len {
                    for &label in excluded {
                        log_probs[[t, label]] = f32::NEG_INFINITY;
                    }
                }
                let hypothesis = if beam_width > 1 {
                    decoder.decode_beam(log_probs.view(), beam_width)
                } else {
                    decoder.decode_greedy(log_probs.view())
                };
                let chars = self.line_chars(&line, &hypothesis, log_probs.view(), downsample);
                (line.index, chars)
            })
            .collect())
    }

    /// Map decoded labels back to characters with image coordinates and probabilities.
    fn line_chars(
        &self,
        line: &PendingLine,
        hypothesis: &CtcHypothesis,
        log_probs: NdTensorView<f32, 2>,
        downsample: u32,
    ) -> Option<Vec<RecChar>> {
        let line_rect = line.region.bounding_rect();
        let resized_width = line.image.size(1) as u32;
        let x_scale = line_rect.width() as f32 / resized_width as f32;
        let steps = hypothesis.steps();

        let chars: Vec<RecChar> = steps
            .iter()
            .enumerate()
            .filter_map(|(i, step)| {
                let next = steps.get(i + 1);
                // X range of the character in the recognition input, then in the image.
                let start_x = step.pos * downsample;
                let end_x = next.map_or(resized_width, |next| next.pos * downsample);
                let [start_x, end_x] =
                    [start_x, end_x].map(|x| line_rect.left() + (x as f32 * x_scale) as i32);

                // Characters predicted over the right-hand padding are not in the line.
                if start_x >= line_rect.right() {
                    return None;
                }

                let char = self
                    .alphabet
                    .get(step.label as usize - 1)
                    .copied()
                    .unwrap_or('?');
                let rect = polygon_slice_bounding_rect(line.region.borrow(), start_x, end_x)
                    .unwrap_or_else(|| {
                        Rect::from_tlbr(line_rect.top(), start_x, line_rect.bottom(), end_x)
                    });
                // Peak probability of the label over the frames it spans.
                let frames =
                    step.pos as usize..next.map_or(log_probs.size(0), |next| next.pos as usize);
                let prob = frames
                    .map(|t| label_probability(log_probs.slice([t]), step.label as usize))
                    .fold(0.0, f32::max);

                Some(RecChar { char, rect, prob })
            })
            .collect();

        (!chars.is_empty()).then_some(chars)
    }
}

impl OcrEngine for OcrsEngine {
    fn recognize_lines(&self, image: &DynamicImage, options: &OcrOptions) -> Result<Vec<Line>> {
        let input = self.prepare(image)?;
        let words = self
            .engine
            .detect_words(&input)
            .map_err(engine_err("text detection"))?;
        let lines = self.engine.find_text_lines(&input, &words);
        let recognized = self.recognize_text(&input, &lines, options)?;
        Ok(recognized
            .into_iter()
            .flatten()
            .filter_map(|chars| build_line(&chars, image.width(), image.height()))
            .collect())
    }
}

/// A text line ready for the recognition model.
struct PendingLine {
    /// Position of the line in reading order.
    index: usize,
    /// Greyscale `[height, width]` line image from ocrs.
    image: NdTensor<f32, 2>,
    /// Region of the page covered by the line.
    region: Polygon,
}

/// A recognized character.
#[derive(Clone, Debug)]
struct RecChar {
    char: char,
    rect: Rect,
    /// Probability of the label, in `[0, 1]`.
    prob: f32,
}

/// Line image height a recognition model expects (ocrs falls back to 50).
fn input_height(model: &Model) -> usize {
    model
        .input_ids()
        .first()
        .and_then(|&id| model.node_info(id))
        .and_then(|info| info.shape())
        .and_then(|shape| match shape.get(2) {
            Some(Dimension::Fixed(height)) => Some(*height),
            _ => None,
        })
        .unwrap_or(50)
}

/// Width to resize a line image to for a given height (port of ocrs').
fn resized_line_width(orig_width: i32, orig_height: i32, height: i32) -> u32 {
    // ocrs' bounds: long lines are squashed beyond 2400px, tiny ones padded to 10px.
    let aspect_ratio = orig_width as f32 / orig_height as f32;
    (height as f32 * aspect_ratio).clamp(10., 2400.) as u32
}

/// Labels of characters outside `allowed`. Spaces stay allowed so that the
/// model can still separate words.
fn excluded_labels(alphabet: &[char], allowed: Option<&str>) -> Vec<usize> {
    let Some(allowed) = allowed else {
        return Vec::new();
    };
    alphabet
        .iter()
        .enumerate()
        .filter(|&(_, &c)| c != ' ' && !allowed.contains(c))
        .map(|(i, _)| i + 1)
        .collect()
}

/// Probability of `label` in one frame of (masked) log-probabilities.
///
/// Normalizes with a softmax, so it is also correct for models that output
/// raw logits, and renormalizes over the characters left by `allowed_chars`.
fn label_probability(log_probs: NdTensorView<f32, 1>, label: usize) -> f32 {
    let max = log_probs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    if !max.is_finite() {
        return 0.0;
    }
    let sum: f32 = log_probs.iter().map(|&lp| (lp - max).exp()).sum();
    let prob = (log_probs[[label]] - max).exp() / sum;
    if prob.is_finite() {
        prob
    } else {
        0.0
    }
}

fn mean(values: impl Iterator<Item = f32>) -> Option<f32> {
    let (sum, n) = values.fold((0.0f32, 0usize), |(sum, n), v| (sum + v, n + 1));
    (n > 0).then(|| sum / n as f32)
}

/// Group a line's characters into words and clamp boxes to the image.
fn build_line(chars: &[RecChar], width: u32, height: u32) -> Option<Line> {
    let to_bbox = |r: Rect| {
        let x0 = r.left().clamp(0, width as i32) as u32;
        let y0 = r.top().clamp(0, height as i32) as u32;
        let x1 = r.right().clamp(0, width as i32) as u32;
        let y1 = r.bottom().clamp(0, height as i32) as u32;
        BoundingBox {
            x: x0,
            y: y0,
            width: x1.saturating_sub(x0),
            height: y1.saturating_sub(y0),
        }
    };

    let words: Vec<Word> = chars
        .split(|c| c.char == ' ')
        .filter(|group| !group.is_empty())
        .map(|group| Word {
            text: group.iter().map(|c| c.char).collect(),
            confidence: mean(group.iter().map(|c| c.prob)),
            bbox: to_bbox(
                group
                    .iter()
                    .skip(1)
                    .fold(group[0].rect, |acc, c| acc.union(c.rect)),
            ),
        })
        .collect();
    let bbox = words.iter().map(|w| w.bbox).reduce(|a, b| a.union(&b))?;

    Some(Line {
        text: chars
            .iter()
            .map(|c| c.char)
            .collect::<String>()
            .trim()
            .to_string(),
        confidence: mean(chars.iter().filter(|c| c.char != ' ').map(|c| c.prob)),
        bbox,
        words,
    })
}

fn load_err(which: &'static str) -> impl Fn(rten::LoadError) -> LocrError {
    move |e| LocrError::EngineError(format!("failed to load {which} model: {e}"))
}

fn engine_err<E: std::fmt::Display>(stage: &'static str) -> impl Fn(E) -> LocrError {
    move |e| LocrError::EngineError(format!("{stage} failed: {e}"))
}

fn wrong_output(detail: String) -> LocrError {
    LocrError::EngineError(format!("unexpected recognition model output: {detail}"))
}

// Geometry helpers below are ports of ocrs' private `geom_util` and
// `recognition` helpers, so character boxes match ocrs exactly.

/// Edge of a rotated rect made of its two left-most vertices.
fn leftmost_edge(r: &RotatedRect) -> LineF {
    let mut corners = r.corners();
    corners.sort_by(|a, b| a.x.total_cmp(&b.x));
    Segment::from_endpoints(corners[0], corners[1])
}

/// Edge of a rotated rect made of its two right-most vertices.
fn rightmost_edge(r: &RotatedRect) -> LineF {
    let mut corners = r.corners();
    corners.sort_by(|a, b| a.x.total_cmp(&b.x));
    Segment::from_endpoints(corners[2], corners[3])
}

/// Orient a line so that its endpoints go from top to bottom.
fn downwards_line(l: LineF) -> LineF {
    if l.start.y <= l.end.y {
        l
    } else {
        Segment::from_endpoints(l.end, l.start)
    }
}

/// Polygon following the top and bottom edges of a line's words, clockwise
/// from the top-left.
fn line_polygon(words: &[RotatedRect]) -> Vec<Point> {
    let floor_point = |p: PointF| Point::from_yx(p.y as i32, p.x as i32);
    let edges = |word: &RotatedRect| {
        (
            downwards_line(leftmost_edge(word)),
            downwards_line(rightmost_edge(word)),
        )
    };

    let mut polygon = Vec::with_capacity(words.len() * 4);
    for word in words {
        let (left, right) = edges(word);
        polygon.push(floor_point(left.start));
        polygon.push(floor_point(right.start));
    }
    for word in words.iter().rev() {
        let (left, right) = edges(word);
        polygon.push(floor_point(right.end));
        polygon.push(floor_point(left.end));
    }
    polygon
}

/// Bounding rect of the slice of `poly` with X coordinates in `[min_x, max_x]`.
fn polygon_slice_bounding_rect(
    poly: Polygon<i32, &[Point]>,
    min_x: i32,
    max_x: i32,
) -> Option<Rect> {
    poly.edges()
        .filter_map(|e| {
            let e = e.rightwards();
            if (e.start.x < min_x && e.end.x < min_x) || (e.start.x > max_x && e.end.x > max_x) {
                return None;
            }
            let start = e
                .to_f32()
                .y_for_x(min_x as f32)
                .map_or(e.start, |y| Point::from_yx(y.round() as i32, min_x));
            let end = e
                .to_f32()
                .y_for_x(max_x as f32)
                .map_or(e.end, |y| Point::from_yx(y.round() as i32, max_x));
            Some(Segment::from_endpoints(start, end))
        })
        .fold(None, |rect: Option<Rect>, e| {
            let edge_rect = e.bounding_rect();
            rect.map(|r| r.union(edge_rect)).or(Some(edge_rect))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocrs::{DecodeMethod, TextItem};

    const SYNTHETIC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/synthetic");

    fn load(name: &str) -> Model {
        Model::load_file(format!("{SYNTHETIC}/{name}")).unwrap()
    }

    fn synthetic_engine() -> OcrsEngine {
        OcrsEngine::from_models_dir(SYNTHETIC).unwrap()
    }

    /// Stock ocrs engine on the same models: the reference implementation.
    fn reference(params: OcrEngineParams) -> OcrsInner {
        OcrsInner::new(OcrEngineParams {
            detection_model: Some(load("text-detection.rten")),
            recognition_model: Some(load("text-recognition.rten")),
            ..params
        })
        .unwrap()
    }

    /// Deterministic xorshift, so pages are reproducible without a `rand` dependency.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self, bound: u32) -> u32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 % bound as u64) as u32
        }
    }

    /// White page with `rows` lines of dark-grey "glyphs" grouped into words.
    fn page(seed: u64, rows: u32, row_pitch: u32, max_words: u32) -> image::RgbImage {
        let mut rng = Rng(seed);
        let mut img =
            image::RgbImage::from_pixel(600, 20 + rows * row_pitch, image::Rgb([255, 255, 255]));
        for row in 0..rows {
            let y0 = 10 + row * row_pitch;
            let mut x = 10 + rng.next(40);
            for _ in 0..1 + rng.next(max_words) {
                for _ in 0..1 + rng.next(6) {
                    let (w, h) = (6 + rng.next(10), 12 + rng.next(6));
                    if x + w >= 590 {
                        break;
                    }
                    let grey = (15 + rng.next(110)) as u8;
                    for y in y0..y0 + h {
                        for gx in x..x + w {
                            img.put_pixel(gx, y, image::Rgb([grey, grey, grey]));
                        }
                    }
                    x += w + 3 + rng.next(4);
                }
                x += 20 + rng.next(20);
            }
        }
        img
    }

    fn input_and_lines(
        engine: &OcrsEngine,
        img: &image::RgbImage,
    ) -> (OcrInput, Vec<Vec<RotatedRect>>) {
        let input = engine
            .prepare(&DynamicImage::ImageRgb8(img.clone()))
            .unwrap();
        let words = engine.engine.detect_words(&input).unwrap();
        let lines = engine.engine.find_text_lines(&input, &words);
        (input, lines)
    }

    /// Recognize with both implementations and require identical characters and boxes.
    fn assert_matches_ocrs(
        engine: &OcrsEngine,
        reference: &OcrsInner,
        options: &OcrOptions,
        img: &image::RgbImage,
    ) -> usize {
        let (input, lines) = input_and_lines(engine, img);
        let expected = reference.recognize_text(&input, &lines).unwrap();
        let actual = engine.recognize_text(&input, &lines, options).unwrap();
        assert_eq!(expected.len(), actual.len());

        let mut chars = 0;
        for (i, (expected, actual)) in expected.iter().zip(&actual).enumerate() {
            match (expected, actual) {
                (None, None) => {}
                (Some(expected), Some(actual)) => {
                    let expected: Vec<(char, Rect)> =
                        expected.chars().iter().map(|c| (c.char, c.rect)).collect();
                    // ocrs prints the model's '€' label as 'E'.
                    let actual: Vec<(char, Rect)> = actual
                        .iter()
                        .map(|c| (if c.char == '€' { 'E' } else { c.char }, c.rect))
                        .collect();
                    assert_eq!(expected, actual, "line {i}");
                    chars += actual.len();
                }
                _ => panic!(
                    "line {i}: ocrs={} locr={}",
                    expected.is_some(),
                    actual.is_some()
                ),
            }
        }
        chars
    }

    #[test]
    fn recognition_matches_ocrs() {
        let engine = synthetic_engine();
        let reference = reference(OcrEngineParams::default());
        let mut total = 0;
        // Varied widths -> several batch groups; 25 short rows -> a group split into batches of 20.
        for (seed, rows, pitch, words) in
            [(1, 6, 60, 5), (2, 12, 40, 3), (3, 25, 30, 1), (4, 3, 90, 8)]
        {
            let img = page(seed, rows, pitch, words);
            total += assert_matches_ocrs(&engine, &reference, &OcrOptions::default(), &img);
        }
        assert!(
            total > 100,
            "too few characters to be a meaningful comparison: {total}"
        );
    }

    #[test]
    fn allowed_chars_match_ocrs() {
        let engine = synthetic_engine();
        // locr always keeps spaces; ocrs needs them listed explicitly.
        let reference = reference(OcrEngineParams {
            allowed_chars: Some("0123456789 ".into()),
            ..Default::default()
        });
        let options = OcrOptions {
            allowed_chars: Some("0123456789".into()),
            ..Default::default()
        };
        let img = page(5, 8, 50, 4);
        assert!(assert_matches_ocrs(&engine, &reference, &options, &img) > 0);

        let (input, lines) = input_and_lines(&engine, &img);
        for chars in engine
            .recognize_text(&input, &lines, &options)
            .unwrap()
            .into_iter()
            .flatten()
        {
            assert!(chars
                .iter()
                .all(|c| c.char.is_ascii_digit() || c.char == ' '));
        }
    }

    #[test]
    fn beam_search_matches_ocrs() {
        let engine = synthetic_engine();
        let reference = reference(OcrEngineParams {
            decode_method: DecodeMethod::BeamSearch { width: 5 },
            ..Default::default()
        });
        let options = OcrOptions {
            beam_width: 5,
            ..Default::default()
        };
        assert!(assert_matches_ocrs(&engine, &reference, &options, &page(6, 6, 60, 4)) > 0);
    }

    #[test]
    fn probabilities_are_valid() {
        let engine = synthetic_engine();
        let (input, lines) = input_and_lines(&engine, &page(7, 6, 60, 4));
        let chars: Vec<RecChar> = engine
            .recognize_text(&input, &lines, &OcrOptions::default())
            .unwrap()
            .into_iter()
            .flatten()
            .flatten()
            .collect();
        assert!(!chars.is_empty());
        assert!(
            chars.iter().all(|c| c.prob > 0.0 && c.prob <= 1.0),
            "{chars:?}"
        );
    }

    #[test]
    fn label_probability_is_a_softmax() {
        let logits = NdTensor::from([1.0f32, 2.0, f32::NEG_INFINITY, 3.0]);
        let p: Vec<f32> = (0..4)
            .map(|l| label_probability(logits.view(), l))
            .collect();
        assert!((p.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        assert_eq!(p[2], 0.0);
        assert!(p[3] > p[1] && p[1] > p[0]);
        let masked = NdTensor::from([f32::NEG_INFINITY; 3]);
        assert_eq!(label_probability(masked.view(), 0), 0.0);
    }

    #[test]
    fn excluded_labels_keep_spaces() {
        let alphabet: Vec<char> = " 01a".chars().collect();
        assert!(excluded_labels(&alphabet, None).is_empty());
        assert_eq!(excluded_labels(&alphabet, Some("1")), vec![2, 4]);
    }

    #[test]
    fn build_line_groups_words_and_clamps() {
        let c = |char, left, right, prob| RecChar {
            char,
            rect: Rect::from_tlbr(-2, left, 12, right),
            prob,
        };
        let chars = [
            c(' ', 0, 5, 0.1),
            c('a', 5, 10, 0.8),
            c('b', 10, 15, 0.6),
            c(' ', 15, 20, 0.2),
            c('c', 20, 30, 1.0),
        ];
        let line = build_line(&chars, 25, 10).unwrap();
        assert_eq!(line.text, "ab c");
        assert_eq!(line.words.len(), 2);
        assert_eq!(
            line.words[0].bbox,
            BoundingBox {
                x: 5,
                y: 0,
                width: 10,
                height: 10
            }
        );
        assert_eq!(
            line.words[1].bbox,
            BoundingBox {
                x: 20,
                y: 0,
                width: 5,
                height: 10
            }
        );
        assert!((line.words[0].confidence.unwrap() - 0.7).abs() < 1e-6);
        assert!((line.confidence.unwrap() - 0.8).abs() < 1e-6);
        assert_eq!(
            line.bbox,
            BoundingBox {
                x: 5,
                y: 0,
                width: 20,
                height: 10
            }
        );
        assert!(build_line(&[c(' ', 0, 5, 0.5)], 25, 10).is_none());
    }

    #[test]
    fn wrong_alphabet_is_reported() {
        let engine = synthetic_engine().with_alphabet("abc");
        let img = DynamicImage::ImageRgb8(page(8, 2, 60, 2));
        let err = engine
            .recognize_lines(&img, &OcrOptions::default())
            .unwrap_err();
        assert!(matches!(err, LocrError::EngineError(msg) if msg.contains("classes")));
    }

    #[test]
    fn missing_model_files_are_reported() {
        let err = OcrsEngine::from_models_dir("/nonexistent/locr-models").unwrap_err();
        assert!(matches!(err, LocrError::EngineError(msg) if msg.contains("text-detection.rten")));
        let err = OcrsEngine::from_model_bytes(b"junk".to_vec(), b"junk".to_vec()).unwrap_err();
        assert!(matches!(err, LocrError::EngineError(msg) if msg.contains("detection model")));
    }
}
