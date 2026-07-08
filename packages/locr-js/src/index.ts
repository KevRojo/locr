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
  const worker = await createWorker(options.language ?? 'eng', 1, {
    logger: options.logger,
  });
  const ret = await worker.recognize(image);
  await worker.terminate();
  return ret.data.text.trim();
}

export default imageToText;
