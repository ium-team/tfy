'use strict';

const SUPPORTED = new Map([
  ['darwin:arm64', { platform: 'darwin', arch: 'arm64', rustTarget: 'aarch64-apple-darwin', archivePlatform: 'darwin', archiveArch: 'arm64', binName: 'tfy' }],
  ['linux:x64', { platform: 'linux', arch: 'x64', rustTarget: 'x86_64-unknown-linux-gnu', archivePlatform: 'linux', archiveArch: 'x86_64', binName: 'tfy' }],
  ['linux:arm64', { platform: 'linux', arch: 'arm64', rustTarget: 'aarch64-unknown-linux-gnu', archivePlatform: 'linux', archiveArch: 'arm64', binName: 'tfy' }],
  ['win32:x64', { platform: 'win32', arch: 'x64', rustTarget: 'x86_64-pc-windows-msvc', archivePlatform: 'windows', archiveArch: 'x86_64', binName: 'tfy.exe' }]
]);

function cloneTarget(key, target) {
  return { ...target, key };
}

function sourceBuildGuidance() {
  return 'Intel Mac prebuilt npm installs are not currently provided. Build from source with `git clone https://github.com/ium-team/tfy && cd tfy && cargo install --path crates/tfy-cli`, or set TFY_BINARY_PATH to a built tfy binary.';
}

function supportedPlatformList() {
  return Array.from(SUPPORTED.keys()).join(', ');
}

function unsupportedPlatformMessage(platform, arch) {
  const key = `${platform}:${arch}`;
  return `Unsupported TFY npm platform ${key}. Supported prebuilt platforms: ${supportedPlatformList()}. ${sourceBuildGuidance()}`;
}

function currentPlatform(options = {}) {
  return resolvePlatform(options.platform || process.platform, options.arch || process.arch);
}

function resolvePlatform(platform, arch) {
  const key = `${platform}:${arch}`;
  const resolved = SUPPORTED.get(key);
  if (!resolved) {
    throw new Error(unsupportedPlatformMessage(platform, arch));
  }
  return cloneTarget(key, resolved);
}

function supportedTargets() {
  return Array.from(SUPPORTED.entries()).map(([key, target]) => cloneTarget(key, target));
}

function resolveRustTarget(rustTarget) {
  const resolved = supportedTargets().find(target => target.rustTarget === rustTarget);
  if (!resolved) {
    const supported = supportedTargets().map(target => target.rustTarget).join(', ');
    throw new Error(`Unsupported TFY Rust target ${rustTarget}. Supported: ${supported}`);
  }
  return resolved;
}

function cleanVersion(version) {
  return String(version).replace(/^v/, '');
}

function baseVersion(version) {
  return cleanVersion(version).replace(/-preview\.\d+$/, '');
}

function assetBaseName(version, target) {
  return `tfy-${baseVersion(version)}-${target.archivePlatform}-${target.archiveArch}`;
}

function archiveName(version, target) {
  return `${assetBaseName(version, target)}.tar.gz`;
}

module.exports = {
  SUPPORTED,
  archiveName,
  assetBaseName,
  baseVersion,
  cleanVersion,
  currentPlatform,
  sourceBuildGuidance,
  resolvePlatform,
  resolveRustTarget,
  supportedPlatformList,
  supportedTargets,
  unsupportedPlatformMessage
};
