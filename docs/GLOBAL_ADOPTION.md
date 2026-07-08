# locr — Global Adoption Plan

> Goal: any engineer at any company — from a solo dev to Google or Microsoft — can add `locr`
> to their stack in under 5 minutes, in their own language, through their own package manager,
> and pass their own security review. Zero friction. Zero cloud. Zero paywall.

## 1. The universal core: one engine, every ABI

The Rust core (`crates/locr-core`) is the single source of truth. Everything else is a thin shell.

| Layer | Tech | Who it unlocks |
|-------|------|----------------|
| **C ABI** (`locr-ffi`, `cdylib` + `cbindgen` header) | stable `extern "C"` surface | C, C++, C#/.NET (P/Invoke), Java (JNI/Panama), Go (cgo), Swift, Zig, PHP, Ruby, anything alive since 1972 |
| **WASM** (`locr-wasm`, wasm-bindgen + WASI) | `wasm32-unknown-unknown` + `wasm32-wasi` | Browsers, Deno, Bun, Cloudflare Workers, edge, plugins/sandboxes |
| **UniFFI** (Mozilla) | auto-generated bindings | Swift (iOS/macOS), Kotlin (Android), Python — same tooling Firefox ships with |
| **Native wrappers** | npm (N-API prebuilds), pip (PyO3/maturin wheels) | The two biggest dev ecosystems, already scaffolded |

The C ABI is the key move: it is the *lingua franca* of every corporation's build system.
Frozen, versioned (`locr_version()`), semver-guaranteed:

```c
// locr.h — the whole standard in 4 functions
LocrResult locr_image_to_text(const uint8_t* bytes, size_t len, char** out_text);
void       locr_free_text(char* text);
const char* locr_version(void);
LocrResult locr_image_to_text_with_opts(const uint8_t* bytes, size_t len, const LocrOpts* opts, char** out_text);
```

## 2. Distribution matrix — meet every dev where they already are

| Channel | Artifact | Status |
|---------|----------|--------|
| crates.io | `locr-core`, `locr-ffi`, `locr-wasm` | release.yml ready |
| npm | `locr` (WASM + N-API prebuilds per platform) | scaffolded |
| PyPI | `locr` (abi3 wheels: manylinux/musllinux, macOS universal2, Windows) | scaffolded |
| NuGet | `Locr` (.NET wrapper over C ABI, runtimes/* native assets) | planned |
| Maven Central | `dev.locr:locr` (JNI/JNA over C ABI) | planned |
| Go | `github.com/KevRojo/locr-go` (cgo, vendored static lib) | planned |
| Homebrew | `brew install locr` (CLI + dylib) | planned |
| winget | `winget install locr` | planned |
| vcpkg / conan | C/C++ consumption (Microsoft's own C++ package manager) | planned |
| conda-forge | scientific/data stack | planned |
| apt/rpm (via cargo-deb / cargo-generate-rpm) | Linux distros | planned |
| Docker / OCI | `ghcr.io/kevrojo/locr` (CLI image, scratch-based, <50MB) | planned |
| CDN (jsDelivr/unpkg) | `locr.wasm` + ESM loader, zero-install browser use | planned |

Rule: **every channel ships the exact same core**, built once in CI, signed, reproducible.

## 3. Enterprise trust checklist (what Google/Microsoft reviewers actually check)

- [ ] **Dual license MIT OR Apache-2.0** — Apache-2.0 adds the patent grant big-co legal teams require. (Engine candidate `ocrs`/`rten` is Rust + permissively licensed — compatible.)
- [ ] **OpenSSF Scorecard** action in CI + badge in README.
- [ ] **OpenSSF Best Practices badge** (bestpractices.dev).
- [ ] **SLSA Level 3 provenance** — build on GitHub Actions with `slsa-github-generator`.
- [ ] **Sigstore/cosign signing** of every release artifact.
- [ ] **SBOM** (CycloneDX) attached to every release (`cargo-cyclonedx`).
- [ ] **Trusted Publishing** on PyPI + npm provenance (`npm publish --provenance`).
- [ ] **SemVer + frozen C ABI** — no breaking changes without major bump.
- [ ] **Zero telemetry, zero network calls** at runtime — verifiable by design (models bundled or one-time fetch, offline mode flag).
- [ ] `SECURITY.md`, `CODE_OF_CONDUCT.md`, `CONTRIBUTING.md`, signed-off commits.
- [ ] **Reproducible builds** (pinned toolchain via `rust-toolchain.toml`, locked deps).

## 4. Engine decision (pending, tracked)

Candidate: **`ocrs` + `rten`** — pure Rust, MIT, PyTorch-trained models exported to `.rten`
(~30MB, downloaded once to cache or bundled). Fits the locked decision: 100% Rust, native
ARM/x64, compiles to WASM. Latin alphabet today; multilingual on the roadmap (community-trainable
models = our moat vs closed OCR APIs).

## 5. The "standard" part

`locr` is not just a library — it's a spec. `SPEC.md` (planned) will define:
1. The C ABI surface (locr.h) — any conforming implementation can replace ours.
2. The result schema (text, confidence, boxes) as a versioned JSON schema.
3. Conformance test suite (golden images + expected outputs) runnable against any implementation.

That's how you become infrastructure instead of a product: **anyone can implement it, nobody can paywall it.**

## 6. Execution order

1. `locr-ffi` crate + `cbindgen` header + CI artifacts per target (this unlocks NuGet/Maven/Go/vcpkg all at once).
2. Wire `ocrs` into `locr-core` behind the `OcrEngine` trait; real OCR test in CI.
3. Fix remaining CI (Node lockfile ✅ exists, maturin config).
4. Supply-chain hardening (Scorecard, SLSA, cosign, SBOM) — cheap now, expensive later.
5. Fan out packaging channels in priority order: NuGet + Maven → Homebrew/winget → vcpkg/conda-forge → distros.
