//! `locr` — local OCR from the command line. Images in, text or JSON out.

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use locr_core::{EnhanceOptions, Locr, OcrOptions, OcrsEngine};

const HELP: &str = "\
locr — local OCR. Images in, text out. Nothing leaves your machine.

Usage: locr [OPTIONS] <IMAGE>...

Arguments:
  <IMAGE>...              PNG, JPEG, WEBP, BMP, TIFF, GIF, ... files; - reads stdin

Options:
  -j, --json              Full result (lines, words, boxes, confidence) as JSON,
                          one object per image and line of output
  -a, --auto              Retry with contrast/brightness/... transforms when the
                          score is low
      --min-score <F>     Score below which --auto retries [default: 0.55]
      --max-attempts <N>  OCR passes for --auto, including the first [default: 7]
      --allowed-chars <S> Only emit these characters, e.g. 0123456789
      --beam <N>          CTC beam search width [default: greedy]
      --models <DIR>      Load text-detection.rten and text-recognition.rten from DIR
  -h, --help              Print help
  -V, --version           Print version
";

struct Args {
    images: Vec<String>,
    json: bool,
    models: Option<PathBuf>,
    options: OcrOptions,
}

fn parse_args() -> Result<Option<Args>, lexopt::Error> {
    use lexopt::prelude::*;

    let mut images = Vec::new();
    let (mut json, mut auto, mut models) = (false, false, None);
    let mut enhance = EnhanceOptions::default();
    let mut options = OcrOptions::default();

    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Short('j') | Long("json") => json = true,
            Short('a') | Long("auto") => auto = true,
            Long("min-score") => enhance.min_score = parser.value()?.parse()?,
            Long("max-attempts") => enhance.max_attempts = parser.value()?.parse()?,
            Long("allowed-chars") => options.allowed_chars = Some(parser.value()?.string()?),
            Long("beam") => options.beam_width = parser.value()?.parse()?,
            Long("models") => models = Some(PathBuf::from(parser.value()?)),
            Short('h') | Long("help") => {
                print!("{HELP}");
                return Ok(None);
            }
            Short('V') | Long("version") => {
                println!("locr {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            Value(image) => images.push(image.string()?),
            _ => return Err(arg.unexpected()),
        }
    }
    if images.is_empty() {
        return Err("no images given (try --help)".into());
    }
    options.enhance = auto.then_some(enhance);
    options.validate().map_err(|e| e.to_string())?;
    Ok(Some(Args {
        images,
        json,
        models,
        options,
    }))
}

fn read_image(source: &str) -> io::Result<Vec<u8>> {
    if source == "-" {
        let mut bytes = Vec::new();
        io::stdin().lock().read_to_end(&mut bytes)?;
        Ok(bytes)
    } else {
        std::fs::read(source)
    }
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(Some(args)) => args,
        Ok(None) => return ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("locr: {err}");
            return ExitCode::from(2);
        }
    };

    let owned;
    let locr: &Locr<OcrsEngine> = match &args.models {
        Some(dir) => match OcrsEngine::from_models_dir(dir) {
            Ok(engine) => {
                owned = Locr::new(engine);
                &owned
            }
            Err(err) => {
                eprintln!("locr: {err}");
                return ExitCode::FAILURE;
            }
        },
        None => match locr_core::shared() {
            Ok(locr) => locr,
            Err(err) => {
                eprintln!("locr: {err}");
                return ExitCode::FAILURE;
            }
        },
    };

    let mut stdout = io::stdout().lock();
    let mut failed = false;
    for (i, source) in args.images.iter().enumerate() {
        let result = read_image(source)
            .map_err(|e| e.to_string())
            .and_then(|bytes| {
                locr.image_to_data(&bytes, &args.options)
                    .map_err(|e| e.to_string())
            });
        let result = match result {
            Ok(result) => result,
            Err(err) => {
                eprintln!("locr: {source}: {err}");
                failed = true;
                continue;
            }
        };

        let written = if args.json {
            // Prepend "source" to the schema object, keeping its field order.
            let json = result.to_json();
            let source = serde_json::to_string(source).expect("strings serialize");
            writeln!(stdout, "{{\"source\":{source},{}", &json[1..])
        } else if args.images.len() > 1 {
            let gap = if i > 0 { "\n" } else { "" };
            writeln!(stdout, "{gap}==> {source} <==\n{}", result.text)
        } else {
            writeln!(stdout, "{}", result.text)
        };
        if written.is_err() {
            // Downstream closed the pipe (e.g. `| head`): stop quietly.
            break;
        }
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
