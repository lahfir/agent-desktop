import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createBridge} from '../bridge.mjs';
import {listenAvailable} from '../http.mjs';

async function start(t, execute) {
  const token = 'test-token';
  const server = createBridge({execute}, token);
  const port = await listenAvailable(server, 0);
  t.after(() => { server.closeAllConnections(); server.close(); });
  const url = `http://127.0.0.1:${port}`;
  return {url, headers: {origin: url, 'x-inspector-token': token, 'content-type': 'application/json'}};
}

test('real HTTP bridge rejects cross-origin, unauthenticated, and mutating requests', async t => {
  let calls = 0;
  const {url, headers} = await start(t, async () => { calls++; return {apps: []}; });
  for (const changed of [{origin: 'http://evil.test'}, {'x-inspector-token': ''}]) {
    const response = await fetch(`${url}/api`, {method: 'POST', headers: {...headers, ...changed}, body: '{"operation":"apps"}'});
    assert.equal(response.status, 403);
  }
  const denied = await fetch(`${url}/api`, {method: 'POST', headers, body: '{"operation":"click","ref":"@sone:e1"}'});
  assert.equal(denied.status, 400);
  assert.equal(calls, 0);
  const allowed = await fetch(`${url}/api`, {method: 'POST', headers, body: '{"operation":"apps"}'});
  assert.deepEqual(await allowed.json(), {ok: true, apps: []});
  assert.equal(calls, 1);
  assert.equal(allowed.headers.get('access-control-allow-origin'), null);
});

test('public assets never disclose the capability token and retain browser isolation headers', async t => {
  const {url} = await start(t, () => assert.fail('Must not dispatch'));
  const response = await fetch(url);
  assert.equal(response.status, 200);
  assert.ok(!(await response.text()).includes('test-token'));
  assert.match(response.headers.get('content-security-policy'), /frame-ancestors 'none'/);
  assert.equal(response.headers.get('cache-control'), 'no-store');
  assert.equal((await fetch(url, {headers: {'sec-fetch-site': 'cross-site'}})).status, 403);
  assert.equal((await fetch(url, {method: 'PUT'})).status, 405);
});

test('oversized bodies and filesystem paths are not accepted', async t => {
  const {url, headers} = await start(t, () => assert.fail('Must not dispatch'));
  const response = await fetch(`${url}/api`, {method: 'POST', headers, body: JSON.stringify({operation: 'apps', text: 'a'.repeat(9000)})});
  assert.equal(response.status, 413);
  assert.equal((await fetch(`${url}/server.mjs`)).status, 404);
  assert.equal((await fetch(`${url}/api`)).status, 404);
});

test('simultaneous requests cannot race snapshot ownership', async t => {
  let release;
  const pending = new Promise(resolve => { release = resolve; });
  let entered;
  const started = new Promise(resolve => { entered = resolve; });
  const {url, headers} = await start(t, () => { entered(); return pending; });
  const first = fetch(`${url}/api`, {method: 'POST', headers, body: '{"operation":"apps"}'});
  await started;
  const second = await fetch(`${url}/api`, {method: 'POST', headers, body: '{"operation":"apps"}'});
  assert.equal(second.status, 409);
  release({apps: []});
  assert.equal((await first).status, 200);
});
