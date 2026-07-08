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
  logHumanOptInGuidance,
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
assert.strictEqual(defaultReleaseVersion('v0.1.1-beta.0'), '0.1.1-beta.0');
assert.deepStrictEqual(releaseUrls('0.1.1-beta.0', target), {
  asset: 'tfy-0.1.1-linux-x86_64.tar.gz',
  archiveUrl: 'https://github.com/ium-team/tfy/releases/download/v0.1.1-beta.0/tfy-0.1.1-linux-x86_64.tar.gz',
  checksumUrl: 'https://github.com/ium-team/tfy/releases/download/v0.1.1-beta.0/tfy-0.1.1-linux-x86_64.tar.gz.sha256'
});
assert.strictEqual(defaultReleaseVersion('v0.1.0-preview.0'), '0.1.0-preview.0');

const rcHome = path.join(tmp, 'home');
fs.mkdirSync(path.join(rcHome, '.config', 'fish'), { recursive: true });
const rcFiles = [
  path.join(rcHome, '.bashrc'),
  path.join(rcHome, '.bash_profile'),
  path.join(rcHome, '.zshrc'),
  path.join(rcHome, '.config', 'fish', 'config.fish')
];
for (const rcFile of rcFiles) fs.writeFileSync(rcFile, `# user rc ${path.basename(rcFile)}\n`);
const rcBefore = new Map(rcFiles.map(rcFile => [rcFile, fs.readFileSync(rcFile, 'utf8')]));
const originalHome = process.env.HOME;
const originalUserProfile = process.env.USERPROFILE;
process.env.HOME = rcHome;
process.env.USERPROFILE = rcHome;
process.on('exit', () => {
  if (originalHome === undefined) delete process.env.HOME; else process.env.HOME = originalHome;
  if (originalUserProfile === undefined) delete process.env.USERPROFILE; else process.env.USERPROFILE = originalUserProfile;
});

const localBinary = path.join(tmp, 'local-tfy');
const copiedBinary = path.join(tmp, 'copied-local-tfy');
fs.writeFileSync(localBinary, '#!/usr/bin/env sh\necho local\n');
let copiedStderr = '';
const originalStderrWrite = process.stderr.write;
process.stderr.write = function(chunk, encoding, callback) {
  copiedStderr += String(chunk);
  if (typeof callback === 'function') callback();
  return true;
};
try {
  copyLocalBinary(localBinary, copiedBinary);
} finally {
  process.stderr.write = originalStderrWrite;
}
assert.strictEqual(fs.readFileSync(copiedBinary, 'utf8'), fs.readFileSync(localBinary, 'utf8'));
assert.match(copiedStderr, /human auto-activation is opt-in/);
assert.match(copiedStderr, /tfy setup --human --apply/);
assert.match(manualInstallDestination(localBinary), /vendor[\\/]manual[\\/]local-tfy$/);
for (const rcFile of rcFiles) assert.strictEqual(fs.readFileSync(rcFile, 'utf8'), rcBefore.get(rcFile));

let guidanceOnly = '';
process.stderr.write = function(chunk, encoding, callback) {
  guidanceOnly += String(chunk);
  if (typeof callback === 'function') callback();
  return true;
};
try {
  logHumanOptInGuidance();
} finally {
  process.stderr.write = originalStderrWrite;
}
assert.match(guidanceOnly, /trusted TFY-marked repos: tfy setup --human --apply/);


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
  let releaseStderr = '';
  process.stderr.write = function(chunk, encoding, callback) {
    releaseStderr += String(chunk);
    if (typeof callback === 'function') callback();
    return true;
  };
  try {
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
  } finally {
    process.stderr.write = originalStderrWrite;
  }
  assert.deepStrictEqual(seen, [expected.archiveUrl, expected.checksumUrl]);
  assert.strictEqual(fs.readFileSync(releaseDestination, 'utf8'), fs.readFileSync(path.join(releaseSrc, target.binName), 'utf8'));
  assert.match(releaseStderr, /human auto-activation is opt-in/);
  assert.match(releaseStderr, /tfy setup --human --apply/);
  for (const rcFile of rcFiles) assert.strictEqual(fs.readFileSync(rcFile, 'utf8'), rcBefore.get(rcFile));
}

exerciseReleaseFlow().catch(error => {
  process.stderr.write(`${error.stack || error}\n`);
  process.exit(1);
});
