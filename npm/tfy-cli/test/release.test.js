'use strict';

const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');
const { archiveName, resolveRustTarget, supportedTargets } = require('../scripts/lib/platform');
const { buildTarInvocation, packageRelease } = require('../../../scripts/package-release');
const { npmPublishPlan } = require('../../../scripts/npm-publish-plan');
const { validateDistTags } = require('../../../scripts/npm-dist-tag-check');
const { assertSupportedTargetSet, isSafeSourceRef, validateRelease } = require('../../../scripts/check-release-version');


const identityRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'tfy-identity-test-'));
fs.mkdirSync(path.join(identityRoot, 'npm', 'tfy-cli'), { recursive: true });
fs.writeFileSync(path.join(identityRoot, 'npm', 'tfy-cli', 'package.json'), JSON.stringify({
  name: 'wrong-name',
  version: '0.1.1-beta.0',
  bin: { tfy: 'bin/tfy.js' }
}));
assert.throws(() => validateRelease({
  root: identityRoot,
  channel: 'beta',
  version: '0.1.1-beta.0',
  cargoVersion: '0.1.1',
  npmVersion: '0.1.1-beta.0'
}), /package.json name must equal @ium\/tfy-cli/);
fs.writeFileSync(path.join(identityRoot, 'npm', 'tfy-cli', 'package.json'), JSON.stringify({
  name: '@ium/tfy-cli',
  version: '0.1.1-beta.0',
  bin: { nope: 'bin/tfy.js' }
}));
assert.throws(() => validateRelease({
  root: identityRoot,
  channel: 'beta',
  version: '0.1.1-beta.0',
  cargoVersion: '0.1.1',
  npmVersion: '0.1.1-beta.0'
}), /package.json bin must expose tfy/);

const beta = validateRelease({
  channel: 'beta',
  version: '0.1.1-beta.0',
  cargoVersion: '0.1.1',
  npmVersion: '0.1.1-beta.0'
});
assert.strictEqual(beta.tag, 'v0.1.1-beta.0');
assert.strictEqual(beta.source_ref, 'develop');
assert.strictEqual(beta.npm_dist_tag, 'beta');
assert.strictEqual(beta.prerelease, true);
assert.strictEqual(beta.archive_version, '0.1.1');
assert.strictEqual(beta.targets.length, supportedTargets().length);
assert(beta.targets.some(target => target.archive === 'tfy-0.1.1-linux-x86_64.tar.gz'));

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

const betaPublish = npmPublishPlan({
  channel: 'beta',
  version: '0.1.1-beta.0',
  sourceRef: 'develop',
  cargoVersion: '0.1.1',
  npmVersion: '0.1.1-beta.0'
});
assert.strictEqual(betaPublish.package_name, '@ium/tfy-cli');
assert.strictEqual(betaPublish.package_dir, 'npm/tfy-cli');
assert.strictEqual(betaPublish.binary_name, 'tfy');
assert.strictEqual(betaPublish.npm_dist_tag, 'beta');
assert.deepStrictEqual(betaPublish.publish_command, ['npm', 'publish', './npm/tfy-cli', '--tag', 'beta', '--access', 'public']);
assert(betaPublish.github_release_required_first);
assert(betaPublish.warnings.some(warning => warning.includes('npm dist-tag check')));
assert(betaPublish.verify_commands.some(command => command[0] === 'node' && command[1] === 'scripts/npm-dist-tag-check.js'));
assert.strictEqual(validateDistTags({ channel: 'beta', version: '0.1.1-beta.0', distTags: { beta: '0.1.1-beta.0' } }).status, 'pass');
assert.strictEqual(validateDistTags({ channel: 'beta', version: '0.1.1-beta.0', distTags: { beta: '0.1.1-beta.0', latest: '0.1.1-beta.0' } }).status, 'fail');
assert.strictEqual(validateDistTags({ channel: 'beta', version: '0.1.1-beta.0', distTags: { beta: '0.1.1-beta.0', latest: '0.1.0-beta.0' }, allowPrestablePrereleaseLatest: true }).status, 'pass');
assert.strictEqual(validateDistTags({ channel: 'beta', version: '0.1.1-beta.0', distTags: { beta: '0.1.1-beta.0', latest: '0.1.0' } }).status, 'pass');
assert.strictEqual(validateDistTags({ channel: 'beta', version: '0.1.1', distTags: { beta: '0.1.1' } }).status, 'fail');
const betaDistTagCli = spawnSync(process.execPath, [path.resolve(__dirname, '..', '..', '..', 'scripts', 'npm-dist-tag-check.js'), '--version', '0.1.1-beta.0', '--channel', 'beta', '--dist-tags-json', '{"beta":"0.1.1-beta.0"}', '--json'], { encoding: 'utf8' });
assert.strictEqual(betaDistTagCli.status, 0, betaDistTagCli.stderr);
assert.strictEqual(JSON.parse(betaDistTagCli.stdout).status, 'pass');
const betaLatestCli = spawnSync(process.execPath, [path.resolve(__dirname, '..', '..', '..', 'scripts', 'npm-dist-tag-check.js'), '--version', '0.1.1-beta.0', '--channel', 'beta', '--dist-tags-json', '{"beta":"0.1.1-beta.0","latest":"0.1.1-beta.0"}', '--json'], { encoding: 'utf8' });
assert.strictEqual(betaLatestCli.status, 1);
assert.strictEqual(JSON.parse(betaLatestCli.stdout).status, 'fail');
const betaAllowedLatestCli = spawnSync(process.execPath, [path.resolve(__dirname, '..', '..', '..', 'scripts', 'npm-dist-tag-check.js'), '--version', '0.1.1-beta.0', '--channel', 'beta', '--dist-tags-json', '{"beta":"0.1.1-beta.0","latest":"0.1.0-beta.0"}', '--allow-prestable-prerelease-latest', '--json'], { encoding: 'utf8' });
assert.strictEqual(betaAllowedLatestCli.status, 0, betaAllowedLatestCli.stderr);
assert.strictEqual(JSON.parse(betaAllowedLatestCli.stdout).allow_prestable_prerelease_latest, true);

const stablePublish = npmPublishPlan({
  channel: 'stable',
  version: '1.2.3',
  sourceRef: 'main',
  cargoVersion: '1.2.3',
  npmVersion: '1.2.3'
});
assert.strictEqual(stablePublish.npm_dist_tag, 'latest');
assert.deepStrictEqual(stablePublish.publish_command, ['npm', 'publish', './npm/tfy-cli', '--tag', 'latest', '--access', 'public']);
assert(stablePublish.warnings.some(warning => warning.includes('Only reviewed stable releases')));
assert.strictEqual(validateDistTags({ channel: 'stable', version: '1.2.3', distTags: { latest: '1.2.3', beta: '1.2.4-beta.0' } }).status, 'pass');
assert.strictEqual(validateDistTags({ channel: 'stable', version: '1.2.3', distTags: { latest: '1.2.2' } }).status, 'fail');
assert.strictEqual(validateDistTags({ channel: 'stable', version: '1.2.3-beta.0', distTags: { latest: '1.2.3-beta.0' } }).status, 'fail');
assert.throws(() => npmPublishPlan({
  channel: 'stable',
  version: '1.2.3-beta.0',
  sourceRef: 'develop',
  cargoVersion: '1.2.3',
  npmVersion: '1.2.3-beta.0'
}), /stable releases must use source_ref=main|stable version must match/);

assert.throws(() => validateRelease({
  channel: 'stable',
  version: '1.2.3-beta.0',
  cargoVersion: '1.2.3',
  npmVersion: '1.2.3-beta.0'
}), /stable version must match/);
assert.throws(() => validateRelease({
  channel: 'beta',
  version: '1.2.3-beta.0',
  sourceRef: 'main',
  cargoVersion: '1.2.3',
  npmVersion: '1.2.3-beta.0'
}), /beta releases must use source_ref=develop or release/);
assert.throws(() => validateRelease({
  channel: 'beta',
  version: '1.2.3-beta.0',
  cargoVersion: '1.2.3-beta.0',
  npmVersion: '1.2.3-beta.0'
}), /Cargo workspace version must equal beta base/);

assert.throws(() => validateRelease({
  channel: 'beta',
  version: '1.2.3-beta.0',
  cargoVersion: '1.2.3',
  npmVersion: '1.2.3-beta.0',
  targets: supportedTargets().slice(1)
}), /release target matrix must exactly match supported targets/);
assert.throws(() => validateRelease({
  channel: 'beta',
  version: '1.2.3-beta.0',
  sourceRef: 'release/../bad',
  cargoVersion: '1.2.3',
  npmVersion: '1.2.3-beta.0'
}), /beta releases must use source_ref=develop or release/);
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

const windowsTarget = resolveRustTarget('x86_64-pc-windows-msvc');
const windowsTar = buildTarInvocation({
  binary: 'D:\\a\\tfy\\tfy\\target\\x86_64-pc-windows-msvc\\release\\tfy.exe',
  dist: 'D:\\a\\tfy\\tfy\\dist',
  archivePath: 'D:\\a\\tfy\\tfy\\dist\\tfy-0.1.0-windows-x86_64.tar.gz',
  target: windowsTarget,
  pathLib: path.win32
});
assert.deepStrictEqual(windowsTar.args, [
  '-C',
  '../target/x86_64-pc-windows-msvc/release',
  '-czf',
  'tfy-0.1.0-windows-x86_64.tar.gz',
  'tfy.exe'
]);
assert.strictEqual(windowsTar.options.cwd, 'D:\\a\\tfy\\tfy\\dist');
assert(!windowsTar.args.some(arg => /^D:/.test(arg)), 'tar arguments must not include Windows drive absolute paths');

assert.throws(() => buildTarInvocation({
  binary: 'E:\\other\\target\\x86_64-pc-windows-msvc\\release\\tfy.exe',
  dist: 'D:\\a\\tfy\\tfy\\dist',
  archivePath: 'D:\\a\\tfy\\tfy\\dist\\tfy-0.1.0-windows-x86_64.tar.gz',
  target: windowsTarget,
  pathLib: path.win32
}), /tar source directory must be relative to dist/);

assert.throws(() => packageRelease({ root: process.cwd(), rustTarget: 'x86_64-unknown-linux-gnu' }), /version is required/);
assert.throws(() => packageRelease({ root: process.cwd(), version: '../0.1.0', rustTarget: 'x86_64-unknown-linux-gnu' }), /version must match/);

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'tfy-package-release-test-'));
const target = resolveRustTarget('x86_64-unknown-linux-gnu');
const binDir = path.join(tmp, 'target', target.rustTarget, 'release');
fs.mkdirSync(binDir, { recursive: true });
fs.writeFileSync(path.join(binDir, target.binName), '#!/usr/bin/env sh\necho packaged\n');
const manifest = packageRelease({ root: tmp, version: '0.1.1-beta.0', rustTarget: target.rustTarget, dist: 'dist' });
assert.strictEqual(manifest.archive_name, archiveName('0.1.1-beta.0', target));
assert.strictEqual(manifest.archive_name, 'tfy-0.1.1-linux-x86_64.tar.gz');
assert.strictEqual(fs.readFileSync(manifest.checksum_path, 'utf8'), `${manifest.sha256}  ${manifest.archive_name}\n`);
const listed = spawnSync('tar', ['-tzf', manifest.archive_path], { encoding: 'utf8' });
assert.strictEqual(listed.status, 0, listed.stderr);
assert.deepStrictEqual(listed.stdout.trim().split(/\r?\n/), [target.binName]);
