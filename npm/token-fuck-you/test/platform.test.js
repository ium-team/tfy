'use strict';

const assert = require('assert');
const { resolvePlatform, resolveRustTarget, archiveName, baseVersion, cleanVersion, supportedTargets } = require('../scripts/lib/platform');

assert.strictEqual(resolvePlatform('darwin', 'arm64').rustTarget, 'aarch64-apple-darwin');
assert.strictEqual(archiveName('v0.1.0', resolvePlatform('darwin', 'x64')), 'tfy-0.1.0-darwin-x86_64.tar.gz');
assert.strictEqual(resolvePlatform('linux', 'x64').binName, 'tfy');
assert.strictEqual(resolvePlatform('win32', 'x64').binName, 'tfy.exe');
assert.strictEqual(archiveName('0.1.0-preview.0', resolvePlatform('linux', 'x64')), 'tfy-0.1.0-linux-x86_64.tar.gz');
assert.strictEqual(cleanVersion('v0.1.0-preview.0'), '0.1.0-preview.0');
assert.strictEqual(baseVersion('v0.1.0-preview.0'), '0.1.0');
assert.deepStrictEqual(supportedTargets().map(target => target.rustTarget).sort(), [
  'aarch64-apple-darwin',
  'aarch64-unknown-linux-gnu',
  'x86_64-apple-darwin',
  'x86_64-pc-windows-msvc',
  'x86_64-unknown-linux-gnu'
]);
assert.strictEqual(resolveRustTarget('x86_64-pc-windows-msvc').binName, 'tfy.exe');
assert.throws(() => resolvePlatform('sunos', 'x64'), /Unsupported TFY npm platform/);
assert.throws(() => resolveRustTarget('wasm32-wasip1'), /Unsupported TFY Rust target/);
