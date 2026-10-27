// Run after `npm run build` (or `npm run build:wasm -- --no-bundle-models && npx tsup`
// offline: the tests then load locr-core's tiny synthetic models).
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { test } from 'node:test';
import { pathToFileURL } from 'node:url';
import { deflateSync } from 'node:zlib';

import imageToTextDefault, { hasBundledModels, imageToData, imageToText, loadModels } from '../dist/node.js';

const fixtures = new URL('../../../crates/locr-core/tests/fixtures/', import.meta.url);
const helloPath = new URL('hello.png', fixtures);

/** White RGBA page with rows of dark-grey glyphs (what the synthetic models detect). */
function page() {
  const width = 300;
  const height = 160;
  const data = new Uint8Array(width * height * 4).fill(255);
  for (let row = 0; row < 2; row++) {
    for (let g = 0; g < 6; g++) {
      const grey = 30 + ((row * 31 + g * 17) % 90);
      const x0 = 20 + g * 16 + (g > 2 ? 30 : 0);
      for (let y = 30 + row * 70; y < 52 + row * 70; y++) {
        for (let x = x0; x < x0 + 12; x++) data.set([grey, grey, grey, 255], (y * width + x) * 4);
      }
    }
  }
  return { data, width, height };
}

function crc32(bytes) {
  let crc = ~0;
  for (const b of bytes) {
    crc ^= b;
    for (let k = 0; k < 8; k++) crc = crc & 1 ? (crc >>> 1) ^ 0xedb88320 : crc >>> 1;
  }
  return ~crc >>> 0;
}

/** Minimal RGBA PNG encoder, to compare the pixel and encoded-image paths. */
function png({ data, width, height }) {
  const chunk = (type, body) => {
    const out = Buffer.alloc(12 + body.length);
    out.writeUInt32BE(body.length, 0);
    out.write(type, 4, 'latin1');
    body.copy(out, 8);
    out.writeUInt32BE(crc32(out.subarray(4, 8 + body.length)), 8 + body.length);
    return out;
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr.set([8, 6, 0, 0, 0], 8); // 8-bit RGBA
  const rows = Buffer.alloc((width * 4 + 1) * height);
  for (let y = 0; y < height; y++) rows.set(data.subarray(y * width * 4, (y + 1) * width * 4), y * (width * 4 + 1) + 1);
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(rows)),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

const models = async () => [
  await readFile(new URL('synthetic/text-detection.rten', fixtures)),
  await readFile(new URL('synthetic/text-recognition.rten', fixtures)),
];

test('models: bundled, or loaded at runtime', async () => {
  if (!(await hasBundledModels())) {
    await assert.rejects(imageToText(helloPath), /loadModels/);
    await loadModels(...(await models()));
  }
});

test('full result schema', async () => {
  const result = await imageToData(png(page()));
  assert.equal(result.version, 1);
  assert.deepEqual([result.width, result.height], [300, 160]);
  assert.equal(result.transform, 'none');
  assert.equal(result.attempts, 1);
  assert.ok(result.score >= 0 && result.score <= 1);
  assert.equal(result.text, result.lines.map((l) => l.text).join('\n'));
  for (const line of result.lines) {
    assert.deepEqual(Object.keys(line.bbox), ['x', 'y', 'width', 'height']);
    for (const word of line.words) assert.ok(word.confidence > 0 && word.confidence <= 1);
  }
});

test('every input type reads the same', async () => {
  const bytes = await readFile(helloPath);
  const expected = await imageToData(bytes);
  const copy = new Uint8Array(bytes);
  const inputs = {
    path: helloPath.pathname,
    fileUrl: pathToFileURL(helloPath.pathname),
    fileUrlString: pathToFileURL(helloPath.pathname).href,
    Uint8Array: copy,
    ArrayBuffer: copy.buffer.slice(copy.byteOffset, copy.byteOffset + copy.byteLength),
    DataView: new DataView(copy.buffer, copy.byteOffset, copy.byteLength),
    Blob: new Blob([bytes]),
  };
  for (const [name, input] of Object.entries(inputs)) {
    assert.deepEqual(await imageToData(input), expected, name);
  }
  assert.equal(await imageToText(bytes), expected.text);
  assert.equal(await imageToTextDefault(bytes), expected.text);
});

test('raw pixels read like the encoded image', async () => {
  const pixels = page();
  const fromPng = await imageToData(png(pixels));
  assert.deepEqual(await imageToData(pixels), fromPng);
  // ImageData-like objects carry a Uint8ClampedArray.
  const clamped = { ...pixels, data: new Uint8ClampedArray(pixels.data.buffer) };
  assert.equal(await imageToText(clamped), fromPng.text);
});

test('options reach the engine', async () => {
  const input = png(page());
  const digits = await imageToData(input, { allowedChars: '0123456789' });
  assert.match(digits.text, /^[\d\s]*$/);
  const enhanced = await imageToData(input, { autoEnhance: true, minScore: 2, maxAttempts: 2 });
  assert.equal(enhanced.attempts, 2);
  // hello.png has a line for both the real and the synthetic models.
  assert.ok((await imageToData(helloPath, { beamWidth: 4 })).lines.length > 0);
  await assert.rejects(imageToData(input, { alowedChars: '1' }), (err) => err instanceof TypeError && /alowedChars/.test(err.message));
  await assert.rejects(imageToData(input, { beamWidth: 'wide' }), /beam_width/);
  await assert.rejects(imageToData(input, { beamWidth: 1000 }), /at most 64/);
});

test('errors', async () => {
  await assert.rejects(imageToText(new Uint8Array([1, 2, 3])), (err) => err instanceof Error && /decode/.test(err.message));
  await assert.rejects(imageToText(42), TypeError);
  await assert.rejects(imageToText('/nonexistent/locr.png'), /ENOENT/);
  await assert.rejects(imageToData({ data: new Uint8Array(5), width: 2, height: 2 }), /invalid input/);
});

test('concurrent calls are consistent', async () => {
  const input = png(page());
  const results = await Promise.all([1, 2, 3, 4].map(() => imageToData(input)));
  for (const r of results) assert.deepEqual(r, results[0]);
});

test('CommonJS build', async () => {
  const require = createRequire(import.meta.url);
  const cjs = require('../dist/node.cjs');
  if (!(await cjs.hasBundledModels())) await cjs.loadModels(...(await models()));
  assert.equal(await cjs.imageToText(helloPath.pathname), await imageToText(helloPath.pathname));
});
