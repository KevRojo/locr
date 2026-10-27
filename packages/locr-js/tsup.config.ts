import { copyFile } from 'node:fs/promises';
import { defineConfig } from 'tsup';

export default defineConfig([
  {
    entry: { node: 'src/node.ts' },
    format: ['esm', 'cjs'],
    platform: 'node',
    target: 'node18',
    dts: true,
    // `__dirname` in the ESM build (locates the .wasm).
    shims: true,
    async onSuccess() {
      await copyFile('src/wasm/locr_bg.wasm', 'dist/locr_bg.wasm');
    },
  },
  {
    entry: { browser: 'src/browser.ts' },
    format: ['esm'],
    platform: 'browser',
    target: 'es2022',
    dts: true,
  },
]);
