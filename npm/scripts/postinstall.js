#!/usr/bin/env node

const {
  chmodSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  symlinkSync,
  unlinkSync,
  writeFileSync,
} = require('fs');
const { dirname, isAbsolute, join } = require('path');
const { platform, arch } = require('os');
const { execFileSync } = require('child_process');
const { createHash } = require('crypto');

const projectRoot = join(__dirname, '..');
const binDir = join(projectRoot, 'bin');
const packageJson = JSON.parse(readFileSync(join(projectRoot, 'package.json'), 'utf8'));
const version = packageJson.version;

const GITHUB_REPO = 'lahfir/agent-desktop';
const MACOS_HELPER_NAME = 'agent-desktop-macos-helper';
const MACOS_HELPER_PATH_ENV = 'AGENT_DESKTOP_MACOS_HELPER_PATH';

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
    execFileSync('curl', ['-fsSL', '--retry', '3', '--retry-delay', '2', '-o', tmpDest, url], {
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
  return sha256(filePath) === expectedHash.toLowerCase();
}

function sha256(filePath) {
  return createHash('sha256').update(readFileSync(filePath)).digest('hex');
}

function checksumFor(contents, fileName) {
  for (const line of contents.split('\n')) {
    const match = line.trim().match(/^([0-9a-fA-F]{64})\s+\*?(.+)$/);
    if (match && match[2] === fileName) {
      return match[1].toLowerCase();
    }
  }
  throw new Error(`Checksum entry missing for ${fileName}`);
}

function installExecutable(source, destination) {
  const temporary = `${destination}.install-${process.pid}`;
  try {
    writeFileSync(temporary, readFileSync(source), { mode: 0o755 });
    chmodSync(temporary, 0o755);
    if (sha256(source) !== sha256(temporary)) {
      throw new Error(`Executable copy verification failed for ${destination}`);
    }
    renameSync(temporary, destination);
  } finally {
    try { unlinkSync(temporary); } catch {}
  }
}

function customHelperPath(customBinaryPath) {
  const override = process.env[MACOS_HELPER_PATH_ENV];
  if (override) {
    if (!isAbsolute(override)) {
      throw new Error(`${MACOS_HELPER_PATH_ENV} must be an absolute path`);
    }
    return override;
  }
  return join(dirname(customBinaryPath), MACOS_HELPER_NAME);
}

function validateArchive(tarballPath) {
  const listing = execFileSync('tar', ['-tzf', tarballPath], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
    timeout: 30000,
  })
    .split('\n')
    .filter(Boolean)
    .sort();
  const expected = ['agent-desktop', MACOS_HELPER_NAME].sort();
  if (JSON.stringify(listing) !== JSON.stringify(expected)) {
    throw new Error(`Release archive has unexpected entries: ${listing.join(', ')}`);
  }
}

function installArchive(tarballPath, binaryPath, helperPath) {
  validateArchive(tarballPath);
  const staging = mkdtempSync(join(binDir, '.extract-'));
  try {
    execFileSync('tar', ['-xzf', tarballPath, '-C', staging], {
      stdio: 'pipe',
      timeout: 30000,
    });
    const entries = readdirSync(staging).sort();
    const expected = ['agent-desktop', MACOS_HELPER_NAME].sort();
    if (JSON.stringify(entries) !== JSON.stringify(expected)) {
      throw new Error(`Extracted archive has unexpected entries: ${entries.join(', ')}`);
    }
    const extractedBinary = join(staging, 'agent-desktop');
    const extractedHelper = join(staging, MACOS_HELPER_NAME);
    if (!lstatSync(extractedBinary).isFile() || !lstatSync(extractedHelper).isFile()) {
      throw new Error('Release archive executables must be regular files');
    }
    installExecutable(extractedHelper, helperPath);
    installExecutable(extractedBinary, binaryPath);
  } finally {
    rmSync(staging, { recursive: true, force: true });
  }
}

function fixGlobalInstallBin() {
  if (platform() === 'win32') return;

  let npmBinDir;
  try {
    const prefix = execFileSync('npm', ['prefix', '-g'], { encoding: 'utf8', timeout: 5000 }).trim();
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

function promptSkillInstall() {
  const platformSkill = {
    darwin: 'agent-desktop-macos',
    win32: 'agent-desktop-windows',
    linux: 'agent-desktop-linux',
  }[platform()];

  log('');
  log('Claude Code skills available for agent-desktop!');
  log('Install with:');
  log('  claude mcp add-skill lahfir/agent-desktop');
  if (platformSkill) {
    log(`  claude mcp add-skill lahfir/${platformSkill}`);
  }
  log('');
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
  const helperPath = join(binDir, MACOS_HELPER_NAME);

  if (!existsSync(binDir)) {
    mkdirSync(binDir, { recursive: true });
  }

  if (process.env.AGENT_DESKTOP_BINARY_PATH) {
    const customPath = process.env.AGENT_DESKTOP_BINARY_PATH;
    try {
      if (!existsSync(customPath) || !lstatSync(customPath).isFile()) {
        throw new Error(`binary is not a regular file: ${customPath}`);
      }
      const sourceHelper = customHelperPath(customPath);
      if (!existsSync(sourceHelper) || !lstatSync(sourceHelper).isFile()) {
        throw new Error(`macOS helper not found at ${sourceHelper}`);
      }
      installExecutable(sourceHelper, helperPath);
      installExecutable(customPath, binaryPath);
      log(`Using binary from AGENT_DESKTOP_BINARY_PATH: ${customPath}`);
      fixGlobalInstallBin();
      promptSkillInstall();
    } catch (err) {
      log(`Failed to install from AGENT_DESKTOP_BINARY_PATH: ${err.message}`);
      process.exitCode = 1;
    }
    return;
  }

  if (existsSync(binaryPath) && existsSync(helperPath)) {
    chmodSync(binaryPath, 0o755);
    chmodSync(helperPath, 0o755);
    log(`Native executables ready: ${binaryName}, ${MACOS_HELPER_NAME}`);
    fixGlobalInstallBin();
    promptSkillInstall();
    return;
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
    download(checksumsUrl, checksumsPath);
    const checksums = readFileSync(checksumsPath, 'utf8');
    const expectedHash = checksumFor(checksums, tarball);
    if (!verifyChecksum(tarballPath, expectedHash)) {
      throw new Error('Checksum verification failed');
    }
    unlinkSync(checksumsPath);
    log('Checksum verified');

    installArchive(tarballPath, binaryPath, helperPath);
    unlinkSync(tarballPath);
    log(`Installed native executables: ${binaryName}, ${MACOS_HELPER_NAME}`);
  } catch (err) {
    log(`Could not download native binary: ${err.message}`);
    log('');
    log('Download manually from:');
    log(`  ${tarballUrl}`);
    log(`Then place agent-desktop at: ${binaryPath}`);
    log(`And ${MACOS_HELPER_NAME} at: ${helperPath}`);

    try { if (existsSync(tarballPath)) unlinkSync(tarballPath); } catch {}
    try { if (existsSync(checksumsPath)) unlinkSync(checksumsPath); } catch {}
    process.exitCode = 1;
    return;
  }

  fixGlobalInstallBin();

  promptSkillInstall();
}

if (require.main === module) {
  main();
}

module.exports = {
  checksumFor,
  customHelperPath,
  installArchive,
  validateArchive,
};
