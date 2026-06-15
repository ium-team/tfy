'use strict';

const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { resolveLauncherBinary } = require('../bin/tfy');

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'tfy-launcher-test-'));
const fake = path.join(tmp, process.platform === 'win32' ? 'tfy.cmd' : 'tfy');
if (process.platform === 'win32') {
  fs.writeFileSync(fake, '@echo off\necho fake-tfy %*\n');
} else {
  fs.writeFileSync(fake, '#!/usr/bin/env sh\necho fake-tfy "$@"\n');
  fs.chmodSync(fake, 0o755);
}
const result = spawnSync(process.execPath, [path.join(__dirname, '..', 'bin', 'tfy.js'), 'hello'], {
  env: { ...process.env, TFY_BINARY_PATH: fake },
  encoding: 'utf8'
});
assert.strictEqual(result.status, 0, result.stderr);
assert.match(result.stdout, /fake-tfy hello/);
assert.strictEqual(resolveLauncherBinary({ env: { TFY_BINARY_PATH: fake }, platform: 'darwin', arch: 'x64' }).binary, fake);
assert.throws(() => resolveLauncherBinary({ env: {}, platform: 'darwin', arch: 'x64' }), /Intel Mac prebuilt npm installs are not currently provided/);

const missing = spawnSync(process.execPath, [path.join(__dirname, '..', 'bin', 'tfy.js')], {
  env: { ...process.env, TFY_BINARY_PATH: path.join(tmp, 'missing-tfy') },
  encoding: 'utf8'
});
assert.strictEqual(missing.status, 127);
assert.match(missing.stderr, /missing TFY binary/);
