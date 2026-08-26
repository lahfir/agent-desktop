const { spawn } = require('node:child_process');
const { mkdtempSync, unlinkSync, writeFileSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');

const postinstallScriptPath = join(__dirname, '..', '..', 'npm', 'scripts', 'postinstall.js');
const wrapperScriptPath = join(__dirname, '..', '..', 'npm', 'bin', 'agent-desktop.js');

function writeOsStub(osPlatform, osArch) {
  const known = /^[a-zA-Z0-9]+$/;
  if (!known.test(osPlatform) || !known.test(osArch)) {
    throw new Error(`unsupported os stub identity: ${osPlatform}-${osArch}`);
  }
  const stub = join(tmpdir(), `agent-desktop-os-stub-${process.pid}-${osPlatform}-${osArch}.js`);
  writeFileSync(
    stub,
    [
      "const os = require('os');",
      `os.platform = () => ${JSON.stringify(osPlatform)};`,
      `os.arch = () => ${JSON.stringify(osArch)};`,
      '',
    ].join('\n'),
  );
  return stub;
}

function runScriptWithOsStub(scriptPath, osPlatform, osArch, args = []) {
  const stub = writeOsStub(osPlatform, osArch);
  const env = { ...process.env };
  delete env.AGENT_DESKTOP_SKIP_DOWNLOAD;
  return new Promise((settle) => {
    let settled = false;
    let stderr = '';
    const child = spawn(process.execPath, ['-r', stub, scriptPath, ...args], {
      stdio: ['ignore', 'pipe', 'pipe'],
      env,
    });
    child.stderr.on('data', (chunk) => {
      stderr += String(chunk);
    });
    const finish = (code) => {
      if (settled) return;
      settled = true;
      try { unlinkSync(stub); } catch {}
      settle({ code, stderr });
    };
    child.on('close', finish);
    child.on('error', () => finish(-1));
  });
}

// Both suites create scratch directories and sweep them in afterEach. They
// keep separate collections because their teardown differs in what else it
// removes, so the factory hands each file its own rather than sharing one.
function createTemporaryRoots() {
  const roots = [];
  return {
    temporaryDirectory() {
      const root = mkdtempSync(join(tmpdir(), 'agent-desktop-npm-test-'));
      roots.push(root);
      return root;
    },
    trackRoot(root) {
      roots.push(root);
      return root;
    },
    drainRoots() {
      return roots.splice(0);
    },
  };
}

module.exports = {
  createTemporaryRoots,
  postinstallScriptPath,
  runScriptWithOsStub,
  wrapperScriptPath,
  writeOsStub,
};
