const assert = require('node:assert/strict');
const {
  chmodSync,
  closeSync,
  openSync,
  readFileSync,
  unlinkSync,
  writeFileSync,
} = require('node:fs');
const { join } = require('node:path');
const { afterEach, test } = require('node:test');

const postinstall = require('../../npm/scripts/postinstall.js');
const { resolve } = require('../../npm/lib/platform.js');
const {
  createTemporaryRoots,
  postinstallScriptPath,
  runScriptWithOsStub,
  wrapperScriptPath,
  writeOsStub,
} = require('./helpers.js');

const { temporaryDirectory, drainRoots } = createTemporaryRoots();

afterEach(() => {
  delete process.env.AGENT_DESKTOP_MACOS_HELPER_PATH;
  for (const name of ['agent-desktop-win32-x64.exe', 'agent-desktop-win32-arm64.exe']) {
    try { unlinkSync(join(__dirname, '..', '..', 'npm', 'bin', name)); } catch {}
  }
  for (const root of drainRoots()) {
    postinstall.trashRecoverably(root);
  }
});

test('wrapper resolves the released win32-x64 mapping and names the missing-binary cause', async () => {
  const { code, stderr } = await runScriptWithOsStub(wrapperScriptPath, 'win32', 'x64', ['version']);
  assert.equal(code, 1);
  assert.match(stderr, /Error: Native binary not found for win32-x64/);
  assert.match(stderr, /--ignore-scripts/);
});

test('wrapper resolves the released win32-arm64 mapping and exits non-zero when the binary is absent', async () => {
  const { code, stderr } = await runScriptWithOsStub(wrapperScriptPath, 'win32', 'arm64', ['version']);
  assert.equal(code, 1);
  assert.match(stderr, /Error: Native binary not found for win32-arm64/);
  assert.match(stderr, /Expected: .*agent-desktop-win32-arm64\.exe/);
});

test('wrapper refuses an unmapped platform key by name', async () => {
  const { code, stderr } = await runScriptWithOsStub(wrapperScriptPath, 'sunos', 'x64', ['version']);
  assert.equal(code, 1);
  assert.match(stderr, /Error: Unsupported platform: sunos-x64/);
});

test('wrapper reports a non-zero exit status when the child dies to a signal', {
  skip: process.platform === 'win32',
}, async () => {
  const entry = resolve('darwin', 'arm64');
  const fakeBinary = join(wrapperScriptPath, '..', entry.binaryName);
  const fakeHelper = join(wrapperScriptPath, '..', 'agent-desktop-macos-helper');
  writeFileSync(fakeBinary, '#!/usr/bin/env node\nprocess.kill(process.pid, "SIGKILL");\n', { mode: 0o755 });
  chmodSync(fakeBinary, 0o755);
  writeFileSync(fakeHelper, '#!/usr/bin/env node\n', { mode: 0o755 });
  chmodSync(fakeHelper, 0o755);
  try {
    const { code, stderr } = await runScriptWithOsStub(
      wrapperScriptPath,
      'darwin',
      'arm64',
      ['version'],
    );
    assert.notEqual(code, 0, 'a signal-killed child must not be reported as success');
    assert.match(stderr, /terminated by signal/);
  } finally {
    try { unlinkSync(fakeBinary); } catch {}
    try { unlinkSync(fakeHelper); } catch {}
  }
});

test('AGENT_DESKTOP_BINARY_PATH installs on windows without any helper present', () => {
  const scratch = temporaryDirectory();
  const customBinary = join(scratch, 'stand-in.exe');
  writeFileSync(customBinary, 'fake-win-binary');
  const installedBinary = join(__dirname, '..', '..', 'npm', 'bin', 'agent-desktop-win32-x64.exe');
  const stub = writeOsStub('win32', 'x64');
  let stderr = '';
  const env = { ...process.env };
  delete env.AGENT_DESKTOP_SKIP_DOWNLOAD;
  env.AGENT_DESKTOP_BINARY_PATH = customBinary;
  const errFile = join(scratch, 'child-stderr.txt');
  const errFd = openSync(errFile, 'w');
  try {
    require('node:child_process').execFileSync(
      process.execPath,
      ['-r', stub, postinstallScriptPath],
      { env, stdio: ['ignore', 'ignore', errFd] },
    );
  } finally {
    closeSync(errFd);
    stderr = readFileSync(errFile, 'utf8');
    try { unlinkSync(stub); } catch {}
    try { unlinkSync(errFile); } catch {}
  }
  try {
    assert.equal(readFileSync(installedBinary, 'utf8'), 'fake-win-binary');
    assert.match(stderr, /Using binary from AGENT_DESKTOP_BINARY_PATH/);
    assert.doesNotMatch(stderr, /macOS helper not found/);
  } finally {
    try { unlinkSync(installedBinary); } catch {}
  }
});
