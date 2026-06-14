#!/usr/bin/env node
'use strict';

const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');
const { currentPlatform } = require('../scripts/lib/platform');

const root = path.resolve(__dirname, '..');
const target = currentPlatform();
const binary = process.env.TFY_BINARY_PATH || path.join(root, 'vendor', target.rustTarget, target.binName);

if (!fs.existsSync(binary)) {
  process.stderr.write(`[tfy] missing TFY binary for ${target.key}: ${binary}\n`);
  process.stderr.write('[tfy] reinstall the package or set TFY_BINARY_PATH to a built tfy binary.\n');
  process.exit(127);
}

const result = spawnSync(binary, process.argv.slice(2), {
  stdio: 'inherit',
  shell: process.platform === 'win32' && /\.(cmd|bat)$/i.test(binary)
});
if (result.error) {
  process.stderr.write(`[tfy] failed to execute ${binary}: ${result.error.message}\n`);
  process.exit(127);
}
if (result.signal) {
  process.kill(process.pid, result.signal);
} else {
  process.exit(result.status === null ? 1 : result.status);
}
