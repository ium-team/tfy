#!/usr/bin/env node
'use strict';

const crypto = require('crypto');
const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');
const { archiveName, baseVersion, cleanVersion, resolveRustTarget } = require('../npm/token-fuck-you/scripts/lib/platform');

const RELEASE_VERSION_RE = /^\d+\.\d+\.\d+(?:-preview\.\d+)?$/;

function parseArgs(argv) {
  const args = { dist: 'dist' };
  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (!token.startsWith('--')) throw new Error(`unexpected argument: ${token}`);
    const key = token.slice(2).replace(/-([a-z])/g, (_, ch) => ch.toUpperCase());
    const value = argv[i + 1];
    if (!value || value.startsWith('--')) throw new Error(`missing value for ${token}`);
    args[key] = value;
    i += 1;
  }
  return args;
}

function sha256(file) {
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

function defaultBinaryPath(root, target) {
  return path.join(root, 'target', target.rustTarget, 'release', target.binName);
}

function toTarPath(value, pathLib = path) {
  return String(value || '.').split(pathLib.sep).join('/');
}

function buildTarInvocation({ binary, dist, archivePath, target, pathLib = path }) {
  const binaryDir = pathLib.dirname(binary);
  const relativeSourceDir = pathLib.relative(dist, binaryDir) || '.';
  if (pathLib.isAbsolute(relativeSourceDir) || /^[A-Za-z]:/.test(relativeSourceDir)) {
    throw new Error(`tar source directory must be relative to dist; got ${relativeSourceDir}`);
  }
  const sourceDir = toTarPath(relativeSourceDir, pathLib);
  return {
    command: 'tar',
    args: ['-C', sourceDir, '-czf', pathLib.basename(archivePath), target.binName],
    options: { cwd: dist, encoding: 'utf8' }
  };
}

function packageRelease(options) {
  const root = options.root || process.cwd();
  if (options.version == null || String(options.version).trim() === '') throw new Error('version is required');
  const version = cleanVersion(options.version);
  if (!RELEASE_VERSION_RE.test(version)) throw new Error(`version must match N.N.N or N.N.N-preview.N: ${version}`);
  const target = options.target || resolveRustTarget(options.rustTarget);
  const binary = path.resolve(root, options.binary || defaultBinaryPath(root, target));
  const dist = path.resolve(root, options.dist || 'dist');
  const archive = archiveName(version, target);
  const archivePath = path.join(dist, archive);
  const checksumPath = `${archivePath}.sha256`;
  const manifestPath = options.manifest ? path.resolve(root, options.manifest) : null;

  if (!fs.existsSync(binary)) throw new Error(`release binary does not exist: ${binary}`);
  const stat = fs.lstatSync(binary);
  if (!stat.isFile() || stat.isSymbolicLink()) throw new Error(`release binary is not a regular file: ${binary}`);

  fs.mkdirSync(dist, { recursive: true });
  fs.rmSync(archivePath, { force: true });
  fs.rmSync(checksumPath, { force: true });
  const tarInvocation = buildTarInvocation({ binary, dist, archivePath, target });
  const tar = spawnSync(tarInvocation.command, tarInvocation.args, tarInvocation.options);
  if (tar.status !== 0) throw new Error(`tar failed for ${target.rustTarget}: ${(tar.stderr || tar.stdout || '').trim()}`);

  const hash = sha256(archivePath);
  fs.writeFileSync(checksumPath, `${hash}  ${archive}\n`);

  const manifest = {
    schema_version: 1,
    version,
    base_version: baseVersion(version),
    rust_target: target.rustTarget,
    platform: target.archivePlatform,
    arch: target.archiveArch,
    binary_name: target.binName,
    archive_name: archive,
    archive_path: archivePath,
    checksum_name: `${archive}.sha256`,
    checksum_path: checksumPath,
    sha256: hash
  };
  if (manifestPath) {
    fs.mkdirSync(path.dirname(manifestPath), { recursive: true });
    fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  }
  return manifest;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const manifest = packageRelease({
    version: args.version,
    rustTarget: args.rustTarget,
    binary: args.binary,
    dist: args.dist,
    manifest: args.manifest,
    root: args.root ? path.resolve(args.root) : process.cwd()
  });
  process.stdout.write(`${JSON.stringify(manifest, null, 2)}\n`);
}

if (require.main === module) {
  try {
    main();
  } catch (error) {
    process.stderr.write(`release package failed: ${error.message}\n`);
    process.exit(1);
  }
}

module.exports = {
  RELEASE_VERSION_RE,
  buildTarInvocation,
  defaultBinaryPath,
  packageRelease,
  sha256,
  toTarPath
};
