import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, mkdir, writeFile, readdir, rm} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {fileURLToPath} from 'node:url';
import {createCli} from '../cli.mjs';
import {commandPreview, formatCommand} from '../public/feedback.js';

const fixture = fileURLToPath(new URL('./fixture-cli.mjs', import.meta.url));

test('capture joins artifact classifications and saved identity, then removes the temporary HTML', async t => {
  const home = await mkdtemp(join(tmpdir(), 'inspector-capture-'));
  t.after(() => rm(home, {recursive: true, force: true}));
  const cli = createCli(process.execPath, home);
  const result = await cli.capture([fixture, '--capture']);
  assert.equal(result.snapshotId, 'sone');
  assert.equal(result.tree.kind, 'root');
  assert.equal(result.tree.children[0].reason, 'Addressable');
  assert.ok(result.tree.children[0].identity_key);
  assert.deepEqual(await readdir(home), ['snapshots']);
});

test('capture preserves a successful tree when debug output is unavailable', async t => {
  const home = await mkdtemp(join(tmpdir(), 'inspector-warning-'));
  t.after(() => rm(home, {recursive: true, force: true}));
  const result = await createCli(process.execPath, home).capture([fixture, '--capture', '--warning']);
  assert.equal(result.frame, null);
  assert.equal(result.warning, 'Screenshot unavailable');
  assert.ok(result.tree.children[0].identity_key);
});

test('capture cleanup failure cannot replace the original structured CLI error', async t => {
  const home = await mkdtemp(join(tmpdir(), 'inspector-cleanup-'));
  t.after(() => rm(home, {recursive: true, force: true}));
  const warning = t.mock.method(console, 'error', () => {});
  await assert.rejects(createCli(process.execPath, home).capture([fixture, '--fail', '--cleanup-failure']), {code: 'STALE_REF'});
  assert.equal(warning.mock.callCount(), 1);
});

test('screen recording denial falls back to one plain observation', async t => {
  const home = await mkdtemp(join(tmpdir(), 'inspector-permission-'));
  t.after(() => rm(home, {recursive: true, force: true}));
  const log = [];
  const result = await createCli(process.execPath, home).capture([fixture, '--capture', '--deny-screen'], log);
  assert.equal(result.frame, null);
  assert.match(result.warning, /permission/);
  assert.ok(result.tree.children[0].identity_key);
  assert.equal(log.length, 2);
  assert.ok(!log[1].args.includes('--debug'));
  assert.ok(!log[1].args.includes('--screenshot'));
});

test('capture rejects a mismatched visual tree rather than silently mislabeling nodes', async t => {
  const home = await mkdtemp(join(tmpdir(), 'inspector-mismatch-'));
  t.after(() => rm(home, {recursive: true, force: true}));
  await assert.rejects(createCli(process.execPath, home).capture([fixture, '--capture', '--mismatch']), /incompatible/);
  assert.deepEqual(await readdir(home), ['snapshots']);
});

test('CLI execution preserves argument boundaries and isolates snapshot state', async t => {
  const home = await mkdtemp(join(tmpdir(), 'inspector-test-'));
  t.after(() => rm(home, {recursive: true, force: true}));
  const previous = process.env.AGENT_DESKTOP_SESSION;
  process.env.AGENT_DESKTOP_SESSION = 'unrelated-session';
  let cli;
  try { cli = createCli(process.execPath, home); }
  finally { if (previous === undefined) delete process.env.AGENT_DESKTOP_SESSION; else process.env.AGENT_DESKTOP_SESSION = previous; }
  const query = '--name=a; $(echo should-not-run) "quoted"';
  const log = [];
  const result = await cli.run([fixture, query], log);
  assert.equal(log[0].stateRoot, home);
  assert.equal(log[0].binary, process.execPath);
  assert.deepEqual(result.args, [query]);
  assert.equal(result.stateRoot, home);
  assert.equal(result.session, null);
});

test('ref lookup uses each qualified snapshot in the inspector store, not the latest snapshot', async t => {
  const home = await mkdtemp(join(tmpdir(), 'inspector-refs-'));
  t.after(() => rm(home, {recursive: true, force: true}));
  for (const id of ['sone', 'stwo']) {
    const directory = join(home, 'snapshots', id);
    await mkdir(directory, {recursive: true});
    await writeFile(join(directory, 'refmap.json'), JSON.stringify({inner: {'@e1': {name: id}}}));
  }
  const entries = await createCli(process.execPath, home).entries(['@sone:e1', '@stwo:e1']);
  assert.equal(entries.get('@sone:e1').name, 'sone');
  assert.equal(entries.get('@stwo:e1').name, 'stwo');
});

test('command history includes the private store and safely quotes shell arguments', () => {
  assert.equal(formatCommand({binary: '/path with spaces/agent-desktop', stateRoot: '/tmp/inspector', args: ['find', '--root', '@sone:e1', '--name=$(echo bad)']}),
    "env -u AGENT_DESKTOP_SESSION AGENT_DESKTOP_HOME=/tmp/inspector '/path with spaces/agent-desktop' find --root @sone:e1 '--name=$(echo bad)'");
});

test('nonzero CLI exits retain the structured error instead of guessing success', async () => {
  const cli = createCli(process.execPath, tmpdir());
  await assert.rejects(cli.run([fixture, '--fail']), {code: 'STALE_REF', message: 'Refresh the snapshot'});
});

test('progress shows the actual requested root and property command', () => {
  assert.equal(commandPreview({operation: 'expand', ref: '@sone:e3'}), 'agent-desktop snapshot --root @sone:e3 --max-depth 3');
  assert.equal(commandPreview({operation: 'is', ref: '@sone:e3', property: 'visible'}), 'agent-desktop is @sone:e3 --property visible');
  assert.match(commandPreview({operation: 'find', ref: '@sone:e3', name: 'Save as'}), /--root @sone:e3 "--text=Save as"/);
  assert.match(commandPreview({operation: 'find', ref: '@sone:e3', field: 'name', name: 'Save as'}), /--root @sone:e3 "--name=Save as"/);
  for (const field of ['value', 'text']) {
    assert.equal(commandPreview({operation: 'find', ref: '@sone:e3', field, name: 'agent'}), `agent-desktop find --root @sone:e3 --${field}=agent`);
  }
});
