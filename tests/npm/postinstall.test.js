const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const {
  chmodSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');
const { afterEach, test } = require('node:test');

const postinstall = require('../../npm/scripts/postinstall.js');

const roots = [];

afterEach(() => {
  delete process.env.AGENT_DESKTOP_MACOS_HELPER_PATH;
  for (const root of roots.splice(0)) {
    rmSync(root, { recursive: true, force: true });
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

test('custom helper override must be absolute', () => {
  process.env.AGENT_DESKTOP_MACOS_HELPER_PATH = 'relative-helper';
  assert.throws(
    () => postinstall.customHelperPath('/tmp/agent-desktop'),
    /must be an absolute path/,
  );
});
