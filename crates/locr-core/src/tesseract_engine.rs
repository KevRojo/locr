use image::DynamicImage;

use crate::{BoundingBox, Line, LocrError, OcrEngine, OcrOptions, Result, Word};

/// Default engine using the local `tesseract` CLI.
#[derive(Debug, Default, Clone, Copy)]
pub struct TesseractCliEngine;

impl TesseractCliEngine {
    pub fn new() -> Self {
        Self
    }
}

impl OcrEngine for TesseractCliEngine {
    fn recognize_lines(&self, image: &DynamicImage, options: &OcrOptions) -> Result<Vec<Line>> {
        let mut temp = tempfile::NamedTempFile::with_suffix(".png")
            .map_err(|e| LocrError::EngineError(e.to_string()))?;
        image
            .write_to(&mut temp, image::ImageFormat::Png)
            .map_err(|e| LocrError::EngineError(e.to_string()))?;

        let mut command = std::process::Command::new("tesseract");
        command.arg(temp.path()).arg("stdout").arg("-l").arg("eng");
        if let Some(allowed) = &options.allowed_chars {
            command
                .arg("-c")
                .arg(format!("tessedit_char_whitelist={allowed}"));
        }
        // Word-level boxes and confidence instead of plain text.
        command.arg("tsv");

        let output = command.output().map_err(|e| {
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

        let tsv =
            String::from_utf8(output.stdout).map_err(|e| LocrError::EngineError(e.to_string()))?;
        Ok(parse_tsv(&tsv))
    }
}

/// Parse `tesseract ... tsv` output into lines of words.
///
/// Columns: level page_num block_num par_num line_num word_num left top
/// width height conf text. Level 5 rows are words; `conf` is 0-100, or -1
/// when unknown.
fn parse_tsv(tsv: &str) -> Vec<Line> {
    let mut lines: Vec<([u32; 4], Line)> = Vec::new();

    for (key, word) in tsv.lines().skip(1).filter_map(parse_word_row) {
        match lines.last_mut() {
            Some((last_key, line)) if *last_key == key => {
                line.text.push(' ');
                line.text.push_str(&word.text);
                line.bbox = line.bbox.union(&word.bbox);
                line.words.push(word);
            }
            _ => lines.push((
                key,
                Line {
                    text: word.text.clone(),
                    confidence: None,
                    bbox: word.bbox,
                    words: vec![word],
                },
            )),
        }
    }

    lines
        .into_iter()
        .map(|(_, mut line)| {
            // Mean over characters, like the ocrs engine.
            let (sum, n) = line
                .words
                .iter()
                .filter_map(|w| Some((w.confidence?, w.text.chars().count() as f32)))
                .fold((0.0, 0.0), |(sum, n), (c, len)| (sum + c * len, n + len));
            line.confidence = (n > 0.0).then(|| sum / n);
            line
        })
        .collect()
}

/// A level-5 (word) row: its line key (page, block, paragraph, line) and the word.
fn parse_word_row(row: &str) -> Option<([u32; 4], Word)> {
    let cols: Vec<&str> = row.splitn(12, '\t').collect();
    let [level, page, block, par, line, _word, left, top, width, height, conf, text] = cols[..]
    else {
        return None;
    };
    let text = text.trim();
    if level != "5" || text.is_empty() {
        return None;
    }
    let num = |s: &str| s.trim().parse::<u32>().ok();
    let key = [num(page)?, num(block)?, num(par)?, num(line)?];
    let bbox = BoundingBox {
        x: num(left)?,
        y: num(top)?,
        width: num(width)?,
        height: num(height)?,
    };
    let confidence = conf
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|c| *c >= 0.0)
        .map(|c| (c / 100.0).min(1.0));
    Some((
        key,
        Word {
            text: text.to_string(),
            confidence,
            bbox,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_words_into_lines() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
                   1\t1\t0\t0\t0\t0\t0\t0\t400\t120\t-1\t\n\
                   4\t1\t1\t1\t1\t0\t20\t30\t200\t40\t-1\t\n\
                   5\t1\t1\t1\t1\t1\t20\t30\t90\t40\t96.5\tHELLO\n\
                   5\t1\t1\t1\t1\t2\t130\t32\t90\t38\t90\tLOCR\n\
                   5\t1\t1\t1\t2\t1\t20\t80\t50\t20\t-1\tbye\n\
                   5\t1\t1\t1\t2\t2\t80\t80\t10\t20\t95\t \n";
        let lines = parse_tsv(tsv);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "HELLO LOCR");
        assert_eq!(
            lines[0].bbox,
            BoundingBox {
                x: 20,
                y: 30,
                width: 200,
                height: 40
            }
        );
        assert_eq!(lines[0].words[1].confidence, Some(0.9));
        let expected = (0.965 * 5.0 + 0.9 * 4.0) / 9.0;
        assert!((lines[0].confidence.unwrap() - expected).abs() < 1e-6);
        assert_eq!(lines[1].text, "bye");
        assert_eq!(lines[1].confidence, None);
    }

    #[test]
    fn ignores_malformed_rows() {
        assert!(parse_tsv("header\n5\t1\tx\n5\t1\t1\t1\t1\t1\ta\tb\tc\td\t9\tword\n").is_empty());
    }
}
