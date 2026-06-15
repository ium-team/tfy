'use strict';

const SUPPORTED = new Map([
  ['darwin:x64', { platform: 'darwin', arch: 'x64', rustTarget: 'x86_64-apple-darwin', archivePlatform: 'darwin', archiveArch: 'x86_64', binName: 'tfy' }],
  ['darwin:arm64', { platform: 'darwin', arch: 'arm64', rustTarget: 'aarch64-apple-darwin', archivePlatform: 'darwin', archiveArch: 'arm64', binName: 'tfy' }],
  ['linux:x64', { platform: 'linux', arch: 'x64', rustTarget: 'x86_64-unknown-linux-gnu', archivePlatform: 'linux', archiveArch: 'x86_64', binName: 'tfy' }],
  ['linux:arm64', { platform: 'linux', arch: 'arm64', rustTarget: 'aarch64-unknown-linux-gnu', archivePlatform: 'linux', archiveArch: 'arm64', binName: 'tfy' }],
  ['win32:x64', { platform: 'win32', arch: 'x64', rustTarget: 'x86_64-pc-windows-msvc', archivePlatform: 'windows', archiveArch: 'x86_64', binName: 'tfy.exe' }]
]);

function cloneTarget(key, target) {
  return { ...target, key };
}

function currentPlatform() {
  return resolvePlatform(process.platform, process.arch);
}

function resolvePlatform(platform, arch) {
  const key = `${platform}:${arch}`;
  const resolved = SUPPORTED.get(key);
  if (!resolved) {
    const supported = Array.from(SUPPORTED.keys()).join(', ');
    throw new Error(`Unsupported TFY npm platform ${key}. Supported: ${supported}`);
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
  resolvePlatform,
  resolveRustTarget,
  supportedTargets
};
