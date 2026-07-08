//! Download or copy the ocrs models so they can be embedded with `include_bytes!`.
//!
//! The models are licensed separately (CC-BY-SA 4.0). See `NOTICE-MODELS` in the crate root.

use std::env;
use std::fs;
use std::io::Read;
use std::path::PathBuf;

const MODELS: &[(&str, &str)] = &[
    (
        "text-detection.rten",
        "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten",
    ),
    (
        "text-recognition.rten",
        "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten",
    ),
];

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set"));

    // Only bundle models when the `bundle-models` feature is enabled.
    if env::var("CARGO_FEATURE_BUNDLE_MODELS").is_err() {
        return;
    }

    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set"));
    let assets_dir = crate_dir.join("assets");

    for (name, url) in MODELS {
        let out_path = out_dir.join(name);
        if out_path.exists() {
            continue;
        }

        // Prefer a pre-downloaded asset next to Cargo.toml (useful for offline/reproducible builds).
        let asset_path = assets_dir.join(name);
        if asset_path.exists() {
            fs::copy(&asset_path, &out_path).unwrap_or_else(|e| {
                panic!("failed to copy {} to OUT_DIR: {}", name, e);
            });
            continue;
        }

        // Otherwise download from the public model repository.
        println!("cargo:warning=downloading model {} from {}", name, url);
        let response = ureq::get(url)
            .call()
            .unwrap_or_else(|e| panic!("failed to download {}: {}", name, e));
        let mut file = fs::File::create(&out_path).unwrap_or_else(|e| {
            panic!("failed to create {} in OUT_DIR: {}", name, e);
        });
        let mut reader = response.into_reader();
        let mut reader = Read::take(&mut reader, 50_000_000);
        let bytes_copied = std::io::copy(&mut reader, &mut file)
            .unwrap_or_else(|e| panic!("failed to write {}: {}", name, e));
        if bytes_copied == 0 {
            panic!("model {} download was empty", name);
        }
        println!("cargo:warning=downloaded {} ({} bytes)", name, bytes_copied);
    }

    println!("cargo:rerun-if-changed=build.rs");
}
