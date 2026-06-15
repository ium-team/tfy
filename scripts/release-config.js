#!/usr/bin/env node
'use strict';

const fs = require('fs');
const path = require('path');

const DEFAULT_REPO_ROOT = path.resolve(__dirname, '..');

const NPM_PACKAGE_NAME = '@ium/tfy-cli';
const NPM_PACKAGE_DIR = 'npm/tfy-cli';
const NPM_PUBLISH_TARGET = `./${NPM_PACKAGE_DIR}`;
const NPM_BINARY_NAME = 'tfy';

function readNpmPackageManifest(root = DEFAULT_REPO_ROOT) {
  const packagePath = path.join(root, NPM_PACKAGE_DIR, 'package.json');
  return JSON.parse(fs.readFileSync(packagePath, 'utf8'));
}

function assertNpmPackageIdentity(root = DEFAULT_REPO_ROOT) {
  const pkg = readNpmPackageManifest(root);
  if (pkg.name !== NPM_PACKAGE_NAME) {
    throw new Error(`${NPM_PACKAGE_DIR}/package.json name must equal ${NPM_PACKAGE_NAME}, got ${pkg.name || '(missing)'}`);
  }
  if (!pkg.bin || pkg.bin[NPM_BINARY_NAME] == null) {
    throw new Error(`${NPM_PACKAGE_DIR}/package.json bin must expose ${NPM_BINARY_NAME}`);
  }
  return pkg;
}

module.exports = {
  DEFAULT_REPO_ROOT,
  assertNpmPackageIdentity,
  readNpmPackageManifest,
  NPM_BINARY_NAME,
  NPM_PACKAGE_DIR,
  NPM_PACKAGE_NAME,
  NPM_PUBLISH_TARGET
};
