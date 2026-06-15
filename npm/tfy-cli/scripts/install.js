#!/usr/bin/env node
'use strict';

const crypto = require('crypto');
const fs = require('fs');
const https = require('https');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { currentPlatform, archiveName, cleanVersion } = require('./lib/platform');
const { parseChecksum } = require('./lib/checksum');

const root = path.resolve(__dirname, '..');
const pkg = require(path.join(root, 'package.json'));
const DEFAULT_DOWNLOAD_TIMEOUT_MS = 30_000;
const DEFAULT_REDIRECT_LIMIT = 5;

function log(message) {
  process.stderr.write(`[tfy installer] ${message}\n`);
}

function fail(message) {
  process.stderr.write(`[tfy installer] error: ${message}\n`);
  process.exit(1);
}

function sha256(file) {
  const hash = crypto.createHash('sha256');
  hash.update(fs.readFileSync(file));
  return hash.digest('hex');
}

function ensureExecutable(file) {
  if (process.platform !== 'win32') fs.chmodSync(file, 0o755);
}

function defaultInstallPaths(platformTarget) {
  const vendorDir = path.join(root, 'vendor', platformTarget.rustTarget);
  return { vendorDir, installedBin: path.join(vendorDir, platformTarget.binName) };
}

function manualInstallDestination(source) {
  const name = path.basename(source) || (process.platform === 'win32' ? 'tfy.exe' : 'tfy');
  return path.join(root, 'vendor', 'manual', name);
}

function copyLocalBinary(source, destination = manualInstallDestination(source)) {
  if (!fs.existsSync(source)) throw new Error(`TFY_BINARY_PATH does not exist: ${source}`);
  fs.mkdirSync(path.dirname(destination), { recursive: true });
  fs.copyFileSync(source, destination);
  ensureExecutable(destination);
  log(`installed local TFY binary from ${source}`);
  return destination;
}

function installLocalBinary(source, options = {}) {
  const resolvedSource = path.resolve(source);
  let destination = options.destinationBinary;
  if (!destination) {
    try {
      const platformTarget = options.platformTarget || currentPlatform(options);
      destination = defaultInstallPaths(platformTarget).installedBin;
    } catch (error) {
      destination = manualInstallDestination(resolvedSource);
      log(`${error.message}; copied local binary to ${destination}. Set TFY_BINARY_PATH at runtime on unsupported platforms.`);
    }
  }
  return copyLocalBinary(resolvedSource, destination);
}

function canonicalReleaseBase(version) {
  const cleanVersion = String(version).replace(/^v/, '');
  return `https://github.com/ium-team/tfy/releases/download/v${cleanVersion}`;
}

function defaultReleaseVersion(version) {
  return cleanVersion(version);
}

function releaseUrls(version, platformTarget) {
  const asset = archiveName(version, platformTarget);
  const base = canonicalReleaseBase(version);
  const archiveUrl = `${base}/${asset}`;
  return { asset, archiveUrl, checksumUrl: `${archiveUrl}.sha256` };
}

function download(url, destination, options = {}) {
  const timeoutMs = options.timeoutMs ?? DEFAULT_DOWNLOAD_TIMEOUT_MS;
  const redirectLimit = options.redirectLimit ?? DEFAULT_REDIRECT_LIMIT;
  return new Promise((resolve, reject) => {
    function fetch(currentUrl, remainingRedirects) {
      const request = https.get(currentUrl, response => {
        if ([301, 302, 303, 307, 308].includes(response.statusCode)) {
          response.resume();
          if (remainingRedirects <= 0) return reject(new Error(`too many redirects downloading ${url}`));
          if (!response.headers.location) return reject(new Error(`redirect missing Location for ${currentUrl}`));
          const nextUrl = new URL(response.headers.location, currentUrl).toString();
          return fetch(nextUrl, remainingRedirects - 1);
        }
        if (response.statusCode !== 200) {
          response.resume();
          return reject(new Error(`download failed ${response.statusCode}: ${currentUrl}`));
        }
        const out = fs.createWriteStream(destination, { flags: 'wx' });
        response.pipe(out);
        out.on('finish', () => out.close(resolve));
        out.on('error', reject);
      });
      request.setTimeout(timeoutMs, () => request.destroy(new Error(`download timed out: ${currentUrl}`)));
      request.on('error', reject);
    }
    fetch(url, redirectLimit);
  });
}

function normalizeArchiveEntry(entry) {
  return String(entry).replace(/\\/g, '/').replace(/^\.\//, '');
}

function validateArchiveEntries(entries, platformTarget) {
  const normalized = entries.map(normalizeArchiveEntry).filter(Boolean);
  if (normalized.length !== 1 || normalized[0] !== platformTarget.binName) {
    throw new Error(`archive must contain only ${platformTarget.binName}; found: ${normalized.join(', ') || '(empty)'}`);
  }
  const entry = normalized[0];
  if (path.posix.isAbsolute(entry) || entry.split('/').includes('..')) {
    throw new Error(`unsafe archive entry: ${entry}`);
  }
  return entry;
}

function listArchiveEntries(archivePath) {
  const listed = spawnSync('tar', ['-tzf', archivePath], { encoding: 'utf8' });
  if (listed.status !== 0) {
    throw new Error(`failed to inspect archive ${archivePath}: ${(listed.stderr || '').trim()}`);
  }
  return listed.stdout.split(/\r?\n/).filter(Boolean);
}

function installVerifiedArchive({ archivePath, checksumPath, asset, platformTarget, vendorDirectory, destinationBinary }) {
  const expected = parseChecksum(fs.readFileSync(checksumPath, 'utf8'), asset);
  const actual = sha256(archivePath);
  if (actual !== expected) throw new Error(`checksum mismatch for ${asset}: expected ${expected}, got ${actual}`);

  validateArchiveEntries(listArchiveEntries(archivePath), platformTarget);
  const extractDir = fs.mkdtempSync(path.join(os.tmpdir(), 'tfy-extract-'));
  const extractedPath = path.join(extractDir, platformTarget.binName);
  const tar = spawnSync('tar', ['-xzf', archivePath, '-C', extractDir], { stdio: 'inherit' });
  if (tar.status !== 0) throw new Error(`failed to extract ${asset} with tar`);
  const realExtractRoot = fs.realpathSync(extractDir);
  const realExtractedParent = fs.realpathSync(path.dirname(extractedPath));
  if (realExtractedParent !== realExtractRoot) throw new Error(`archive extracted outside temp directory: ${platformTarget.binName}`);
  const stat = fs.lstatSync(extractedPath);
  if (!stat.isFile() || stat.isSymbolicLink()) throw new Error(`archive entry is not a regular binary file: ${platformTarget.binName}`);

  fs.mkdirSync(vendorDirectory, { recursive: true });
  fs.copyFileSync(extractedPath, destinationBinary);
  ensureExecutable(destinationBinary);
}

async function installFromRelease(options = {}) {
  const platformTarget = options.platformTarget || currentPlatform(options);
  const version = defaultReleaseVersion(options.version || process.env.TFY_RELEASE_VERSION || pkg.version);
  const { asset, archiveUrl, checksumUrl } = releaseUrls(version, platformTarget);
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'tfy-npm-'));
  const archivePath = path.join(tmp, asset);
  const checksumPath = `${archivePath}.sha256`;
  const downloadFile = options.downloadFile || download;
  const defaults = defaultInstallPaths(platformTarget);
  const vendorDirectory = options.vendorDirectory || defaults.vendorDir;
  const destinationBinary = options.destinationBinary || defaults.installedBin;
  log(`downloading ${archiveUrl}`);
  await downloadFile(archiveUrl, archivePath);
  await downloadFile(checksumUrl, checksumPath);
  installVerifiedArchive({ archivePath, checksumPath, asset, platformTarget, vendorDirectory, destinationBinary });
  log(`installed ${destinationBinary}`);
}

async function main() {
  if (process.env.TFY_SKIP_INSTALL === '1') {
    log('TFY_SKIP_INSTALL=1; skipping binary install');
    return;
  }
  if (process.env.TFY_BINARY_PATH) {
    installLocalBinary(process.env.TFY_BINARY_PATH);
    return;
  }
  await installFromRelease();
}

if (require.main === module) {
  main().catch(error => fail(error.message));
}

module.exports = {
  canonicalReleaseBase,
  copyLocalBinary,
  defaultInstallPaths,
  defaultReleaseVersion,
  download,
  installFromRelease,
  installLocalBinary,
  manualInstallDestination,
  installVerifiedArchive,
  listArchiveEntries,
  normalizeArchiveEntry,
  releaseUrls,
  sha256,
  validateArchiveEntries
};
