/**
 * Platform-independent locr API on top of the Rust/WASM engine.
 *
 * The engine is the same pure-Rust core as the C ABI, CLI and Python
 * package: images never leave the process, and nothing is downloaded at
 * runtime.
 */

import initWasm, * as wasm from './wasm/locr.js';
import type { InitInput } from './wasm/locr.js';

/** Recognition options. Every field is optional. */
export interface OcrOptions {
  /** Only emit these characters, e.g. `'0123456789'`. Spaces are always allowed. */
  allowedChars?: string;
  /** CTC beam search width. `0`/`1` = greedy decoding (fastest, default). */
  beamWidth?: number;
  /** Retry with contrast/brightness/saturation/... transforms when the score is low. */
  autoEnhance?: boolean;
  /** Score below which `autoEnhance` retries. Default `0.55`. */
  minScore?: number;
  /** OCR passes for `autoEnhance`, including the first. Default `7`. */
  maxAttempts?: number;
}

/** Pixel box in the image as displayed (EXIF orientation applied). */
export interface BoundingBox {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Word {
  text: string;
  /** Mean recognition probability of the characters, `0..1`. */
  confidence: number | null;
  bbox: BoundingBox;
}

export interface Line {
  text: string;
  confidence: number | null;
  bbox: BoundingBox;
  words: Word[];
}

/** Full OCR result (locr JSON schema version 1). */
export interface OcrResult {
  version: number;
  text: string;
  /** Quality score `0..1`: model confidence blended with a text heuristic. */
  score: number;
  /** Mean recognition probability over all characters, `0..1`. */
  confidence: number | null;
  /** Image transform that produced the result (`'none'` without enhancement). */
  transform: string;
  attempts: number;
  width: number;
  height: number;
  lines: Line[];
}

/** Raw 8-bit pixels (grey, RGB or RGBA), e.g. a canvas `ImageData`. */
export interface PixelData {
  data: Uint8Array | Uint8ClampedArray;
  width: number;
  height: number;
}

/**
 * Anything locr can read:
 * - encoded image bytes (PNG, JPEG, WEBP, BMP, TIFF, GIF...): `Uint8Array`
 *   (incl. Node `Buffer`), `ArrayBuffer`, any `ArrayBufferView`, `Blob`/`File`
 * - a path or `file:` URL (Node), or a URL to fetch (browser)
 * - raw pixels (`ImageData` and friends)
 * - in browsers, also `<img>`, `<canvas>`, `OffscreenCanvas`, `ImageBitmap`, `<video>`
 */
export type ImageInput = string | URL | ArrayBuffer | ArrayBufferView | Blob | PixelData | object;

/** What differs between Node and browsers. */
export interface Platform {
  /** The compiled `.wasm`: bytes, a `Response`, or a URL to fetch. */
  wasm(): Promise<InitInput>;
  /** Read an image given by path or URL. */
  read(source: string | URL): Promise<Uint8Array>;
  /** Pixels of a platform image object (canvas, `<img>`...), if supported. */
  pixels?(image: object): Promise<PixelData | undefined>;
}

type Prepared = { bytes: Uint8Array } | { pixels: PixelData };

const OPTION_KEYS: Record<keyof OcrOptions, string> = {
  allowedChars: 'allowed_chars',
  beamWidth: 'beam_width',
  autoEnhance: 'auto_enhance',
  minScore: 'min_score',
  maxAttempts: 'max_attempts',
};

function optionsJson(options: OcrOptions = {}): string {
  const json: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(options)) {
    const name = OPTION_KEYS[key as keyof OcrOptions];
    if (!name) {
      throw new TypeError(`unknown locr option "${key}" (expected one of ${Object.keys(OPTION_KEYS).join(', ')})`);
    }
    if (value !== undefined) json[name] = value;
  }
  return JSON.stringify(json);
}

function isPixelData(image: object): image is PixelData {
  const { data, width, height } = image as Partial<PixelData>;
  return (data instanceof Uint8Array || data instanceof Uint8ClampedArray) && typeof width === 'number' && typeof height === 'number';
}

export function createLocr(platform: Platform) {
  let ready: Promise<void> | undefined;

  /** Load and instantiate the engine. Called automatically; await it to warm up. */
  function init(): Promise<void> {
    ready ??= platform
      .wasm()
      .then((module_or_path) => initWasm({ module_or_path }))
      .then(
        () => undefined,
        (err) => {
          ready = undefined;
          throw err;
        },
      );
    return ready;
  }

  async function prepare(image: ImageInput): Promise<Prepared> {
    if (typeof image === 'string' || image instanceof URL) return { bytes: await platform.read(image) };
    if (image instanceof Uint8Array) return { bytes: image };
    if (image instanceof ArrayBuffer) return { bytes: new Uint8Array(image) };
    if (ArrayBuffer.isView(image)) return { bytes: new Uint8Array(image.buffer, image.byteOffset, image.byteLength) };
    if (typeof Blob !== 'undefined' && image instanceof Blob) return { bytes: new Uint8Array(await image.arrayBuffer()) };
    if (image !== null && typeof image === 'object') {
      if (isPixelData(image)) return { pixels: image };
      const pixels = await platform.pixels?.(image);
      if (pixels) return { pixels };
    }
    throw new TypeError(`locr cannot read images of type ${Object.prototype.toString.call(image)}`);
  }

  function run(input: Prepared, options: string): OcrResult {
    if ('bytes' in input) return JSON.parse(wasm.imageToDataJson(input.bytes, options));
    const { data, width, height } = input.pixels;
    const pixels = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
    return JSON.parse(wasm.pixelsToDataJson(width, height, pixels, options));
  }

  /**
   * Full OCR result: text, lines, words, bounding boxes and confidence.
   *
   * @example
   * ```ts
   * const { lines } = await imageToData('invoice.png', { allowedChars: '0123456789.,' });
   * for (const line of lines) console.log(line.text, line.bbox, line.confidence);
   * ```
   */
  async function imageToData(image: ImageInput, options?: OcrOptions): Promise<OcrResult> {
    const json = optionsJson(options);
    const [input] = await Promise.all([prepare(image), init()]);
    return run(input, json);
  }

  /**
   * Extract text from an image, locally.
   *
   * @example
   * ```ts
   * import { imageToText } from 'locr';
   * const text = await imageToText('invoice.png');
   * ```
   */
  async function imageToText(image: ImageInput, options?: OcrOptions): Promise<string> {
    if (options === undefined) {
      const [input] = await Promise.all([prepare(image), init()]);
      if ('bytes' in input) return wasm.imageToText(input.bytes);
      return run(input, '{}').text;
    }
    return (await imageToData(image, options)).text;
  }

  /**
   * Use these `.onnx`/`.rten` models instead of the bundled ones (custom models, or a
   * build without embedded models that fetches and caches them itself).
   */
  async function loadModels(detection: ArrayBuffer | ArrayBufferView, recognition: ArrayBuffer | ArrayBufferView): Promise<void> {
    const bytes = (b: ArrayBuffer | ArrayBufferView) =>
      ArrayBuffer.isView(b) ? new Uint8Array(b.buffer, b.byteOffset, b.byteLength) : new Uint8Array(b);
    await init();
    wasm.loadModels(bytes(detection), bytes(recognition));
  }

  /** Whether this build embeds the OCR models (if not, call `loadModels` first). */
  async function hasBundledModels(): Promise<boolean> {
    await init();
    return wasm.hasBundledModels();
  }

  return { init, imageToText, imageToData, loadModels, hasBundledModels };
}
