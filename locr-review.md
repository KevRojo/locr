# 🔍 Code Review: locr — estado (2026-10-27)

## Veredicto
Las cinco superficies (Rust, C ABI, CLI, npm, pip) corren ahora el **mismo core
Rust** y devuelven el **mismo esquema JSON** (texto, líneas, palabras, cajas,
confianza). npm ya no usa tesseract.js ni descarga nada en runtime.

## ✅ Esta pasada (2026-10-27)

| # | Issue | Fix |
|---|-------|-----|
| 1 | Solo texto plano; sin cajas ni confianza | `image_to_data` / `locr_image_to_json` / `imageToData`: líneas → palabras con bbox y confianza real (probabilidades del modelo; ocrs las descartaba) |
| 2 | PNG con fondo transparente → negro sobre negro, OCR vacío | alpha aplanado sobre fondo que contrasta con la tinta |
| 3 | Fotos de celular rotadas (EXIF) se leían de lado | orientación EXIF aplicada al decodificar |
| 4 | npm = tesseract.js + CDN | paquete npm sobre `locr-wasm` (Node ESM/CJS + navegador), sin red en runtime |
| 5 | Panic en FFI abortaba el proceso host | `catch_unwind` en todo el C ABI → `LOCR_INTERNAL` + `locr_last_error()` |
| 6 | Leak de `*out_text` en `locr_image_to_text_ex` si fallaba tarde | salidas convertidas antes de escribir |
| 7 | Python mantenía el GIL durante el OCR | GIL liberado (`py.detach`); entradas PIL/numpy/archivos; wheels abi3; pyo3 0.30 (0.23 tenía RUSTSEC-2025-0020) |
| 8 | `shared()` mapeaba errores por `msg.contains("models")` | `LocrError: Clone`, sin string matching |
| 9 | El reconocedor se cargaba dos veces (ocrs + locr) | locr prepara las imágenes de línea; ocrs solo carga el detector |
| 10 | ocrs 0.12 filtraba su propia API JS en locr-wasm | ocrs 0.13 sin `export-wasm`; `image` sin encoders AVIF/EXR (rav1e) |
| 11 | Sin CLI | `locr` (texto o JSON Lines) |
| 12 | Sin whitelist ni beam search | `allowed_chars` / `beam_width` en todas las superficies |
| 13 | Caché de modelos en CI nunca se llenaba (`build.rs` baja a `OUT_DIR`) | action `ocr-models` + `assets/` (SHA-256 verificado igual) |
| 14 | Release: `npm ci` sin lockfile, crates sin `version` en deps path, sin binarios | `npm install`, versiones, binarios C ABI + CLI por plataforma en el GitHub Release |
| 15 | Wheels/npm redistribuían modelos CC-BY-SA sin `NOTICE-MODELS` | notice incluido en ambos paquetes |

Tests offline con **modelos sintéticos** (`crates/locr-core/tests/fixtures/synthetic`):
el pipeline completo de ocrs corre sin los modelos reales, y el reconocimiento
de locr se compara contra el de ocrs (texto y cajas idénticos).

## 🚑 Hotfix CI (2026-10-27)

El bucket de ocrs ya no sirve los `.rten` (ocrs 0.13 publica solo `.onnx`):
todo build con modelos reales daba **404**, también con el `build.rs` viejo.
locr ahora embebe `text-detection.onnx` / `text-recognition.onnx` (rten con
`onnx_format`), y los SHA-256 viven en `crates/locr-core/models.sha256`, que
leen `build.rs` y la action `ocr-models`. Si un digest falta o no coincide, CI
falla con una anotación que muestra el digest descargado, para revisarlo y
pinnearlo.

## ⚠️ Decisiones pendientes (del dueño)

1. **Nombre en PyPI**: `locr` ya está tomado por otro proyecto (Library of
   Congress, desde 2021). `pip install locr` instala ese paquete y el publish
   del release va a fallar. Opciones: `locr-ocr`, `pylocr`... (el import puede
   seguir siendo `import locr`).
2. Dual license MIT OR Apache-2.0 (enterprise legal).

## 🔜 Next

1. Multi-language: modelos de reconocimiento + alfabetos (la API ya acepta `with_alphabet` / modelos en runtime)
2. Detección por tiles para páginas grandes con texto pequeño
3. Worker pool en JS (hoy el OCR corre en el hilo que llama)
4. Entrada edge (Cloudflare Workers importa `.wasm` como módulo)
5. Integraciones (Office / Google / VS Code) sobre el paquete npm real
6. JSON Schema formal + suite de conformidad
7. Tag `v0.1.0`

## Historial (2026-07-08)

Bug `Image.open(bytes)`, `OnceLock` compartido, SHA-256 de modelos, mapeo
`ModelsNotFound` → `EngineNotFound`, README honesto: **DONE**.
