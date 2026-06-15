'use strict';

const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { archiveName, resolveRustTarget, supportedTargets } = require('../scripts/lib/platform');
const { packageRelease } = require('../../../scripts/package-release');
const { assertSupportedTargetSet, isSafeSourceRef, validateRelease } = require('../../../scripts/check-release-version');

const preview = validateRelease({
  channel: 'preview',
  version: '0.1.1-preview.0',
  cargoVersion: '0.1.1',
  npmVersion: '0.1.1-preview.0'
});
assert.strictEqual(preview.tag, 'v0.1.1-preview.0');
assert.strictEqual(preview.source_ref, 'develop');
assert.strictEqual(preview.npm_dist_tag, 'preview');
assert.strictEqual(preview.prerelease, true);
assert.strictEqual(preview.archive_version, '0.1.1');
assert.strictEqual(preview.targets.length, supportedTargets().length);
assert(preview.targets.some(target => target.archive === 'tfy-0.1.1-linux-x86_64.tar.gz'));

const stable = validateRelease({
  channel: 'stable',
  version: '1.2.3',
  sourceRef: 'main',
  cargoVersion: '1.2.3',
  npmVersion: '1.2.3'
});
assert.strictEqual(stable.tag, 'v1.2.3');
assert.strictEqual(stable.npm_dist_tag, 'latest');
assert.strictEqual(stable.prerelease, false);
assert.strictEqual(stable.github_latest, true);

assert.throws(() => validateRelease({
  channel: 'stable',
  version: '1.2.3-preview.0',
  cargoVersion: '1.2.3',
  npmVersion: '1.2.3-preview.0'
}), /stable version must match/);
assert.throws(() => validateRelease({
  channel: 'preview',
  version: '1.2.3-preview.0',
  sourceRef: 'main',
  cargoVersion: '1.2.3',
  npmVersion: '1.2.3-preview.0'
}), /preview releases must use source_ref=develop or release/);
assert.throws(() => validateRelease({
  channel: 'preview',
  version: '1.2.3-preview.0',
  cargoVersion: '1.2.3-preview.0',
  npmVersion: '1.2.3-preview.0'
}), /Cargo workspace version must equal preview base/);

assert.throws(() => validateRelease({
  channel: 'preview',
  version: '1.2.3-preview.0',
  cargoVersion: '1.2.3',
  npmVersion: '1.2.3-preview.0',
  targets: supportedTargets().slice(1)
}), /release target matrix must exactly match supported targets/);
assert.throws(() => validateRelease({
  channel: 'preview',
  version: '1.2.3-preview.0',
  sourceRef: 'release/../bad',
  cargoVersion: '1.2.3',
  npmVersion: '1.2.3-preview.0'
}), /preview releases must use source_ref=develop or release/);
const duplicateTargets = supportedTargets();
duplicateTargets[1] = duplicateTargets[0];
assert.throws(() => assertSupportedTargetSet(duplicateTargets), /release target matrix must exactly match supported targets/);
assert.doesNotThrow(() => assertSupportedTargetSet(supportedTargets()));
assert.strictEqual(isSafeSourceRef('release/1.2.3'), true);
assert.strictEqual(isSafeSourceRef('release/../bad'), false);
assert.strictEqual(isSafeSourceRef('release/bad;rm'), false);
assert.strictEqual(isSafeSourceRef('release/.bad'), false);
assert.strictEqual(isSafeSourceRef('release/bad.'), false);
assert.strictEqual(isSafeSourceRef('release/foo/.bar'), false);

const workflow = fs.readFileSync(path.resolve(__dirname, '..', '..', '..', '.github', 'workflows', 'release.yml'), 'utf8');
const workflowTargets = Array.from(workflow.matchAll(/rust_target: ([a-zA-Z0-9_-]+-[a-zA-Z0-9_-]+-[a-zA-Z0-9_-]+(?:-[a-zA-Z0-9_-]+)?)/g)).map(match => match[1]).sort();
assert.deepStrictEqual(workflowTargets, supportedTargets().map(target => target.rustTarget).sort());

assert.throws(() => packageRelease({ root: process.cwd(), rustTarget: 'x86_64-unknown-linux-gnu' }), /version is required/);
assert.throws(() => packageRelease({ root: process.cwd(), version: '../0.1.0', rustTarget: 'x86_64-unknown-linux-gnu' }), /version must match/);

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'tfy-package-release-test-'));
const target = resolveRustTarget('x86_64-unknown-linux-gnu');
const binDir = path.join(tmp, 'target', target.rustTarget, 'release');
fs.mkdirSync(binDir, { recursive: true });
fs.writeFileSync(path.join(binDir, target.binName), '#!/usr/bin/env sh\necho packaged\n');
const manifest = packageRelease({ root: tmp, version: '0.1.1-preview.0', rustTarget: target.rustTarget, dist: 'dist' });
assert.strictEqual(manifest.archive_name, archiveName('0.1.1-preview.0', target));
assert.strictEqual(manifest.archive_name, 'tfy-0.1.1-linux-x86_64.tar.gz');
assert.strictEqual(fs.readFileSync(manifest.checksum_path, 'utf8'), `${manifest.sha256}  ${manifest.archive_name}\n`);
const listed = spawnSync('tar', ['-tzf', manifest.archive_path], { encoding: 'utf8' });
assert.strictEqual(listed.status, 0, listed.stderr);
assert.deepStrictEqual(listed.stdout.trim().split(/\r?\n/), [target.binName]);
