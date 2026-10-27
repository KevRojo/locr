/**
 * locr for Node.js (also Bun and Deno): the engine is loaded from the
 * `.wasm` file shipped in this package; nothing is fetched over the network.
 */

import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

import { createLocr } from './core.js';

export type { BoundingBox, ImageInput, Line, OcrOptions, OcrResult, PixelData, Word } from './core.js';

const locr = createLocr({
  wasm: () => readFile(fileURLToPath(new URL('./locr_bg.wasm', import.meta.url))),
  read: async (source) => new Uint8Array(await readFile(source)),
});

export const { init, imageToText, imageToData, loadModels, hasBundledModels } = locr;
export default imageToText;
