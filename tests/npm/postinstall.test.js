const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const {
  chmodSync,
  existsSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  unlinkSync,
  writeFileSync,
} = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');
const { afterEach, test } = require('node:test');

const postinstall = require('../../npm/scripts/postinstall.js');
const { PLATFORMS, resolve, tarballName } = require('../../npm/lib/platform.js');
const { runScriptWithOsStub } = require('./helpers.js');

const postinstallScriptPath = join(__dirname, '..', '..', 'npm', 'scripts', 'postinstall.js');

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


test('checksum lookup requires an exact archive name', () => {
  const hash = 'a'.repeat(64);
  assert.equal(postinstall.checksumFor(`${hash}  release.tar.gz\n`, 'release.tar.gz'), hash);
  assert.throws(
    () => postinstall.checksumFor(`${hash}  old-release.tar.gz\n`, 'release.tar.gz'),
    /Checksum entry missing/,
  );
});

test('archive validation rejects missing and additional payloads on the darwin entry set', () => {
  const valid = archive({
    'agent-desktop': 'cli',
    'agent-desktop-macos-helper': 'helper',
  });
  postinstall.validateArchive(valid.tarball, ['agent-desktop', 'agent-desktop-macos-helper']);

  const extra = archive({
    'agent-desktop': 'cli',
    'agent-desktop-macos-helper': 'helper',
    'unexpected': 'payload',
  });
  assert.throws(
    () => postinstall.validateArchive(extra.tarball, ['agent-desktop', 'agent-desktop-macos-helper']),
    /unexpected entries/,
  );
});

test('archive validation is platform-correct on the win32 single-entry set', () => {
  const valid = archive({ 'agent-desktop.exe': 'cli' });
  postinstall.validateArchive(valid.tarball, ['agent-desktop.exe']);

  const extra = archive({
    'agent-desktop.exe': 'cli',
    'unexpected.exe': 'payload',
  });
  assert.throws(
    () => postinstall.validateArchive(extra.tarball, ['agent-desktop.exe']),
    /unexpected entries/,
  );
});

test('archive installation preserves the exact paired executables', () => {
  const payload = archive({
    'agent-desktop': 'cli-build-v1',
    'agent-desktop-macos-helper': 'helper-build-v1',
  });
  const destination = temporaryDirectory();
  const binary = join(destination, 'agent-desktop-darwin-arm64');
  const helper = join(destination, 'agent-desktop-macos-helper');

  postinstall.installArchive(payload.tarball, binary, helper, resolve('darwin', 'arm64'));

  assert.equal(readFileSync(binary, 'utf8'), 'cli-build-v1');
  assert.equal(readFileSync(helper, 'utf8'), 'helper-build-v1');
});

test('archive installation places the windows executable and leaves no staging directory behind', () => {
  const payload = archive({ 'agent-desktop.exe': 'cli-win-build' });
  const destination = temporaryDirectory();
  const binary = join(destination, 'agent-desktop-win32-x64.exe');
  const unusedHelper = join(destination, 'unused-helper');

  const before = readdirSync(destination);
  postinstall.installArchive(payload.tarball, binary, unusedHelper, resolve('win32', 'x64'));

  assert.equal(readFileSync(binary, 'utf8'), 'cli-win-build');
  const newEntries = readdirSync(destination).filter((name) => !before.includes(name));
  assert.deepEqual(newEntries.map((name) => join(destination, name)), [binary]);
});

test('staging cleanup removes its own extract directory and a planted survivor is detectable', () => {
  const payload = archive({ 'agent-desktop.exe': 'cli-win-build' });
  const destination = temporaryDirectory();
  const binary = join(destination, 'agent-desktop-win32-x64.exe');
  const npmBin = join(__dirname, '..', '..', 'npm', 'bin');
  const planted = join(npmBin, '.extract-invert-survivor');
  mkdirSync(planted);
  try {
    postinstall.installArchive(
      payload.tarball,
      binary,
      join(destination, 'unused-helper'),
      resolve('win32', 'x64'),
    );
    const leftovers = readdirSync(npmBin).filter((name) => name.startsWith('.extract-'));
    assert.deepEqual(
      leftovers.sort(),
      ['.extract-invert-survivor'],
      'the real staging must be cleaned while a planted survivor stays detectable',
    );
  } finally {
    rmSync(planted, { recursive: true, force: true });
  }
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
      resolve('darwin', 'arm64'),
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
    released: true,
  },
  'win32-x64': {
    target: 'x86_64-pc-windows-msvc',
    binaryName: 'agent-desktop-win32-x64.exe',
    entries: ['agent-desktop.exe'],
    released: true,
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

test('postinstall refuses unreleased platforms with exit code 0 and names the released keys', async () => {
  for (const [osPlatform, osArch] of [['linux', 'x64'], ['linux', 'arm64']]) {
    const { code, stderr } = await runScriptWithOsStub(postinstallScriptPath, osPlatform, osArch);
    assert.equal(code, 0, `expected exit 0 for ${osPlatform}-${osArch}`);
    const lines = stderr.split('\n').filter((line) => line.startsWith('agent-desktop: '));
    assert.equal(
      lines[0],
      `agent-desktop: agent-desktop has no released native binary for ${osPlatform}-${osArch}.`,
      `unexpected refusal output for ${osPlatform}-${osArch}`,
    );
    assert.match(lines[1], /^agent-desktop: Released platform keys today: darwin-arm64, darwin-x64, win32-arm64, win32-x64\.$/);
    assert.equal(lines[2], 'agent-desktop: See: https://github.com/lahfir/agent-desktop');
  }
});

test('the manual fallback names every archive member and the verification steps', () => {
  const warnings = captureWarnings(() =>
    postinstall.printManualFallback(
      'https://example.invalid/tarball.tar.gz',
      'https://example.invalid/checksums.txt',
      resolve('win32', 'arm64'),
    ),
  );
  assert.match(warnings, /Download manually from:/);
  assert.match(warnings, /https:\/\/example\.invalid\/tarball\.tar\.gz/);
  assert.match(warnings, /agent-desktop\.exe/);
  assert.ok(!warnings.includes('agent-desktop-macos-helper'));
  assert.match(warnings, /curl -fsSL https:\/\/example\.invalid\/checksums\.txt/);
  assert.match(warnings, /sha256sum <downloaded-archive>/);
  assert.match(warnings, /gh attestation verify <downloaded-archive> --repo lahfir\/agent-desktop/);
});

test('prompted skills are a subset of the skill packages that exist', () => {
  const source = readFileSync(postinstallScriptPath, 'utf8');
  const printed = [...source.matchAll(/add-skill lahfir\/([a-z0-9-]+)/g)].map((m) => m[1]);
  const mapped = [...source.matchAll(/^\s{4}(?:darwin|win32|linux): '([a-z0-9-]+)',$/gm)].map(
    (m) => m[1],
  );
  const skillsDir = join(__dirname, '..', '..', 'skills');
  for (const name of new Set([...printed, ...mapped])) {
    assert.ok(existsSync(join(skillsDir, name, 'SKILL.md')), `advertised ${name} must exist`);
  }
  for (const missing of ['agent-desktop-macos', 'agent-desktop-linux']) {
    assert.ok(
      !existsSync(join(skillsDir, missing)),
      `${missing} must not exist to be advertised`,
    );
  }
});
