#!/usr/bin/env node
'use strict';

const { spawnSync } = require('child_process');
const { NPM_PACKAGE_NAME } = require('./release-config');
const { BETA_VERSION_RE, STABLE_VERSION_RE } = require('./check-release-version');
const { cleanVersion } = require('../npm/tfy-cli/scripts/lib/platform');

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (!token.startsWith('--')) throw new Error(`unexpected argument: ${token}`);
    const key = token.slice(2).replace(/-([a-z])/g, (_, ch) => ch.toUpperCase());
    if (key === 'json' || key === 'allowPrestablePrereleaseLatest') {
      args[key] = true;
      continue;
    }
    const value = argv[i + 1];
    if (!value || value.startsWith('--')) throw new Error(`missing value for ${token}`);
    args[key] = value;
    i += 1;
  }
  return args;
}

function readRegistryDistTags(packageName = NPM_PACKAGE_NAME) {
  const result = spawnSync('npm', ['view', packageName, 'dist-tags', '--json'], { encoding: 'utf8' });
  if (result.status !== 0) {
    throw new Error(`npm dist-tag lookup failed: ${(result.stderr || result.stdout || '').trim()}`);
  }
  return JSON.parse(result.stdout || '{}');
}

function validateDistTags(options) {
  const packageName = options.packageName || NPM_PACKAGE_NAME;
  const version = cleanVersion(options.version || '');
  const channel = options.channel;
  const distTags = options.distTags || {};
  const errors = [];

  if (!version) errors.push('version is required');
  if (!['beta', 'stable'].includes(channel)) {
    errors.push('channel must be beta or stable');
  } else if (channel === 'beta' && !BETA_VERSION_RE.test(version)) {
    errors.push(`beta version must match N.N.N-beta.N, got ${version}`);
  } else if (channel === 'stable' && !STABLE_VERSION_RE.test(version)) {
    errors.push(`stable version must match N.N.N, got ${version}`);
  }

  if (channel === 'beta') {
    if (distTags.beta !== version) {
      errors.push(`beta dist-tag must point at ${version}, got ${distTags.beta || '(missing)'}`);
    }
    if (distTags.latest && /-(?:beta|preview)\./.test(cleanVersion(distTags.latest)) && !options.allowPrestablePrereleaseLatest) {
      errors.push(`latest dist-tag must not point at prerelease version ${distTags.latest}`);
    }
  }

  if (channel === 'stable' && distTags.latest !== version) {
    errors.push(`latest dist-tag must point at stable version ${version}, got ${distTags.latest || '(missing)'}`);
  }

  return {
    schema_version: 1,
    action: 'npm_dist_tag_check',
    package_name: packageName,
    channel,
    version,
    dist_tags: distTags,
    allow_prestable_prerelease_latest: Boolean(options.allowPrestablePrereleaseLatest),
    status: errors.length ? 'fail' : 'pass',
    errors
  };
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const distTags = args.distTagsJson ? JSON.parse(args.distTagsJson) : readRegistryDistTags(args.packageName || NPM_PACKAGE_NAME);
  const result = validateDistTags({
    packageName: args.packageName || NPM_PACKAGE_NAME,
    version: args.version,
    channel: args.channel,
    distTags,
    allowPrestablePrereleaseLatest: Boolean(args.allowPrestablePrereleaseLatest)
  });
  if (args.json) {
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  } else if (result.status === 'pass') {
    process.stdout.write(`npm dist-tag check passed for ${result.package_name}@${result.version} (${result.channel})\n`);
  } else {
    process.stderr.write(`npm dist-tag check failed for ${result.package_name}@${result.version} (${result.channel}): ${result.errors.join('; ')}\n`);
  }
  if (result.status !== 'pass') process.exit(1);
}

if (require.main === module) {
  try {
    main();
  } catch (error) {
    process.stderr.write(`npm dist-tag check failed: ${error.message}\n`);
    process.exit(1);
  }
}

module.exports = {
  parseArgs,
  readRegistryDistTags,
  validateDistTags
};
