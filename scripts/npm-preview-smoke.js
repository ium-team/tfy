#!/usr/bin/env node
'use strict';

const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');

const root = path.resolve(__dirname, '..');
const pkgDir = path.join(root, 'npm', 'tfy-cli');
const smokeRoot = path.join(root, '.tfy', 'npm-smoke');
const prefix = path.join(smokeRoot, 'prefix');
const outside = path.join(smokeRoot, 'outside-project');
const releaseBin = path.join(root, 'target', 'release', process.platform === 'win32' ? 'tfy.exe' : 'tfy');

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { stdio: 'inherit', shell: process.platform === 'win32', ...options });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(' ')} failed with status ${result.status}`);
  }
}

function runCapture(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: 'utf8', shell: process.platform === 'win32', ...options });
  if (result.status !== 0) {
    process.stderr.write(result.stderr || '');
    process.stdout.write(result.stdout || '');
    throw new Error(`${command} ${args.join(' ')} failed with status ${result.status}`);
  }
  return result.stdout;
}

function rmrf(p) {
  fs.rmSync(p, { recursive: true, force: true });
}

function main() {
  run('cargo', ['build', '--release', '-p', 'tfy-cli'], { cwd: root });
  rmrf(smokeRoot);
  fs.mkdirSync(prefix, { recursive: true });
  fs.mkdirSync(outside, { recursive: true });
  run('npm', ['test'], { cwd: pkgDir });
  run('npm', ['pack', '--pack-destination', smokeRoot], { cwd: pkgDir });
  const tarball = fs.readdirSync(smokeRoot).filter(name => name.endsWith('.tgz')).map(name => path.join(smokeRoot, name))[0];
  if (!tarball) throw new Error('npm pack did not create a tarball');
  run('npm', ['install', '-g', '--prefix', prefix, tarball], {
    cwd: root,
    env: { ...process.env, TFY_BINARY_PATH: releaseBin }
  });
  const tfy = process.platform === 'win32' ? path.join(prefix, 'tfy.cmd') : path.join(prefix, 'bin', 'tfy');
  runCapture(tfy, ['--help'], { cwd: outside });
  runCapture(tfy, ['status', '--human'], { cwd: outside });
  process.stdout.write(`TFY npm preview smoke: pass\npackage=${tarball}\nprefix=${prefix}\nbinary=${releaseBin}\ncommand=${tfy}\n`);
}

try {
  main();
} catch (error) {
  process.stderr.write(`TFY npm preview smoke: fail: ${error.message}\n`);
  process.exit(1);
}
