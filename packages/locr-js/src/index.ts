/**
 * locr JS entry point.
 *
 * STATUS (v0.1): this package currently uses tesseract.js as a temporary
 * local engine so `npm install locr` works today. The pure-Rust core
 * (`locr-wasm`) is the target path and is already building in CI.
 * Migration plan: ship wasm-pack artifacts and switch this wrapper over.
 *
 * Until then: runs fully on-device, but downloads tesseract worker/traineddata
 * from a CDN on first use (same as stock tesseract.js).
 */

import { createWorker, type ImageLike } from 'tesseract.js';

export interface ImageToTextOptions {
  /** ISO language code (default: 'eng'). */
  language?: string;
  /** Optional progress callback. */
  logger?: (m: any) => void;
}

/**
 * Extract text from an image, locally.
 *
 * @param image Path, URL, Buffer, ArrayBuffer, or HTML image element.
 * @returns Recognized text.
 *
 * @example
 * ```ts
 * import { imageToText } from 'locr';
 * const text = await imageToText('invoice.png');
 * ```
 */
export async function imageToText(
  image: ImageLike,
  options: ImageToTextOptions = {}
): Promise<string> {
  const worker = await createWorker(
    options.language ?? 'eng',
    1,
    options.logger ? { logger: options.logger } : undefined,
  );
  try {
    const ret = await worker.recognize(image);
    return ret.data.text.trim();
  } finally {
    await worker.terminate();
  }
}

export default imageToText;
