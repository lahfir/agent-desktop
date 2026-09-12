import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {authorize, listenAvailable, validateRequest} from '../http.mjs';

const token = 'local-test-token';
const request = (headers = {}) => ({headers: {host: '127.0.0.1:4317', origin: 'http://127.0.0.1:4317', 'x-inspector-token': token, ...headers}});

test('API requires the exact local host, origin, and capability token', () => {
  assert.doesNotThrow(() => authorize(request(), 4317, token));
  for (const headers of [
    {host: 'evil.test:4317'}, {origin: 'https://evil.test'}, {origin: undefined},
    {'x-inspector-token': undefined}, {'x-inspector-token': 'wrong'}, {host: '127.0.0.1:4318'},
  ]) assert.throws(() => authorize(request(headers), 4317, token));
});

test('only typed, read-only operations are accepted', () => {
  assert.deepEqual(validateRequest({operation: 'find', revision: 1, ref: '@sabc:e1', name: 'a; touch /tmp/no', role: 'button'}).name, 'a; touch /tmp/no');
  for (const body of [
    {operation: 'click'}, {operation: 'exec', command: 'whoami'},
    {operation: 'snapshot', app: '--headed'}, {operation: 'get', revision: 1, ref: '../secret', property: 'value'},
    {operation: 'get', revision: 1, ref: '@sabc:e1', property: '--trace'},
    {operation: 'find', revision: 1, name: 'x', extra: true},
    {operation: 'snapshot', app: 'x'.repeat(513)},
  ]) assert.throws(() => validateRequest(body));
});

test('find allows only the supported search fields', () => {
  const request = {operation: 'find', revision: 1, ref: '@sabc:e1', name: 'agent'};
  for (const field of ['name', 'value', 'text']) assert.doesNotThrow(() => validateRequest({...request, field}));
  for (const field of ['', 'trace', '--headed', null, 1, ['name']]) assert.throws(() => validateRequest({...request, field}));
});

test('a busy starting port falls back to another loopback port', async t => {
  const occupied = createServer();
  await listenAvailable(occupied, 0);
  const port = occupied.address().port;
  const server = createServer();
  t.after(() => { occupied.close(); server.close(); });
  await listenAvailable(server, port);
  assert.notEqual(server.address().port, port);
  assert.equal(server.address().address, '127.0.0.1');
});
