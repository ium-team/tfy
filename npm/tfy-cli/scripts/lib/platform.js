'use strict';

const SUPPORTED = new Map([
  ['darwin:x64', { platform: 'darwin', arch: 'x64', rustTarget: 'x86_64-apple-darwin', archivePlatform: 'darwin', archiveArch: 'x86_64', binName: 'tfy' }],
  ['darwin:arm64', { platform: 'darwin', arch: 'arm64', rustTarget: 'aarch64-apple-darwin', archivePlatform: 'darwin', archiveArch: 'arm64', binName: 'tfy' }],
  ['linux:x64', { platform: 'linux', arch: 'x64', rustTarget: 'x86_64-unknown-linux-gnu', archivePlatform: 'linux', archiveArch: 'x86_64', binName: 'tfy' }],
  ['linux:arm64', { platform: 'linux', arch: 'arm64', rustTarget: 'aarch64-unknown-linux-gnu', archivePlatform: 'linux', archiveArch: 'arm64', binName: 'tfy' }],
  ['win32:x64', { platform: 'win32', arch: 'x64', rustTarget: 'x86_64-pc-windows-msvc', archivePlatform: 'windows', archiveArch: 'x86_64', binName: 'tfy.exe' }]
]);

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
  return { ...resolved, key };
}

function assetBaseName(version, target) {
  const cleanVersion = String(version).replace(/^v/, '').replace(/-preview\.\d+$/, '');
  return `tfy-${cleanVersion}-${target.archivePlatform}-${target.archiveArch}`;
}

function archiveName(version, target) {
  return `${assetBaseName(version, target)}.tar.gz`;
}

module.exports = { SUPPORTED, currentPlatform, resolvePlatform, assetBaseName, archiveName };
