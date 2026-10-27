// Build crates/locr-wasm and generate its JS glue in src/wasm/.
//
//   node scripts/build-wasm.mjs                     # models embedded (default)
//   node scripts/build-wasm.mjs --no-bundle-models  # small .wasm; call loadModels()
//
// Needs the wasm32-unknown-unknown target and the wasm-bindgen CLI pinned in
// crates/locr-wasm/Cargo.toml:
//   rustup target add wasm32-unknown-unknown
//   cargo install wasm-bindgen-cli --version <same as Cargo.toml> --locked
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const pkgDir = join(dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = join(pkgDir, '..', '..');
const bundleModels = !process.argv.includes('--no-bundle-models');

const run = (cmd, args) => execFileSync(cmd, args, { cwd: repoRoot, stdio: 'inherit' });

const wantBindgen = readFileSync(join(repoRoot, 'crates/locr-wasm/Cargo.toml'), 'utf8').match(/wasm-bindgen = "=([^"]+)"/)[1];
const haveBindgen = execFileSync('wasm-bindgen', ['--version'], { encoding: 'utf8' }).trim().split(' ')[1];
if (haveBindgen !== wantBindgen) {
  throw new Error(`wasm-bindgen CLI ${haveBindgen} does not match the crate (${wantBindgen}): cargo install wasm-bindgen-cli --version ${wantBindgen} --locked`);
}

run('cargo', [
  'build', '-p', 'locr-wasm', '--release', '--target', 'wasm32-unknown-unknown',
  ...(bundleModels ? [] : ['--no-default-features']),
]);

const { target_directory: targetDir } = JSON.parse(
  execFileSync('cargo', ['metadata', '--format-version', '1', '--no-deps'], { cwd: repoRoot, encoding: 'utf8' }),
);
run('wasm-bindgen', [
  '--target', 'web',
  '--out-dir', join(pkgDir, 'src', 'wasm'),
  '--out-name', 'locr',
  join(targetDir, 'wasm32-unknown-unknown', 'release', 'locr_wasm.wasm'),
]);
