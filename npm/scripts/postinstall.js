#!/usr/bin/env node

const {
  chmodSync,
  existsSync,
  linkSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  unlinkSync,
  writeFileSync,
} = require('fs');
const { basename, dirname, isAbsolute, join } = require('path');
const { platform, arch } = require('os');
const { execFileSync } = require('child_process');
const { createHash } = require('crypto');
const {
  MACOS_HELPER_NAME,
  helperNames,
  releasedKeys,
  resolve,
  tarCommand,
  tarballName,
} = require('../lib/platform');

const projectRoot = join(__dirname, '..');
const binDir = join(projectRoot, 'bin');
const packageJson = JSON.parse(readFileSync(join(projectRoot, 'package.json'), 'utf8'));
const version = packageJson.version;

const GITHUB_REPO = 'lahfir/agent-desktop';
const MACOS_HELPER_PATH_ENV = 'AGENT_DESKTOP_MACOS_HELPER_PATH';

function log(msg) {
  process.stderr.write(`agent-desktop: ${msg}\n`);
}

function cleanupStaging(path) {
  try {
    rmSync(path, { recursive: true, force: true });
  } catch (err) {
    log(`Could not remove staging directory; retained at ${path}: ${err.message}`);
  }
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
    const digest = sha256(source);
    if (digest !== sha256(temporary)) {
      throw new Error(`Executable copy verification failed for ${destination}`);
    }
    renameSync(temporary, destination);
    writeFileSync(`${destination}.sha256`, `${digest}\n`);
  } finally {
    try { unlinkSync(temporary); } catch {}
  }
}

// Verify executable hasn't been tampered with; missing digest means re-download once.
function matchesRecordedDigest(destination) {
  const sidecar = `${destination}.sha256`;
  if (!existsSync(sidecar)) {
    return false;
  }
  try {
    return readFileSync(sidecar, 'utf8').trim() === sha256(destination);
  } catch {
    return false;
  }
}

function customHelperPath(customBinaryPath, helperName = MACOS_HELPER_NAME) {
  const override = helperName === MACOS_HELPER_NAME ? process.env[MACOS_HELPER_PATH_ENV] : undefined;
  if (override) {
    if (!isAbsolute(override)) {
      throw new Error(`${MACOS_HELPER_PATH_ENV} must be an absolute path`);
    }
    return override;
  }
  return join(dirname(customBinaryPath), helperName);
}

function validateArchive(tarballPath, expectedEntries) {
  const listing = execFileSync(tarCommand(platform(), process.env), ['-tzf', tarballPath], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
    timeout: 30000,
  })
    .split(/\r?\n/)
    .filter(Boolean)
    .sort();
  const expected = [...expectedEntries].sort();
  if (JSON.stringify(listing) !== JSON.stringify(expected)) {
    throw new Error(`Release archive has unexpected entries: ${listing.join(', ')}`);
  }
}

function installArchive(tarballPath, binaryPath, helperPath, entry) {
  const [helperName] = helperNames(entry);
  validateArchive(tarballPath, entry.entries);
  const staging = mkdtempSync(join(binDir, '.extract-'));
  try {
    execFileSync(tarCommand(platform(), process.env), ['-xzf', tarballPath, '-C', staging], {
      stdio: 'pipe',
      timeout: 30000,
    });
    const entries = readdirSync(staging).sort();
    const expected = [...entry.entries].sort();
    if (JSON.stringify(entries) !== JSON.stringify(expected)) {
      throw new Error(`Extracted archive has unexpected entries: ${entries.join(', ')}`);
    }
    const extractedBinary = join(staging, entry.entries[0]);
    if (!lstatSync(extractedBinary).isFile()) {
      throw new Error('Release archive executables must be regular files');
    }
    if (helperName) {
      const extractedHelper = join(staging, helperName);
      if (!lstatSync(extractedHelper).isFile()) {
        throw new Error('Release archive executables must be regular files');
      }
      installExecutable(extractedHelper, helperPath);
    }
    installExecutable(extractedBinary, binaryPath);
  } finally {
    cleanupStaging(staging);
  }
}

const BIN_ENTRY_NAME = 'agent-desktop';

function isInstalledPackage(directory) {
  return basename(dirname(dirname(directory))) === 'node_modules';
}

function isNodeWrapper(path) {
  try {
    return readFileSync(path).subarray(0, 2).toString() === '#!';
  } catch {
    return false;
  }
}

function linkBinEntry(directory, binaryName, osPlatform = platform()) {
  if (osPlatform === 'win32' || !isInstalledPackage(directory)) return false;
  const entryPath = join(directory, BIN_ENTRY_NAME);
  const binaryPath = join(directory, binaryName);
  const temporary = `${entryPath}.link-${process.pid}`;
  try {
    const binaryStat = lstatSync(binaryPath);
    try {
      const entryStat = lstatSync(entryPath);
      if (entryStat.ino === binaryStat.ino && entryStat.dev === binaryStat.dev) return false;
    } catch {}
    rmSync(temporary, { force: true });
    linkSync(binaryPath, temporary);
    renameSync(temporary, entryPath);
    log('Linked bin entry to native binary');
    return true;
  } catch (err) {
    if (isNodeWrapper(entryPath)) {
      log(`Could not link bin entry, keeping the Node wrapper: ${err.message}`);
    } else {
      log(`Could not link bin entry to the installed native binary: ${err.message}`);
      process.exitCode = 1;
    }
    return false;
  } finally {
    try { unlinkSync(temporary); } catch {}
  }
}

function promptSkillInstall() {
  const platformSkill = {
    darwin: 'agent-desktop-macos',
    win32: 'agent-desktop-windows',
    linux: 'agent-desktop-linux',
  }[platform()];
  const skills = ['agent-desktop', 'agent-desktop-ffi'];
  if (platformSkill) skills.push(platformSkill);

  log('');
  log('Claude Code skills available for agent-desktop!');
  log('The binary serves its own skills:');
  log('  agent-desktop skills get desktop');
  log('  agent-desktop skills get platform');
  log('Or install a skill with:');
  for (const skill of skills) {
    log(`  npx skills add https://github.com/lahfir/agent-desktop --skill ${skill}`);
  }
  log('');
}

function printManualFallback(tarballUrl, checksumsUrl, entry) {
  log('');
  log('Download manually from:');
  log(`  ${tarballUrl}`);
  const [helperName] = helperNames(entry);
  log('Then extract the archive so the installed file(s) are at:');
  log(`  ${join(binDir, entry.binaryName)}`);
  if (helperName) {
    log(`  ${join(binDir, helperName)}`);
  }
  log('');
  log('Verify the download before running it:');
  log(`  curl -fsSL ${checksumsUrl}`);
  log(`  sha256sum <downloaded-archive>   # compare with the matching checksums.txt line`);
  log(`  gh attestation verify <downloaded-archive> --repo ${GITHUB_REPO}`);
}

function main() {
  if (process.env.AGENT_DESKTOP_SKIP_DOWNLOAD === '1') {
    log('Skipping binary download (AGENT_DESKTOP_SKIP_DOWNLOAD=1)');
    return;
  }

  const platformKey = getPlatformKey();
  const entry = resolve(platform(), arch());

  if (!entry || !entry.released) {
    log(`agent-desktop has no released native binary for ${platformKey}.`);
    log(`Released platform keys today: ${releasedKeys().join(', ')}.`);
    log(`See: https://github.com/${GITHUB_REPO}`);
    return;
  }

  const [helperName] = helperNames(entry);
  const includesHelper = Boolean(helperName);
  const binaryName = entry.binaryName;
  const binaryPath = join(binDir, binaryName);
  const helperPath = includesHelper ? join(binDir, helperName) : undefined;

  if (!existsSync(binDir)) {
    mkdirSync(binDir, { recursive: true });
  }

  if (process.env.AGENT_DESKTOP_BINARY_PATH) {
    const customPath = process.env.AGENT_DESKTOP_BINARY_PATH;
    try {
      if (!existsSync(customPath) || !lstatSync(customPath).isFile()) {
        throw new Error(`binary is not a regular file: ${customPath}`);
      }
      if (includesHelper) {
        const sourceHelper = customHelperPath(customPath, helperName);
        if (!existsSync(sourceHelper) || !lstatSync(sourceHelper).isFile()) {
          throw new Error(`${helperName} not found at ${sourceHelper}`);
        }
        installExecutable(sourceHelper, helperPath);
      }
      installExecutable(customPath, binaryPath);
      log(`Using binary from AGENT_DESKTOP_BINARY_PATH: ${customPath}`);
      linkBinEntry(binDir, binaryName);
      promptSkillInstall();
    } catch (err) {
      log(`Failed to install from AGENT_DESKTOP_BINARY_PATH: ${err.message}`);
      process.exitCode = 1;
    }
    return;
  }

  const alreadyInstalled =
    existsSync(binaryPath) &&
    matchesRecordedDigest(binaryPath) &&
    (!includesHelper || (existsSync(helperPath) && matchesRecordedDigest(helperPath)));

  if (alreadyInstalled) {
    chmodSync(binaryPath, 0o755);
    if (includesHelper) chmodSync(helperPath, 0o755);
    log(
      includesHelper
        ? `Native executables ready: ${binaryName}, ${helperName}`
        : `Native executable ready: ${binaryName}`,
    );
    linkBinEntry(binDir, binaryName);
    promptSkillInstall();
    return;
  }

  const tarball = tarballName(version, entry.target);
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

    installArchive(tarballPath, binaryPath, helperPath, entry);
    unlinkSync(tarballPath);
    log(
      includesHelper
        ? `Installed native executables: ${binaryName}, ${helperName}`
        : `Installed native executable: ${binaryName}`,
    );
  } catch (err) {
    log(`Could not download native binary: ${err.message}`);
    printManualFallback(tarballUrl, checksumsUrl, entry);

    try { if (existsSync(tarballPath)) unlinkSync(tarballPath); } catch {}
    try { if (existsSync(checksumsPath)) unlinkSync(checksumsPath); } catch {}
    process.exitCode = 1;
    return;
  }

  linkBinEntry(binDir, binaryName);

  promptSkillInstall();
}

if (require.main === module) {
  main();
}

module.exports = {
  checksumFor,
  cleanupStaging,
  customHelperPath,
  installArchive,
  linkBinEntry,
  matchesRecordedDigest,
  printManualFallback,
  promptSkillInstall,
  validateArchive,
};
