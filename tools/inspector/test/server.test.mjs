import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {once} from 'node:events';
import {mkdtemp, readdir, rm} from 'node:fs/promises';
import {createConnection} from 'node:net';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';

test('shutdown closes incomplete HTTP connections and removes private state', {timeout: 10000, skip: process.platform === 'win32'}, async t => {
  const root = await mkdtemp(join(tmpdir(), 'inspector-shutdown-'));
  const child = spawn(process.execPath, [fileURLToPath(new URL('../server.mjs', import.meta.url)), '--port', '0', '--bin', process.execPath, '--no-open'],
    {env: {...process.env, TMPDIR: root}, stdio: ['ignore', 'pipe', 'pipe']});
  t.after(async () => { child.kill('SIGKILL'); await rm(root, {recursive: true, force: true}); });
  const exited = once(child, 'exit');
  const port = await new Promise((resolve, reject) => {
    let output = '';
    child.stdout.on('data', data => { output += data; const match = /127\.0\.0\.1:(\d+)/.exec(output); if (match) resolve(Number(match[1])); });
    child.on('error', reject);
    child.on('exit', () => reject(new Error('Server exited before listening')));
  });
  assert.equal((await readdir(root)).length, 1);
  const connection = createConnection({host: '127.0.0.1', port});
  connection.on('error', () => {});
  t.after(() => connection.destroy());
  await once(connection, 'connect');
  connection.write('GET / HTTP/1.1\r\n');
  child.kill('SIGTERM');
  assert.deepEqual(await exited, [0, null]);
  assert.deepEqual(await readdir(root), []);
});
