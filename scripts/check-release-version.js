#!/usr/bin/env node
'use strict';

const fs = require('fs');
const path = require('path');
const { archiveName, baseVersion, cleanVersion, supportedTargets } = require('../npm/tfy-cli/scripts/lib/platform');

const STABLE_VERSION_RE = /^\d+\.\d+\.\d+$/;
const PREVIEW_VERSION_RE = /^(\d+\.\d+\.\d+)-preview\.\d+$/;

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (!token.startsWith('--')) throw new Error(`unexpected argument: ${token}`);
    const key = token.slice(2).replace(/-([a-z])/g, (_, ch) => ch.toUpperCase());
    if (key === 'json') {
      args.json = true;
      continue;
    }
    const value = argv[i + 1];
    if (!value || value.startsWith('--')) throw new Error(`missing value for ${token}`);
    args[key] = value;
    i += 1;
  }
  return args;
}

function readWorkspaceCargoVersion(root = process.cwd()) {
  const manifest = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8');
  const workspacePackage = manifest.match(/\[workspace\.package\]([\s\S]*?)(?:\n\[|$)/);
  if (!workspacePackage) throw new Error('Cargo.toml is missing [workspace.package]');
  const version = workspacePackage[1].match(/^version\s*=\s*"([^"]+)"/m);
  if (!version) throw new Error('Cargo.toml [workspace.package] is missing version');
  return version[1];
}

function readNpmPackageVersion(root = process.cwd()) {
  const pkg = JSON.parse(fs.readFileSync(path.join(root, 'npm', 'tfy-cli', 'package.json'), 'utf8'));
  if (!pkg.version) throw new Error('npm/tfy-cli/package.json is missing version');
  return pkg.version;
}

function defaultSourceRef(channel) {
  if (channel === 'stable') return 'main';
  if (channel === 'preview') return 'develop';
  throw new Error(`unsupported release channel: ${channel}`);
}

function isSafeSourceRef(sourceRef) {
  if (typeof sourceRef !== 'string') return false;
  if (sourceRef === 'main' || sourceRef === 'develop') return true;
  if (!/^release\/[A-Za-z0-9._/-]+$/.test(sourceRef)) return false;
  if (sourceRef.includes('..') || sourceRef.includes('//') || sourceRef.endsWith('/')) return false;
  if (sourceRef.includes('@{')) return false;
  if (sourceRef.split('/').some(segment => segment === '' || segment.startsWith('.') || segment.endsWith('.') || segment.endsWith('.lock'))) return false;
  return true;
}

function sourceRefAllowed(channel, sourceRef) {
  if (!isSafeSourceRef(sourceRef)) return false;
  if (channel === 'stable') return sourceRef === 'main';
  if (channel === 'preview') return sourceRef === 'develop' || sourceRef.startsWith('release/');
  return false;
}

function targetIdentity(target) {
  return `${target.key}:${target.rustTarget}:${target.archivePlatform}:${target.archiveArch}:${target.binName}`;
}

function assertSupportedTargetSet(targets) {
  const actual = targets.map(targetIdentity).sort();
  const expected = supportedTargets().map(targetIdentity).sort();
  const missing = expected.filter(item => !actual.includes(item));
  const extra = actual.filter(item => !expected.includes(item));
  const duplicate = actual.filter((item, index) => actual.indexOf(item) !== index);
  if (missing.length || extra.length || duplicate.length || actual.length !== expected.length) {
    throw new Error(`release target matrix must exactly match supported targets; missing=${missing.join(',') || '(none)'} extra=${extra.join(',') || '(none)'} duplicate=${[...new Set(duplicate)].join(',') || '(none)'}`);
  }
}

function metadataForTarget(version, target) {
  const archive = archiveName(version, target);
  return {
    key: target.key,
    platform: target.platform,
    arch: target.arch,
    rustTarget: target.rustTarget,
    archivePlatform: target.archivePlatform,
    archiveArch: target.archiveArch,
    binName: target.binName,
    archive,
    checksum: `${archive}.sha256`
  };
}

function validateRelease(options) {
  const channel = options.channel;
  if (!['stable', 'preview'].includes(channel)) throw new Error('channel must be stable or preview');

  const version = cleanVersion(options.version || '');
  if (!version) throw new Error('version is required');
  const versionBase = baseVersion(version);
  const sourceRef = options.sourceRef || defaultSourceRef(channel);
  const cargoVersion = cleanVersion(options.cargoVersion || readWorkspaceCargoVersion(options.root));
  const npmVersion = cleanVersion(options.npmVersion || readNpmPackageVersion(options.root));
  const errors = [];

  if (!sourceRefAllowed(channel, sourceRef)) {
    errors.push(channel === 'stable'
      ? `stable releases must use source_ref=main, got ${sourceRef}`
      : `preview releases must use source_ref=develop or release/*, got ${sourceRef}`);
  }

  if (channel === 'stable') {
    if (!STABLE_VERSION_RE.test(version)) errors.push(`stable version must match N.N.N, got ${version}`);
    if (cargoVersion !== version) errors.push(`Cargo workspace version must equal stable version ${version}, got ${cargoVersion}`);
    if (npmVersion !== version) errors.push(`npm package version must equal stable version ${version}, got ${npmVersion}`);
  } else {
    const match = version.match(PREVIEW_VERSION_RE);
    if (!match) errors.push(`preview version must match N.N.N-preview.N, got ${version}`);
    if (cargoVersion !== versionBase) errors.push(`Cargo workspace version must equal preview base ${versionBase}, got ${cargoVersion}`);
    if (npmVersion !== version) errors.push(`npm package version must equal preview version ${version}, got ${npmVersion}`);
  }

  const inputTargets = options.targets || supportedTargets();
  try {
    assertSupportedTargetSet(inputTargets);
  } catch (error) {
    errors.push(error.message);
  }
  const targets = inputTargets.map(target => metadataForTarget(version, target));

  if (errors.length) {
    const error = new Error(errors.join('; '));
    error.errors = errors;
    throw error;
  }

  return {
    schema_version: 1,
    package_name: 'token-fuck-you',
    binary_name: 'tfy',
    channel,
    npm_dist_tag: channel === 'stable' ? 'latest' : 'preview',
    version,
    base_version: versionBase,
    cargo_version: cargoVersion,
    npm_version: npmVersion,
    tag: `v${version}`,
    source_ref: sourceRef,
    prerelease: channel === 'preview',
    github_latest: channel === 'stable',
    archive_version: versionBase,
    targets
  };
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const result = validateRelease({
    version: args.version,
    channel: args.channel,
    sourceRef: args.sourceRef,
    cargoVersion: args.cargoVersion,
    npmVersion: args.npmVersion,
    root: args.root ? path.resolve(args.root) : process.cwd()
  });
  process.stdout.write(args.json ? `${JSON.stringify(result, null, 2)}\n` : `${result.tag}\n`);
}

if (require.main === module) {
  try {
    main();
  } catch (error) {
    process.stderr.write(`release version check failed: ${error.message}\n`);
    process.exit(1);
  }
}

module.exports = {
  PREVIEW_VERSION_RE,
  STABLE_VERSION_RE,
  assertSupportedTargetSet,
  defaultSourceRef,
  isSafeSourceRef,
  readNpmPackageVersion,
  readWorkspaceCargoVersion,
  sourceRefAllowed,
  validateRelease
};
