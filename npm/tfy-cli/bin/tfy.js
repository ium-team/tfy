#!/usr/bin/env node
'use strict';

const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');
const { currentPlatform } = require('../scripts/lib/platform');

const root = path.resolve(__dirname, '..');

function resolveLauncherBinary(options = {}) {
  const env = options.env || process.env;
  if (env.TFY_BINARY_PATH) return { binary: env.TFY_BINARY_PATH, target: null };
  const target = currentPlatform({ platform: options.platform, arch: options.arch });
  return {
    binary: path.join(root, 'vendor', target.rustTarget, target.binName),
    target
  };
}

function main(argv = process.argv.slice(2), options = {}) {
  const env = options.env || process.env;
  let resolved;
  try {
    resolved = resolveLauncherBinary({ env, platform: options.platform, arch: options.arch });
  } catch (error) {
    process.stderr.write(`[tfy] ${error.message}\n`);
    process.stderr.write('[tfy] set TFY_BINARY_PATH to a built tfy binary if you want to run from source on this platform.\n');
    process.exit(127);
  }

  const { binary, target } = resolved;
  if (!fs.existsSync(binary)) {
    const platformLabel = target ? target.key : 'TFY_BINARY_PATH';
    process.stderr.write(`[tfy] missing TFY binary for ${platformLabel}: ${binary}\n`);
    process.stderr.write('[tfy] reinstall the package or set TFY_BINARY_PATH to a built tfy binary.\n');
    process.exit(127);
  }

  const result = spawnSync(binary, argv, {
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
}

if (require.main === module) main();

module.exports = {
  main,
  resolveLauncherBinary
};
