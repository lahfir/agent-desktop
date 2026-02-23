#!/usr/bin/env node

const { existsSync, mkdirSync, chmodSync, unlinkSync, renameSync, writeFileSync, symlinkSync, lstatSync } = require('fs');
const { readFileSync } = require('fs');
const { join } = require('path');
const { platform, arch } = require('os');
const { execSync } = require('child_process');
const { createHash } = require('crypto');

const projectRoot = join(__dirname, '..');
const binDir = join(projectRoot, 'bin');
const packageJson = JSON.parse(readFileSync(join(projectRoot, 'package.json'), 'utf8'));
const version = packageJson.version;

const GITHUB_REPO = 'lahfir/agent-desktop';

const TARGET_MAP = {
  'darwin-arm64': 'aarch64-apple-darwin',
  'darwin-x64': 'x86_64-apple-darwin',
  'linux-x64': 'x86_64-unknown-linux-gnu',
  'linux-arm64': 'aarch64-unknown-linux-gnu',
  'win32-x64': 'x86_64-pc-windows-msvc',
};

const BINARY_NAME_MAP = {
  'darwin-arm64': 'agent-desktop-darwin-arm64',
  'darwin-x64': 'agent-desktop-darwin-x64',
  'linux-x64': 'agent-desktop-linux-x64',
  'linux-arm64': 'agent-desktop-linux-arm64',
  'win32-x64': 'agent-desktop-win32-x64.exe',
};

const SUPPORTED_PLATFORMS = ['darwin'];

function log(msg) {
  process.stderr.write(`agent-desktop: ${msg}\n`);
}

function getPlatformKey() {
  return `${platform()}-${arch()}`;
}

function download(url, dest) {
  const tmpDest = dest + '.tmp';
  try {
    execSync(`curl -fsSL --retry 3 --retry-delay 2 -o "${tmpDest}" "${url}"`, {
      stdio: 'pipe',
      timeout: 60000,
    });
    renameSync(tmpDest, dest);
  } catch (err) {
    try { unlinkSync(tmpDest); } catch {}
    throw new Error(`Failed to download ${url}: ${err.message}`);
  }
}

function verifyChecksum(filePath, expectedHash) {
  const fileBuffer = readFileSync(filePath);
  const hash = createHash('sha256').update(fileBuffer).digest('hex');
  return hash === expectedHash;
}

function fixGlobalInstallBin() {
  if (platform() === 'win32') return;

  let npmBinDir;
  try {
    const prefix = execSync('npm prefix -g', { encoding: 'utf8', timeout: 5000 }).trim();
    npmBinDir = join(prefix, 'bin');
  } catch {
    return;
  }

  const symlinkPath = join(npmBinDir, 'agent-desktop');
  const platformKey = getPlatformKey();
  const binaryName = BINARY_NAME_MAP[platformKey];
  if (!binaryName) return;

  const binaryPath = join(binDir, binaryName);

  try {
    const stat = lstatSync(symlinkPath);
    if (!stat.isSymbolicLink()) return;
  } catch {
    return;
  }

  try {
    unlinkSync(symlinkPath);
    symlinkSync(binaryPath, symlinkPath);
    log('Optimized: symlink points to native binary (zero overhead)');
  } catch (err) {
    log(`Could not optimize symlink: ${err.message}`);
  }
}

function main() {
  if (process.env.AGENT_DESKTOP_SKIP_DOWNLOAD === '1') {
    log('Skipping binary download (AGENT_DESKTOP_SKIP_DOWNLOAD=1)');
    return;
  }

  const platformKey = getPlatformKey();
  const target = TARGET_MAP[platformKey];
  const binaryName = BINARY_NAME_MAP[platformKey];

  if (!SUPPORTED_PLATFORMS.includes(platform()) || !target || !binaryName) {
    log('agent-desktop currently supports macOS only.');
    log('Windows and Linux support is coming in Phase 2.');
    log(`See: https://github.com/${GITHUB_REPO}`);
    return;
  }

  const binaryPath = join(binDir, binaryName);

  if (process.env.AGENT_DESKTOP_BINARY_PATH) {
    const customPath = process.env.AGENT_DESKTOP_BINARY_PATH;
    if (existsSync(customPath)) {
      try {
        writeFileSync(binaryPath, readFileSync(customPath));
        chmodSync(binaryPath, 0o755);
        log(`Using binary from AGENT_DESKTOP_BINARY_PATH: ${customPath}`);
        fixGlobalInstallBin();
        return;
      } catch (err) {
        log(`Failed to copy from AGENT_DESKTOP_BINARY_PATH: ${err.message}`);
      }
    }
  }

  if (existsSync(binaryPath)) {
    chmodSync(binaryPath, 0o755);
    log(`Native binary ready: ${binaryName}`);
    fixGlobalInstallBin();
    return;
  }

  if (!existsSync(binDir)) {
    mkdirSync(binDir, { recursive: true });
  }

  const tarball = `agent-desktop-v${version}-${target}.tar.gz`;
  const baseUrl = `https://github.com/${GITHUB_REPO}/releases/download/v${version}`;
  const tarballUrl = `${baseUrl}/${tarball}`;
  const checksumsUrl = `${baseUrl}/checksums.txt`;
  const tarballPath = join(binDir, tarball);
  const checksumsPath = join(binDir, 'checksums.txt');

  log(`Downloading native binary for ${platformKey}...`);

  try {
    download(tarballUrl, tarballPath);

    try {
      download(checksumsUrl, checksumsPath);
      const checksums = readFileSync(checksumsPath, 'utf8');
      const expectedLine = checksums.split('\n').find((line) => line.includes(tarball));
      if (expectedLine) {
        const expectedHash = expectedLine.split(/\s+/)[0];
        if (!verifyChecksum(tarballPath, expectedHash)) {
          log('WARNING: Checksum verification failed.');
          unlinkSync(tarballPath);
          unlinkSync(checksumsPath);
          return;
        }
        log('Checksum verified');
      }
      unlinkSync(checksumsPath);
    } catch {
      log('Checksum verification skipped');
    }

    execSync(`tar -xzf "${tarballPath}" -C "${binDir}"`, { stdio: 'pipe' });

    const extractedBinary = join(binDir, 'agent-desktop');
    if (existsSync(extractedBinary) && extractedBinary !== binaryPath) {
      renameSync(extractedBinary, binaryPath);
    }

    chmodSync(binaryPath, 0o755);
    unlinkSync(tarballPath);
    log(`Installed native binary: ${binaryName}`);
  } catch (err) {
    log(`Could not download native binary: ${err.message}`);
    log('');
    log('Download manually from:');
    log(`  ${tarballUrl}`);
    log(`Then place at: ${binaryPath}`);

    try { if (existsSync(tarballPath)) unlinkSync(tarballPath); } catch {}
    try { if (existsSync(checksumsPath)) unlinkSync(checksumsPath); } catch {}
    return;
  }

  fixGlobalInstallBin();
}

main();
