#!/usr/bin/env node
'use strict';

const path = require('path');
const { validateRelease } = require('./check-release-version');

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

function shellQuote(value) {
  return `'${String(value).replace(/'/g, `'"'"'`)}'`;
}

function npmPublishPlan(options) {
  const root = options.root ? path.resolve(options.root) : process.cwd();
  const metadata = validateRelease({
    root,
    version: options.version,
    channel: options.channel,
    sourceRef: options.sourceRef,
    cargoVersion: options.cargoVersion,
    npmVersion: options.npmVersion
  });
  const packageDir = 'npm/token-fuck-you';
  const publishCommand = [
    'npm',
    'publish',
    packageDir,
    '--tag',
    metadata.npm_dist_tag,
    '--access',
    'public'
  ];
  const verifyCommands = [
    ['npm', 'view', `${metadata.package_name}@${metadata.version}`, 'version'],
    ['npm', 'view', metadata.package_name, 'dist-tags', '--json']
  ];
  return {
    schema_version: 1,
    action: 'npm_publish_plan',
    package_name: metadata.package_name,
    package_dir: packageDir,
    binary_name: metadata.binary_name,
    channel: metadata.channel,
    version: metadata.version,
    npm_dist_tag: metadata.npm_dist_tag,
    github_release_tag: metadata.tag,
    github_release_required_first: true,
    publish_command: publishCommand,
    publish_command_text: publishCommand.map(shellQuote).join(' '),
    verify_commands: verifyCommands,
    verify_command_text: verifyCommands.map(command => command.map(shellQuote).join(' ')),
    warnings: [
      'This script does not publish to npm; it only validates and prints the exact command.',
      'Create and verify the matching GitHub Release assets/checksums before npm publishing.',
      metadata.channel === 'stable'
        ? 'Stable publishes must use the latest dist-tag and a plain N.N.N version.'
        : 'Public-test publishes must use the preview dist-tag and an N.N.N-preview.N version.',
      'The installed executable remains tfy even though the npm package is token-fuck-you.'
    ]
  };
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const result = npmPublishPlan({
    version: args.version,
    channel: args.channel,
    sourceRef: args.sourceRef,
    cargoVersion: args.cargoVersion,
    npmVersion: args.npmVersion,
    root: args.root
  });
  if (args.json) {
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
    return;
  }
  process.stdout.write([
    `npm package: ${result.package_name}`,
    `package dir: ${result.package_dir}`,
    `installed command: ${result.binary_name}`,
    `channel: ${result.channel}`,
    `version: ${result.version}`,
    `npm dist-tag: ${result.npm_dist_tag}`,
    `GitHub Release required first: ${result.github_release_tag}`,
    '',
    'Publish command:',
    `  ${result.publish_command_text}`,
    '',
    'Verify after publish:',
    ...result.verify_command_text.map(command => `  ${command}`),
    ''
  ].join('\n'));
}

if (require.main === module) {
  try {
    main();
  } catch (error) {
    process.stderr.write(`npm publish plan failed: ${error.message}\n`);
    process.exit(1);
  }
}

module.exports = {
  npmPublishPlan,
  parseArgs,
  shellQuote
};
