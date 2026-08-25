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
const { MACOS_HELPER_NAME, releasedKeys, resolve, tarballName } = require('../lib/platform');

const projectRoot = join(__dirname, '..');
const binDir = join(projectRoot, 'bin');
const packageJson = JSON.parse(readFileSync(join(projectRoot, 'package.json'), 'utf8'));
const version = packageJson.version;

const GITHUB_REPO = 'lahfir/agent-desktop';
const MACOS_HELPER_PATH_ENV = 'AGENT_DESKTOP_MACOS_HELPER_PATH';

function log(msg) {
  process.stderr.write(`agent-desktop: ${msg}\n`);
}

function trashRecoverably(path, trashCommand = 'trash') {
  try {
    execFileSync(trashCommand, [path], { stdio: 'pipe', timeout: 30000 });
  } catch (err) {
    if (!existsSync(path)) return;
    const reason = err.code === 'ENOENT'
      ? `trash command is unavailable: ${trashCommand}`
      : `trash exited with status ${err.status ?? 'unknown'}`;
    log(`Could not move cleanup artifact to Trash; retained at ${path}: ${reason}`);
  }
}

function cleanupStaging(path, trashCommand) {
  if (platform() === 'win32') {
    rmSync(path, { recursive: true, force: true });
    return;
  }
  trashRecoverably(path, trashCommand);
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

function validateArchive(tarballPath, expectedEntries) {
  const listing = execFileSync('tar', ['-tzf', tarballPath], {
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

function installArchive(tarballPath, binaryPath, helperPath, entry, trashCommand) {
  const includesHelper = entry.entries.includes(MACOS_HELPER_NAME);
  validateArchive(tarballPath, entry.entries);
  const staging = mkdtempSync(join(binDir, '.extract-'));
  try {
    execFileSync('tar', ['-xzf', tarballPath, '-C', staging], {
      stdio: 'pipe',
      timeout: 30000,
    });
    const entries = readdirSync(staging).sort();
    const expected = [...entry.entries].sort();
    if (JSON.stringify(entries) !== JSON.stringify(expected)) {
      throw new Error(`Extracted archive has unexpected entries: ${entries.join(', ')}`);
    }
    const binaryMember = entry.entries.find((member) => member !== MACOS_HELPER_NAME);
    const extractedBinary = join(staging, binaryMember);
    if (!lstatSync(extractedBinary).isFile()) {
      throw new Error('Release archive executables must be regular files');
    }
    if (includesHelper) {
      const extractedHelper = join(staging, MACOS_HELPER_NAME);
      if (!lstatSync(extractedHelper).isFile()) {
        throw new Error('Release archive executables must be regular files');
      }
      installExecutable(extractedHelper, helperPath);
    }
    installExecutable(extractedBinary, binaryPath);
  } finally {
    cleanupStaging(staging, trashCommand);
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
  const entry = resolve(platform(), arch());
  if (!entry) return;

  const binaryPath = join(binDir, entry.binaryName);

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
    win32: 'agent-desktop-windows',
  }[platform()];
  const skills = ['agent-desktop', 'agent-desktop-ffi'];
  if (platformSkill) skills.push(platformSkill);

  log('');
  log('Claude Code skills available for agent-desktop!');
  log('Install with:');
  for (const skill of skills) {
    log(`  claude mcp add-skill lahfir/${skill}`);
  }
  log('');
}

function printManualFallback(tarballUrl, checksumsUrl, entry) {
  log('');
  log('Download manually from:');
  log(`  ${tarballUrl}`);
  log('Then place the archive member(s) at:');
  for (const member of entry.entries) {
    log(`  ${join(binDir, member)}`);
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

  const includesHelper = entry.entries.includes(MACOS_HELPER_NAME);
  const binaryName = entry.binaryName;
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
      if (includesHelper) {
        const sourceHelper = customHelperPath(customPath);
        if (!existsSync(sourceHelper) || !lstatSync(sourceHelper).isFile()) {
          throw new Error(`macOS helper not found at ${sourceHelper}`);
        }
        installExecutable(sourceHelper, helperPath);
      }
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

  if (existsSync(binaryPath) && (!includesHelper || existsSync(helperPath))) {
    chmodSync(binaryPath, 0o755);
    if (includesHelper) chmodSync(helperPath, 0o755);
    log(
      includesHelper
        ? `Native executables ready: ${binaryName}, ${MACOS_HELPER_NAME}`
        : `Native executable ready: ${binaryName}`,
    );
    fixGlobalInstallBin();
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
        ? `Installed native executables: ${binaryName}, ${MACOS_HELPER_NAME}`
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

  fixGlobalInstallBin();

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
  printManualFallback,
  promptSkillInstall,
  trashRecoverably,
  validateArchive,
};
