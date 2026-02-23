#!/usr/bin/env node

const { existsSync, mkdirSync, chmodSync, createWriteStream, unlinkSync, renameSync, writeFileSync, symlinkSync, lstatSync } = require('fs');
const { readFileSync } = require('fs');
const { dirname, join } = require('path');
const { platform, arch } = require('os');
const { get } = require('https');
const { execSync } = require('child_process');
const { createHash } = require('crypto');

const projectRoot = join(__dirname, '..');
const binDir = join(projectRoot, 'bin');
const packageJson = JSON.parse(readFileSync(join(projectRoot, 'package.json'), 'utf8'));
const version = packageJson.version;

const GITHUB_REPO = 'lahfir/agent-desktop';
const MAX_RETRIES = 3;
const TIMEOUT_MS = 60000;

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

function downloadWithRedirects(url, dest, timeout) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new Error(`Download timed out after ${timeout}ms`));
    }, timeout);

    const doRequest = (requestUrl) => {
      const parsedUrl = new URL(requestUrl);
      const options = {
        hostname: parsedUrl.hostname,
        path: parsedUrl.pathname + parsedUrl.search,
        headers: { 'User-Agent': `agent-desktop/${version}` },
      };

      const proxy = process.env.HTTPS_PROXY || process.env.https_proxy || process.env.HTTP_PROXY || process.env.http_proxy;
      if (proxy) {
        log(`Using proxy: ${proxy}`);
      }

      get(requestUrl, options, (response) => {
        if (response.statusCode === 301 || response.statusCode === 302) {
          doRequest(response.headers.location);
          return;
        }

        if (response.statusCode !== 200) {
          clearTimeout(timer);
          reject(new Error(`HTTP ${response.statusCode} downloading ${requestUrl}`));
          return;
        }

        const tmpDest = dest + '.tmp';
        const file = createWriteStream(tmpDest);
        response.pipe(file);
        file.on('finish', () => {
          file.close();
          clearTimeout(timer);
          try {
            renameSync(tmpDest, dest);
            resolve();
          } catch (err) {
            reject(err);
          }
        });
      }).on('error', (err) => {
        clearTimeout(timer);
        reject(err);
      });
    };

    doRequest(url);
  });
}

async function downloadWithRetry(url, dest) {
  for (let attempt = 1; attempt <= MAX_RETRIES; attempt++) {
    try {
      await downloadWithRedirects(url, dest, TIMEOUT_MS);
      return;
    } catch (err) {
      if (attempt === MAX_RETRIES) throw err;
      const delay = Math.pow(2, attempt) * 1000;
      log(`Download failed (attempt ${attempt}/${MAX_RETRIES}): ${err.message}`);
      log(`Retrying in ${delay / 1000}s...`);
      await new Promise((r) => setTimeout(r, delay));
    }
  }
}

function readFileContent(path) {
  return readFileSync(path, 'utf8');
}

function verifyChecksum(filePath, expectedHash) {
  const fileBuffer = readFileSync(filePath);
  const hash = createHash('sha256').update(fileBuffer).digest('hex');
  return hash === expectedHash;
}

async function fixGlobalInstallBin() {
  if (platform() === 'win32') return;

  let npmBinDir;
  try {
    const prefix = execSync('npm prefix -g', { encoding: 'utf8' }).trim();
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
    log('CLI will work via Node.js wrapper (slightly slower startup)');
  }
}

async function main() {
  if (process.env.AGENT_DESKTOP_SKIP_DOWNLOAD === '1') {
    log('Skipping binary download (AGENT_DESKTOP_SKIP_DOWNLOAD=1)');
    return;
  }

  const platformKey = getPlatformKey();
  const target = TARGET_MAP[platformKey];
  const binaryName = BINARY_NAME_MAP[platformKey];

  if (!target || !binaryName) {
    if (!SUPPORTED_PLATFORMS.includes(platform())) {
      log(`agent-desktop currently supports macOS only.`);
      log(`Windows and Linux support is coming in Phase 2.`);
      log(`See: https://github.com/${GITHUB_REPO}`);
      return;
    }
    log(`Unsupported architecture: ${platformKey}`);
    return;
  }

  if (!SUPPORTED_PLATFORMS.includes(platform())) {
    log(`agent-desktop currently supports macOS only.`);
    log(`${platform()} support is coming in Phase 2.`);
    log(`See: https://github.com/${GITHUB_REPO}`);
    return;
  }

  const binaryPath = join(binDir, binaryName);

  if (process.env.AGENT_DESKTOP_BINARY_PATH) {
    const customPath = process.env.AGENT_DESKTOP_BINARY_PATH;
    if (existsSync(customPath)) {
      try {
        const content = readFileSync(customPath);
        writeFileSync(binaryPath, content);
        chmodSync(binaryPath, 0o755);
        log(`Using binary from AGENT_DESKTOP_BINARY_PATH: ${customPath}`);
        await fixGlobalInstallBin();
        return;
      } catch (err) {
        log(`Failed to copy from AGENT_DESKTOP_BINARY_PATH: ${err.message}`);
      }
    } else {
      log(`AGENT_DESKTOP_BINARY_PATH not found: ${customPath}`);
    }
  }

  if (existsSync(binaryPath)) {
    if (platform() !== 'win32') {
      chmodSync(binaryPath, 0o755);
    }
    log(`Native binary ready: ${binaryName}`);
    await fixGlobalInstallBin();
    return;
  }

  if (!existsSync(binDir)) {
    mkdirSync(binDir, { recursive: true });
  }

  const tarball = `agent-desktop-v${version}-${target}.tar.gz`;
  const tarballUrl = `https://github.com/${GITHUB_REPO}/releases/download/v${version}/${tarball}`;
  const checksumsUrl = `https://github.com/${GITHUB_REPO}/releases/download/v${version}/checksums.txt`;
  const tarballPath = join(binDir, tarball);
  const checksumsPath = join(binDir, 'checksums.txt');

  log(`Downloading native binary for ${platformKey}...`);

  try {
    await downloadWithRetry(tarballUrl, tarballPath);
    log(`Downloaded: ${tarball}`);

    try {
      await downloadWithRetry(checksumsUrl, checksumsPath);
      const checksums = readFileContent(checksumsPath);
      const expectedLine = checksums.split('\n').find((line) => line.includes(tarball));
      if (expectedLine) {
        const expectedHash = expectedLine.split(/\s+/)[0];
        if (!verifyChecksum(tarballPath, expectedHash)) {
          log('WARNING: Checksum verification failed. Binary may be corrupted.');
          log('Try reinstalling: npm install -g agent-desktop');
          unlinkSync(tarballPath);
          unlinkSync(checksumsPath);
          return;
        }
        log('Checksum verified');
      }
      unlinkSync(checksumsPath);
    } catch (err) {
      log(`Could not verify checksum: ${err.message}`);
    }

    execSync(`tar -xzf "${tarballPath}" -C "${binDir}"`, { stdio: 'pipe' });

    const extractedBinary = join(binDir, 'agent-desktop');
    if (existsSync(extractedBinary) && extractedBinary !== binaryPath) {
      renameSync(extractedBinary, binaryPath);
    }

    if (platform() !== 'win32') {
      chmodSync(binaryPath, 0o755);
    }

    unlinkSync(tarballPath);
    log(`Installed native binary: ${binaryName}`);
  } catch (err) {
    log(`Could not download native binary: ${err.message}`);
    log('');
    log('You can download manually from:');
    log(`  ${tarballUrl}`);
    log('');
    log(`Then place the binary at: ${binaryPath}`);

    try {
      if (existsSync(tarballPath)) unlinkSync(tarballPath);
      if (existsSync(checksumsPath)) unlinkSync(checksumsPath);
    } catch {}

    return;
  }

  await fixGlobalInstallBin();
}

main().catch((err) => {
  log(`Postinstall error: ${err.message}`);
  process.exit(0);
});
