/**
 * locr for browsers. Bundlers (Vite, webpack, Rollup, esbuild) pick up the
 * `.wasm` through `new URL(..., import.meta.url)`; OCR runs in the page (or
 * in your Web Worker), and images are never uploaded.
 */

import { createLocr, type PixelData } from './core.js';

export type { BoundingBox, ImageInput, Line, OcrOptions, OcrResult, PixelData, Word } from './core.js';

const isInstance = (value: object, name: string): boolean => {
  const ctor = (globalThis as Record<string, unknown>)[name];
  return typeof ctor === 'function' && value instanceof ctor;
};

/** Pixels of `<img>`, `<canvas>`, `OffscreenCanvas`, `ImageBitmap` or `<video>`. */
async function pixels(image: object): Promise<PixelData | undefined> {
  const kinds = ['HTMLImageElement', 'HTMLCanvasElement', 'OffscreenCanvas', 'ImageBitmap', 'HTMLVideoElement', 'VideoFrame'];
  if (!kinds.some((kind) => isInstance(image, kind))) return undefined;
  if (isInstance(image, 'HTMLImageElement')) await (image as HTMLImageElement).decode();

  const source = image as CanvasImageSource & Partial<Record<'width' | 'height' | 'naturalWidth' | 'naturalHeight' | 'videoWidth' | 'videoHeight' | 'displayWidth' | 'displayHeight', number>>;
  const width = source.naturalWidth || source.videoWidth || source.displayWidth || source.width || 0;
  const height = source.naturalHeight || source.videoHeight || source.displayHeight || source.height || 0;
  const canvas = typeof OffscreenCanvas === 'function'
    ? new OffscreenCanvas(width, height)
    : Object.assign(document.createElement('canvas'), { width, height });
  const ctx = canvas.getContext('2d') as CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D | null;
  if (!ctx) throw new Error('locr: 2D canvas is not available');
  ctx.drawImage(source, 0, 0);
  return ctx.getImageData(0, 0, width, height);
}

const locr = createLocr({
  wasm: async () => new URL('./locr_bg.wasm', import.meta.url),
  read: async (source) => {
    const response = await fetch(source);
    if (!response.ok) throw new Error(`locr: GET ${source} failed with ${response.status}`);
    return new Uint8Array(await response.arrayBuffer());
  },
  pixels,
});

export const { init, imageToText, imageToData, loadModels, hasBundledModels } = locr;
export default imageToText;
