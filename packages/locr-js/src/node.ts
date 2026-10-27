/**
 * locr for Node.js (also Bun and Deno): the engine is loaded from the
 * `.wasm` file shipped in this package; nothing is fetched over the network.
 */

import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

import { createLocr } from './core.js';

export type { BoundingBox, ImageInput, Line, OcrOptions, OcrResult, PixelData, Word } from './core.js';

const locr = createLocr({
  // `__dirname` (shimmed by tsup in the ESM build) survives install paths
  // with `#` or `%`, which break the CommonJS `import.meta.url` shim.
  wasm: () => readFile(join(__dirname, 'locr_bg.wasm')),
  read: async (source) => {
    const target = typeof source === 'string' && source.startsWith('file:') ? new URL(source) : source;
    return new Uint8Array(await readFile(target));
  },
});

export const { init, imageToText, imageToData, loadModels, hasBundledModels } = locr;
export default imageToText;
