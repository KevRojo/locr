//! Download or copy the ocrs models so they can be embedded with `include_bytes!`.
//!
//! The models are licensed separately (CC-BY-SA 4.0). See `NOTICE-MODELS` in the crate root.
//! Downloads are integrity-checked with pinned SHA-256 digests before embedding.

use std::env;
use std::fs;
use std::io::Read;
use std::path::PathBuf;

/// (filename, download URL, expected SHA-256 hex)
const MODELS: &[(&str, &str, &str)] = &[
    (
        "text-detection.rten",
        "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten",
        "f15cfb56bd02c4bf478a20343986504a1f01e1665c2b3a0ad66340f054b1b5ca",
    ),
    (
        "text-recognition.rten",
        "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten",
        "e484866d4cce403175bd8d00b128feb08ab42e208de30e42cd9889d8f1735a6e",
    ),
];

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn verify_or_panic(name: &str, bytes: &[u8], expected: &str) {
    let got = sha256_hex(bytes);
    if got != expected {
        panic!(
            "SHA-256 mismatch for model {name}: expected {expected}, got {got}. \
             Refusing to embed untrusted bytes."
        );
    }
}

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set"));

    // Only bundle models when the `bundle-models` feature is enabled.
    if env::var("CARGO_FEATURE_BUNDLE_MODELS").is_err() {
        return;
    }

    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set"));
    let assets_dir = crate_dir.join("assets");

    for (name, url, expected_sha) in MODELS {
        let out_path = out_dir.join(name);

        if out_path.exists() {
            let existing = fs::read(&out_path).unwrap_or_else(|e| {
                panic!("failed to read existing model {}: {}", name, e);
            });
            verify_or_panic(name, &existing, expected_sha);
            continue;
        }

        // Prefer a pre-downloaded asset next to Cargo.toml (offline/reproducible builds).
        let asset_path = assets_dir.join(name);
        if asset_path.exists() {
            let bytes = fs::read(&asset_path).unwrap_or_else(|e| {
                panic!("failed to read asset {}: {}", name, e);
            });
            verify_or_panic(name, &bytes, expected_sha);
            fs::write(&out_path, &bytes).unwrap_or_else(|e| {
                panic!("failed to copy {} to OUT_DIR: {}", name, e);
            });
            continue;
        }

        // Otherwise download from the public model repository, then verify.
        println!("cargo:warning=downloading model {} from {}", name, url);
        let response = ureq::get(url)
            .call()
            .unwrap_or_else(|e| panic!("failed to download {}: {}", name, e));
        let mut reader = response.into_reader();
        let mut reader = Read::take(&mut reader, 50_000_000);
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .unwrap_or_else(|e| panic!("failed to read download for {}: {}", name, e));
        if bytes.is_empty() {
            panic!("model {} download was empty", name);
        }
        verify_or_panic(name, &bytes, expected_sha);
        fs::write(&out_path, &bytes).unwrap_or_else(|e| {
            panic!("failed to write {}: {}", name, e);
        });
        println!(
            "cargo:warning=downloaded {} ({} bytes, sha256 ok)",
            name,
            bytes.len()
        );
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets");
}
