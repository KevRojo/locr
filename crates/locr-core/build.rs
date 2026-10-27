//! Download or copy the ocrs models so they can be embedded with `include_bytes!`.
//!
//! The models are licensed separately (CC-BY-SA 4.0). See `NOTICE-MODELS` in the crate root.
//! Every model is integrity-checked against the SHA-256 digest pinned in `models.sha256`
//! before embedding, whatever its source. Sources, in order:
//!
//! 1. `$OUT_DIR` (already verified by a previous build)
//! 2. `$LOCR_BUILD_MODELS_DIR` (offline / air-gapped / distro builds). Not
//!    `LOCR_MODELS_DIR`: that one names custom models loaded at runtime.
//! 3. `assets/` next to `Cargo.toml` (offline / reproducible builds)
//! 4. the public ocrs model bucket

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Public bucket of the ocrs project. It serves the models in ONNX format
/// since ocrs 0.13 (the older `.rten` files are gone).
const MODEL_BASE_URL: &str = "https://ocrs-models.s3-accelerate.amazonaws.com";
const MODEL_FILES: [&str; 2] = ["text-detection.onnx", "text-recognition.onnx"];

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Digests from `models.sha256`: `<sha256>  <file>` lines, `#` comments.
/// Entries spelled `unknown` are treated as not pinned.
fn pinned_digests(crate_dir: &Path) -> HashMap<String, String> {
    let path = crate_dir.join("models.sha256");
    let pins = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));
    pins.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let (digest, file) = (fields.next()?, fields.next()?);
            (digest != "unknown").then(|| (file.to_string(), digest.to_lowercase()))
        })
        .collect()
}

fn verify_or_panic(name: &str, bytes: &[u8], expected: Option<&str>) {
    let got = sha256_hex(bytes);
    match expected {
        Some(expected) if expected == got => {}
        Some(expected) => panic!(
            "SHA-256 mismatch for model {name}: expected {expected}, got {got}. \
             Refusing to embed untrusted bytes."
        ),
        None => panic!(
            "model {name} is not pinned in crates/locr-core/models.sha256 \
             (SHA-256 of the file found: {got}). Verify it and pin that digest."
        ),
    }
}

fn copy_verified(name: &str, src: &Path, dst: &Path, expected_sha: Option<&str>) {
    let bytes = fs::read(src).unwrap_or_else(|e| panic!("failed to read {}: {}", src.display(), e));
    verify_or_panic(name, &bytes, expected_sha);
    fs::write(dst, &bytes).unwrap_or_else(|e| panic!("failed to copy {} to OUT_DIR: {}", name, e));
}

fn download(name: &str, url: &str) -> Vec<u8> {
    println!("cargo:warning=downloading model {} from {}", name, url);
    let response = ureq::get(url).call().unwrap_or_else(|e| {
        panic!(
            "failed to download {name}: {e}\n\
             Offline? Put {files} in crates/locr-core/assets/ or in a directory \
             named by LOCR_BUILD_MODELS_DIR, or build without the `bundle-models` \
             feature and load models at runtime.",
            files = MODEL_FILES.join(" and "),
        )
    });
    let mut reader = response.into_reader();
    let mut reader = Read::take(&mut reader, 100_000_000);
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
    println!("cargo:rerun-if-changed=models.sha256");
    println!("cargo:rerun-if-env-changed=LOCR_BUILD_MODELS_DIR");

    // Only bundle models when the `bundle-models` feature is enabled.
    if env::var("CARGO_FEATURE_BUNDLE_MODELS").is_err() {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set"));

    // docs.rs builds have no network access and never run the code: embed
    // empty placeholders so `include_bytes!` resolves.
    if env::var_os("DOCS_RS").is_some() {
        for name in MODEL_FILES {
            fs::write(out_dir.join(name), b"").expect("failed to write placeholder model");
        }
        return;
    }

    let crate_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set"));
    let pins = pinned_digests(&crate_dir);
    let models_dir = env::var_os("LOCR_BUILD_MODELS_DIR").map(PathBuf::from);
    let assets_dir = crate_dir.join("assets");
    // A missing path would mark the crate dirty on every build.
    if assets_dir.exists() {
        println!("cargo:rerun-if-changed=assets");
    }

    for name in MODEL_FILES {
        let expected_sha = pins.get(name).map(String::as_str);
        let out_path = out_dir.join(name);

        if out_path.exists() {
            let existing = fs::read(&out_path).unwrap_or_else(|e| {
                panic!("failed to read existing model {}: {}", name, e);
            });
            // A stale placeholder or partial file is re-fetched below.
            if Some(sha256_hex(&existing).as_str()) == expected_sha {
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
        let bytes = download(name, &format!("{MODEL_BASE_URL}/{name}"));
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
