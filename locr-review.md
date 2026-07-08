# 🔍 Code Review: locr — post-fix status (2026-07-08)

## Veredicto
Core Rust + wrappers dejados listos para pitch (Microsoft / enterprise).
Los hallazgos críticos de la review original están cerrados o declarados
honestamente en el README.

## ✅ Fixed this pass

| # | Issue | Fix |
|---|-------|-----|
| 1 | npm = tesseract.js silencioso | Documentado en README + header del wrapper JS; path WASM marcado como next |
| 2 | pip hard-deps pytesseract/Pillow | `dependencies = []`; extras `locr[fallback]` |
| 3 | `Image.open(bytes)` TypeError | `io.BytesIO(image)` en fallback |
| 4 | engine rebuild cada call | `locr_core::shared()` con `OnceLock` en FFI / WASM / PyO3 |
| 5 | models sin checksum | SHA-256 pinneado en `build.rs` (sha2) |
| 6 | FFI `ModelsNotFound` → EngineError | mapeo correcto a `EngineNotFound` (-4) |
| 7 | panic en `Default for OcrsEngine` | removido; solo `new()` / `try_new()` |
| 8 | `npm test` roto | script no apunta a archivo fantasma |
| 9 | README mentía "No Tesseract" | tabla de status honesta + current limits |

## 🔜 Next (no bloquean pitch técnico)

1. Wrapper JS real sobre `locr-wasm` (wasm-pack + npm package files)
2. `locr_image_to_text_opts()` + multi-language antes de ABI 1.0
3. Tag `v0.1.0` + release artifacts
4. Smoke test con fixture de texto conocido en CI
5. Dual license MIT OR Apache-2.0 (enterprise legal)

## Prioridad original → estado

1. Bug #3 + OnceLock #4 → **DONE**
2. Honestidad README #1/#2 → **DONE**
3. Checksums #5 → **DONE**
4. JS→WASM real → next war
