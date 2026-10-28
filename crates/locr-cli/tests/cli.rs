//! Drive the `locr` binary with the tiny synthetic models from locr-core.

use std::io::Write;
use std::process::{Command, Output, Stdio};

const HELLO_PNG: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../locr-core/tests/fixtures/hello.png"
);
const MODELS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../locr-core/tests/fixtures/synthetic"
);

fn locr(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_locr"))
        .args(args)
        .env_remove("LOCR_MODELS_DIR")
        .output()
        .unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).unwrap()
}

fn stderr(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).unwrap()
}

#[test]
fn prints_text() {
    let out = locr(&["--models", MODELS, HELLO_PNG]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).ends_with('\n'));
    assert!(!stdout(&out).contains("==>"));
}

#[test]
fn json_lines_with_source() {
    let out = locr(&[
        "--models", MODELS, "--json", "--beam", "3", HELLO_PNG, HELLO_PNG,
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    let lines: Vec<serde_json::Value> = stdout(&out)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    for json in lines {
        assert_eq!(json["version"], 1);
        assert_eq!(json["source"], HELLO_PNG);
        assert_eq!(
            (json["width"].as_u64(), json["height"].as_u64()),
            (Some(400), Some(120))
        );
        assert!(json["lines"].is_array());
    }
}

#[test]
fn several_images_get_headers() {
    let out = locr(&["--models", MODELS, HELLO_PNG, HELLO_PNG]);
    assert!(out.status.success());
    assert_eq!(
        stdout(&out)
            .matches(&format!("==> {HELLO_PNG} <=="))
            .count(),
        2
    );
}

#[test]
fn reads_stdin() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_locr"))
        .args(["--models", MODELS, "--json", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&std::fs::read(HELLO_PNG).unwrap())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let json: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(json["source"], "-");
}

#[test]
fn options_reach_the_engine() {
    let out = locr(&[
        "--models",
        MODELS,
        "--json",
        "--auto",
        "--min-score",
        "2",
        "--max-attempts",
        "2",
        "--allowed-chars",
        "0123456789",
        HELLO_PNG,
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    let json: serde_json::Value = serde_json::from_str(stdout(&out).trim()).unwrap();
    assert_eq!(json["attempts"], 2);
    let text = json["text"].as_str().unwrap();
    assert!(
        text.chars()
            .all(|c| c.is_ascii_digit() || c.is_whitespace()),
        "{text:?}"
    );
}

#[test]
fn bad_files_fail_but_others_still_run() {
    let out = locr(&["--models", MODELS, "/nonexistent.png", HELLO_PNG]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("/nonexistent.png"));
    assert!(stdout(&out).contains(&format!("==> {HELLO_PNG} <==")));

    let out = locr(&["--models", MODELS, env!("CARGO_MANIFEST_DIR")]);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn usage_errors() {
    assert_eq!(locr(&[]).status.code(), Some(2));
    assert_eq!(locr(&["--bogus", HELLO_PNG]).status.code(), Some(2));
    assert_eq!(locr(&["--beam", "wide", HELLO_PNG]).status.code(), Some(2));
    assert_eq!(
        locr(&["--beam", "100000", HELLO_PNG]).status.code(),
        Some(2)
    );
    let out = locr(&["--help"]);
    assert!(out.status.success() && stdout(&out).contains("Usage: locr"));
    let out = locr(&["--version"]);
    assert_eq!(
        stdout(&out).trim(),
        format!("locr {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn missing_models_are_reported() {
    let out = locr(&["--models", "/nonexistent-models", HELLO_PNG]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("text-detection.onnx"));
}
