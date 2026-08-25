const assert = require('node:assert/strict');
const { execFileSync, spawn } = require('node:child_process');
const {
  chmodSync,
  existsSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  unlinkSync,
  writeFileSync,
} = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');
const { afterEach, test } = require('node:test');

const postinstall = require('../../npm/scripts/postinstall.js');
const { PLATFORMS, resolve, tarballName } = require('../../npm/lib/platform.js');

const postinstallScriptPath = join(__dirname, '..', '..', 'npm', 'scripts', 'postinstall.js');
const wrapperScriptPath = join(__dirname, '..', '..', 'npm', 'bin', 'agent-desktop.js');

const roots = [];

afterEach(() => {
  delete process.env.AGENT_DESKTOP_MACOS_HELPER_PATH;
  for (const root of roots.splice(0)) {
    postinstall.trashRecoverably(root);
  }
});

function temporaryDirectory() {
  const root = mkdtempSync(join(tmpdir(), 'agent-desktop-npm-test-'));
  roots.push(root);
  return root;
}

function archive(entries) {
  const root = temporaryDirectory();
  const source = join(root, 'source');
  mkdirSync(source);
  for (const [name, contents] of Object.entries(entries)) {
    const path = join(source, name);
    writeFileSync(path, contents, { mode: 0o755 });
    chmodSync(path, 0o755);
  }
  const tarball = join(root, 'release.tar.gz');
  execFileSync('tar', ['-czf', tarball, '-C', source, ...Object.keys(entries)]);
  return { root, tarball };
}

function executable(contents) {
  const root = temporaryDirectory();
  const path = join(root, 'trash');
  writeFileSync(path, contents, { mode: 0o755 });
  return path;
}

function captureWarnings(run) {
  const warnings = [];
  const write = process.stderr.write;
  process.stderr.write = (chunk) => {
    warnings.push(String(chunk));
    return true;
  };
  try {
    run();
    return warnings.join('');
  } finally {
    process.stderr.write = write;
  }
}

function runScriptWithOsStub(scriptPath, osPlatform, osArch, args = []) {
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

test('checksum lookup requires an exact archive name', () => {
  const hash = 'a'.repeat(64);
  assert.equal(postinstall.checksumFor(`${hash}  release.tar.gz\n`, 'release.tar.gz'), hash);
  assert.throws(
    () => postinstall.checksumFor(`${hash}  old-release.tar.gz\n`, 'release.tar.gz'),
    /Checksum entry missing/,
  );
});

test('archive validation rejects missing and additional payloads', () => {
  const valid = archive({
    'agent-desktop': 'cli',
    'agent-desktop-macos-helper': 'helper',
  });
  postinstall.validateArchive(valid.tarball);

  const extra = archive({
    'agent-desktop': 'cli',
    'agent-desktop-macos-helper': 'helper',
    'unexpected': 'payload',
  });
  assert.throws(() => postinstall.validateArchive(extra.tarball), /unexpected entries/);
});

test('archive installation preserves the exact paired executables', () => {
  const payload = archive({
    'agent-desktop': 'cli-build-v1',
    'agent-desktop-macos-helper': 'helper-build-v1',
  });
  const destination = temporaryDirectory();
  const binary = join(destination, 'agent-desktop-darwin-arm64');
  const helper = join(destination, 'agent-desktop-macos-helper');

  postinstall.installArchive(payload.tarball, binary, helper);

  assert.equal(readFileSync(binary, 'utf8'), 'cli-build-v1');
  assert.equal(readFileSync(helper, 'utf8'), 'helper-build-v1');
});

test('recoverable cleanup invokes trash and removes the original path', () => {
  const target = temporaryDirectory();
  const recovered = `${target}.recovered`;
  roots.push(recovered);
  const fakeTrash = executable('#!/bin/sh\nmv "$1" "$1.recovered"\n');

  postinstall.trashRecoverably(target, fakeTrash);
  assert.equal(existsSync(target), false);
  assert.equal(existsSync(recovered), true);
});

test('recoverable cleanup retains artifacts and warns when trash is unavailable or fails', () => {
  const unavailable = temporaryDirectory();
  const failing = temporaryDirectory();
  const fakeTrash = executable('#!/bin/sh\nexit 9\n');

  for (const [target, command, reason] of [
    [
      unavailable,
      '/definitely-missing-agent-desktop-trash',
      'trash command is unavailable: /definitely-missing-agent-desktop-trash',
    ],
    [failing, fakeTrash, 'trash exited with status 9'],
  ]) {
    const warnings = captureWarnings(() =>
      postinstall.trashRecoverably(target, command),
    );
    assert.equal(existsSync(target), true);
    assert.ok(warnings.includes(`retained at ${target}:`));
    assert.ok(warnings.includes(reason));
  }
});

test('cleanup failure does not mask a successful archive install', () => {
  const payload = archive({
    'agent-desktop': 'cli-build-v2',
    'agent-desktop-macos-helper': 'helper-build-v2',
  });
  const destination = temporaryDirectory();
  const binary = join(destination, 'agent-desktop-darwin-arm64');
  const helper = join(destination, 'agent-desktop-macos-helper');

  const warnings = captureWarnings(() =>
    postinstall.installArchive(
      payload.tarball,
      binary,
      helper,
      '/definitely-missing-agent-desktop-trash',
    ),
  );

  assert.equal(readFileSync(binary, 'utf8'), 'cli-build-v2');
  assert.equal(readFileSync(helper, 'utf8'), 'helper-build-v2');
  assert.match(warnings, /Could not move cleanup artifact to Trash; retained at .*\.extract-/);
  const retained = warnings.match(/retained at (.*\.extract-[^:]+):/)?.[1];
  assert.ok(retained);
  assert.equal(existsSync(retained), true);
  roots.push(retained);
});

test('custom helper override must be absolute', () => {
  process.env.AGENT_DESKTOP_MACOS_HELPER_PATH = 'relative-helper';
  assert.throws(
    () => postinstall.customHelperPath('/tmp/agent-desktop'),
    /must be an absolute path/,
  );
});

const EXPECTED_PLATFORM_ROWS = {
  'darwin-arm64': {
    target: 'aarch64-apple-darwin',
    binaryName: 'agent-desktop-darwin-arm64',
    entries: ['agent-desktop', 'agent-desktop-macos-helper'],
    released: true,
  },
  'darwin-x64': {
    target: 'x86_64-apple-darwin',
    binaryName: 'agent-desktop-darwin-x64',
    entries: ['agent-desktop', 'agent-desktop-macos-helper'],
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
    released: false,
  },
  'win32-x64': {
    target: 'x86_64-pc-windows-msvc',
    binaryName: 'agent-desktop-win32-x64.exe',
    entries: ['agent-desktop.exe'],
    released: false,
  },
};

test('resolve returns target triple, binary name, entry set and released flag for every platform row', () => {
  assert.deepEqual(Object.keys(PLATFORMS).sort(), Object.keys(EXPECTED_PLATFORM_ROWS).sort());
  for (const [key, expected] of Object.entries(EXPECTED_PLATFORM_ROWS)) {
    const separator = key.indexOf('-');
    const entry = resolve(key.slice(0, separator), key.slice(separator + 1));
    assert.deepEqual(entry, expected, `unexpected resolution for ${key}`);
  }
  assert.equal(resolve('sunos', 'x64'), undefined);
});

test('platform table keeps every key carried by the three pre-extraction maps', () => {
  const preExtractionUnion = [
    'darwin-arm64',
    'darwin-x64',
    'linux-arm64',
    'linux-x64',
    'win32-x64',
  ];
  const keys = Object.keys(PLATFORMS);
  for (const key of preExtractionUnion) {
    assert.ok(keys.includes(key), `platform ${key} was dropped from the table`);
  }
});

test('tarball name matches the template postinstall previously constructed inline', () => {
  assert.equal(tarballName('0.8.3', 'aarch64-apple-darwin'), 'agent-desktop-v0.8.3-aarch64-apple-darwin.tar.gz');
  assert.equal(tarballName('0.8.3', 'x86_64-pc-windows-msvc'), 'agent-desktop-v0.8.3-x86_64-pc-windows-msvc.tar.gz');
});

test('postinstall refuses unreleased platforms with exit code 0 and the standing message', async () => {
  for (const [osPlatform, osArch] of [['win32', 'x64'], ['win32', 'arm64']]) {
    const { code, stderr } = await runScriptWithOsStub(postinstallScriptPath, osPlatform, osArch);
    assert.equal(code, 0, `expected exit 0 for ${osPlatform}-${osArch}`);
    assert.deepEqual(
      stderr.split('\n').filter((line) => line.startsWith('agent-desktop: ')),
      [
        'agent-desktop: agent-desktop currently supports macOS only.',
        'agent-desktop: Windows and Linux support is coming in Phase 2.',
        'agent-desktop: See: https://github.com/lahfir/agent-desktop',
      ],
      `unexpected refusal output for ${osPlatform}-${osArch}`,
    );
  }
});

test('wrapper still resolves the unreachable win32-x64 mapping unchanged', async () => {
  const { code, stderr } = await runScriptWithOsStub(wrapperScriptPath, 'win32', 'x64', ['version']);
  assert.equal(code, 1);
  assert.match(stderr, /Error: Native binary not found for win32-x64/);
});

test('wrapper reports an unsupported platform and exits non-zero', async () => {
  const { code, stderr } = await runScriptWithOsStub(wrapperScriptPath, 'win32', 'arm64', ['version']);
  assert.equal(code, 1);
  assert.match(stderr, /Error: Native binary not found for win32-arm64/);
  assert.match(stderr, /Expected: .*agent-desktop-win32-arm64\.exe/);
});
