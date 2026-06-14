'use strict';

const assert = require('assert');
const { resolvePlatform, archiveName } = require('../scripts/lib/platform');

assert.strictEqual(resolvePlatform('darwin', 'arm64').rustTarget, 'aarch64-apple-darwin');
assert.strictEqual(archiveName('v0.1.0', resolvePlatform('darwin', 'x64')), 'tfy-0.1.0-darwin-x86_64.tar.gz');
assert.strictEqual(resolvePlatform('linux', 'x64').binName, 'tfy');
assert.strictEqual(resolvePlatform('win32', 'x64').binName, 'tfy.exe');
assert.strictEqual(archiveName('0.1.0-preview.0', resolvePlatform('linux', 'x64')), 'tfy-0.1.0-linux-x86_64.tar.gz');
assert.throws(() => resolvePlatform('sunos', 'x64'), /Unsupported TFY npm platform/);
