const { spawn } = require('node:child_process');
const { mkdtempSync, unlinkSync, writeFileSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');

const postinstallScriptPath = join(__dirname, '..', '..', 'npm', 'scripts', 'postinstall.js');
const wrapperScriptPath = join(__dirname, '..', '..', 'npm', 'bin', 'agent-desktop.js');

const OS_STUB_PLATFORM_ENV = 'AGENT_DESKTOP_TEST_OS_PLATFORM';
const OS_STUB_ARCH_ENV = 'AGENT_DESKTOP_TEST_OS_ARCH';

// The stub is a constant: it reads the identity it should report from the
// environment rather than having that identity compiled into it. Generating
// the source from the arguments instead - even validated and JSON-encoded -
// makes this a code-construction sink, which is both a real shape to avoid in
// a package that runs on install and one CodeQL flags on sight.
const OS_STUB_SOURCE = [
  "const os = require('os');",
  `const platform = process.env[${JSON.stringify(OS_STUB_PLATFORM_ENV)}];`,
  `const arch = process.env[${JSON.stringify(OS_STUB_ARCH_ENV)}];`,
  'os.platform = () => platform;',
  'os.arch = () => arch;',
  '',
].join('\n');

function writeOsStub(osPlatform, osArch) {
  const known = /^[a-zA-Z0-9]+$/;
  if (!known.test(osPlatform) || !known.test(osArch)) {
    throw new Error(`unsupported os stub identity: ${osPlatform}-${osArch}`);
  }
  const stub = join(tmpdir(), `agent-desktop-os-stub-${process.pid}-${osPlatform}-${osArch}.js`);
  writeFileSync(stub, OS_STUB_SOURCE);
  return stub;
}

// The stub reads its identity from the environment, so any caller that spawns
// the stub itself must carry these alongside it.
function osStubEnv(osPlatform, osArch) {
  return { [OS_STUB_PLATFORM_ENV]: osPlatform, [OS_STUB_ARCH_ENV]: osArch };
}

function runScriptWithOsStub(scriptPath, osPlatform, osArch, args = []) {
  const stub = writeOsStub(osPlatform, osArch);
  const env = { ...process.env };
  delete env.AGENT_DESKTOP_SKIP_DOWNLOAD;
  Object.assign(env, osStubEnv(osPlatform, osArch));
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
  osStubEnv,
  postinstallScriptPath,
  runScriptWithOsStub,
  wrapperScriptPath,
  writeOsStub,
};
