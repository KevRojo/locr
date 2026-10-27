//! Download or copy the ocrs models so they can be embedded with `include_bytes!`.
//!
//! The models are licensed separately (CC-BY-SA 4.0). See `NOTICE-MODELS` in the crate root.
//! Every model is integrity-checked against a pinned SHA-256 digest before embedding,
//! whatever its source. Sources, in order:
//!
//! 1. `$OUT_DIR` (already verified by a previous build)
//! 2. `$LOCR_MODELS_DIR` (offline / air-gapped / distro builds)
//! 3. `assets/` next to `Cargo.toml` (offline / reproducible builds)
//! 4. the public ocrs model bucket

use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

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

fn copy_verified(name: &str, src: &Path, dst: &Path, expected_sha: &str) {
    let bytes = fs::read(src).unwrap_or_else(|e| panic!("failed to read {}: {}", src.display(), e));
    verify_or_panic(name, &bytes, expected_sha);
    fs::write(dst, &bytes).unwrap_or_else(|e| panic!("failed to copy {} to OUT_DIR: {}", name, e));
}

fn download(name: &str, url: &str) -> Vec<u8> {
    println!("cargo:warning=downloading model {} from {}", name, url);
    let response = ureq::get(url).call().unwrap_or_else(|e| {
        panic!(
            "failed to download {name}: {e}\n\
             Offline? Put text-detection.rten and text-recognition.rten in \
             crates/locr-core/assets/ or in a directory named by LOCR_MODELS_DIR, \
             or build without the `bundle-models` feature and load models at runtime."
        )
    });
    let mut reader = response.into_reader();
    let mut reader = Read::take(&mut reader, 50_000_000);
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .unwrap_or_else(|e| panic!("failed to read download for {}: {}", name, e));
    if bytes.is_empty() {
        panic!("model {} download was empty", name);
    }
    bytes
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets");
    println!("cargo:rerun-if-env-changed=LOCR_MODELS_DIR");

    // Only bundle models when the `bundle-models` feature is enabled.
    if env::var("CARGO_FEATURE_BUNDLE_MODELS").is_err() {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set"));

    // docs.rs builds have no network access and never run the code: embed
    // empty placeholders so `include_bytes!` resolves.
    if env::var_os("DOCS_RS").is_some() {
        for (name, _, _) in MODELS {
            fs::write(out_dir.join(name), b"").expect("failed to write placeholder model");
        }
        return;
    }

    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set"));
    let models_dir = env::var_os("LOCR_MODELS_DIR").map(PathBuf::from);
    let assets_dir = crate_dir.join("assets");

    for (name, url, expected_sha) in MODELS {
        let out_path = out_dir.join(name);

        if out_path.exists() {
            let existing = fs::read(&out_path).unwrap_or_else(|e| {
                panic!("failed to read existing model {}: {}", name, e);
            });
            // A stale placeholder or partial file is re-fetched below.
            if sha256_hex(&existing) == *expected_sha {
                continue;
            }
        }

        // Prefer pre-downloaded copies (offline/reproducible builds).
        let local = models_dir
            .iter()
            .chain(std::iter::once(&assets_dir))
            .map(|dir| dir.join(name))
            .find(|path| path.exists());
        if let Some(path) = local {
            copy_verified(name, &path, &out_path, expected_sha);
            continue;
        }

        // Otherwise download from the public model repository, then verify.
        let bytes = download(name, url);
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
}
