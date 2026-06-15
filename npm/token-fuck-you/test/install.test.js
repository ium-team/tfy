'use strict';

const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { resolvePlatform } = require('../scripts/lib/platform');
const {
  canonicalReleaseBase,
  copyLocalBinary,
  installFromRelease,
  manualInstallDestination,
  installVerifiedArchive,
  releaseUrls,
  defaultReleaseVersion,
  sha256,
  validateArchiveEntries
} = require('../scripts/install');

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: 'utf8', ...options });
  assert.strictEqual(result.status, 0, result.stderr || result.stdout);
}

const target = resolvePlatform('linux', 'x64');
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'tfy-install-test-'));
const src = path.join(tmp, 'src');
const archive = path.join(tmp, 'tfy-0.1.0-linux-x86_64.tar.gz');
const checksum = `${archive}.sha256`;
const vendor = path.join(tmp, 'vendor');
const destination = path.join(vendor, target.binName);
fs.mkdirSync(src, { recursive: true });
fs.writeFileSync(path.join(src, target.binName), '#!/usr/bin/env sh\necho tfy-fixture\n');
run('tar', ['-C', src, '-czf', archive, target.binName]);
fs.writeFileSync(checksum, `${sha256(archive)}  ${path.basename(archive)}\n`);
installVerifiedArchive({
  archivePath: archive,
  checksumPath: checksum,
  asset: path.basename(archive),
  platformTarget: target,
  vendorDirectory: vendor,
  destinationBinary: destination
});
assert.strictEqual(fs.readFileSync(destination, 'utf8'), fs.readFileSync(path.join(src, target.binName), 'utf8'));

fs.writeFileSync(checksum, `${'b'.repeat(64)}  ${path.basename(archive)}\n`);
assert.throws(() => installVerifiedArchive({
  archivePath: archive,
  checksumPath: checksum,
  asset: path.basename(archive),
  platformTarget: target,
  vendorDirectory: vendor,
  destinationBinary: destination
}), /checksum mismatch/);

assert.throws(() => validateArchiveEntries(['../tfy'], target), /archive must contain only|unsafe archive entry/);
assert.throws(() => validateArchiveEntries(['tfy', 'extra'], target), /archive must contain only tfy/);
assert.strictEqual(validateArchiveEntries(['./tfy'], target), 'tfy');

assert.strictEqual(canonicalReleaseBase('v0.1.0'), 'https://github.com/ium-team/tfy/releases/download/v0.1.0');
assert.strictEqual(defaultReleaseVersion('v0.1.0-preview.0'), '0.1.0-preview.0');
assert.deepStrictEqual(releaseUrls('0.1.0-preview.0', target), {
  asset: 'tfy-0.1.0-linux-x86_64.tar.gz',
  archiveUrl: 'https://github.com/ium-team/tfy/releases/download/v0.1.0-preview.0/tfy-0.1.0-linux-x86_64.tar.gz',
  checksumUrl: 'https://github.com/ium-team/tfy/releases/download/v0.1.0-preview.0/tfy-0.1.0-linux-x86_64.tar.gz.sha256'
});

const localBinary = path.join(tmp, 'local-tfy');
const copiedBinary = path.join(tmp, 'copied-local-tfy');
fs.writeFileSync(localBinary, '#!/usr/bin/env sh\necho local\n');
copyLocalBinary(localBinary, copiedBinary);
assert.strictEqual(fs.readFileSync(copiedBinary, 'utf8'), fs.readFileSync(localBinary, 'utf8'));
assert.match(manualInstallDestination(localBinary), /vendor[\\/]manual[\\/]local-tfy$/);


async function exerciseReleaseFlow() {
  await assert.rejects(() => installFromRelease({ platform: 'darwin', arch: 'x64' }), /Intel Mac prebuilt npm installs are not currently provided/);

  const releaseTmp = fs.mkdtempSync(path.join(os.tmpdir(), 'tfy-release-flow-test-'));
  const releaseSrc = path.join(releaseTmp, 'src');
  const releaseArchive = path.join(releaseTmp, 'tfy-0.1.0-linux-x86_64.tar.gz');
  const releaseChecksum = `${releaseArchive}.sha256`;
  const releaseVendor = path.join(releaseTmp, 'vendor');
  const releaseDestination = path.join(releaseVendor, target.binName);
  fs.mkdirSync(releaseSrc, { recursive: true });
  fs.writeFileSync(path.join(releaseSrc, target.binName), '#!/usr/bin/env sh\necho release-flow\n');
  run('tar', ['-C', releaseSrc, '-czf', releaseArchive, target.binName]);
  fs.writeFileSync(releaseChecksum, `${sha256(releaseArchive)}  ${path.basename(releaseArchive)}\n`);

  const expected = releaseUrls('0.1.0-preview.0', target);
  const seen = [];
  await installFromRelease({
    version: '0.1.0-preview.0',
    platformTarget: target,
    vendorDirectory: releaseVendor,
    destinationBinary: releaseDestination,
    downloadFile: async (url, destination) => {
      seen.push(url);
      if (url === expected.archiveUrl) return fs.copyFileSync(releaseArchive, destination);
      if (url === expected.checksumUrl) return fs.copyFileSync(releaseChecksum, destination);
      throw new Error(`unexpected release URL ${url}`);
    }
  });
  assert.deepStrictEqual(seen, [expected.archiveUrl, expected.checksumUrl]);
  assert.strictEqual(fs.readFileSync(releaseDestination, 'utf8'), fs.readFileSync(path.join(releaseSrc, target.binName), 'utf8'));
}

exerciseReleaseFlow().catch(error => {
  process.stderr.write(`${error.stack || error}\n`);
  process.exit(1);
});
