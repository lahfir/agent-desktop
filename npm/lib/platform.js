const MACOS_HELPER_NAME = 'agent-desktop-macos-helper';

const PLATFORMS = {
  'darwin-arm64': {
    target: 'aarch64-apple-darwin',
    binaryName: 'agent-desktop-darwin-arm64',
    entries: ['agent-desktop', MACOS_HELPER_NAME],
    released: true,
  },
  'darwin-x64': {
    target: 'x86_64-apple-darwin',
    binaryName: 'agent-desktop-darwin-x64',
    entries: ['agent-desktop', MACOS_HELPER_NAME],
    released: true,
  },
  'linux-arm64': {
    target: 'aarch64-unknown-linux-gnu',
    binaryName: 'agent-desktop-linux-arm64',
    entries: ['agent-desktop'],
    released: false,
  },
  'linux-x64': {
    target: 'x86_64-unknown-linux-gnu',
    binaryName: 'agent-desktop-linux-x64',
    entries: ['agent-desktop'],
    released: false,
  },
  'win32-arm64': {
    target: 'aarch64-pc-windows-msvc',
    binaryName: 'agent-desktop-win32-arm64.exe',
    entries: ['agent-desktop.exe'],
    released: true,
  },
  'win32-x64': {
    target: 'x86_64-pc-windows-msvc',
    binaryName: 'agent-desktop-win32-x64.exe',
    entries: ['agent-desktop.exe'],
    released: true,
  },
};

function resolve(osPlatform, osArch) {
  return PLATFORMS[`${osPlatform}-${osArch}`];
}

function tarballName(version, target) {
  return `agent-desktop-v${version}-${target}.tar.gz`;
}

module.exports = {
  PLATFORMS,
  resolve,
  tarballName,
};
