import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createInspector} from '../inspector.mjs';
import {identityKey} from '../cli.mjs';
import {indexTree, replaceBranch, findIdentity, ancestors} from '../public/tree-model.js';
import {revealTreeRow} from '../public/tree-view.js';

const root = {role: 'window', children: [
  {role: 'group', ref_id: '@sone:e1', children: [{role: 'button', ref_id: '@sone:e2'}]},
  {role: 'button', ref_id: '@sone:e3'},
]};
const view = tree => ({tree, window: {id: 'w-1'}, snapshotId: 'sone', frame: null});

test('revealing a deep row scrolls only the tree and keeps the start of long labels visible', () => {
  const container = {scrollLeft: 0, scrollTop: 0, clientWidth: 240, clientHeight: 300,
    getBoundingClientRect: () => ({left: 20, top: 200})};
  const row = {getBoundingClientRect: () => ({left: 360, top: 550, width: 900, height: 40})};
  revealTreeRow(container, row);
  assert.equal(container.scrollLeft, 338);
  assert.equal(container.scrollTop, 92);
  container.clientWidth = 0;
  revealTreeRow(container, row);
  assert.equal(container.scrollLeft, 338);
});

test('revealing an already visible row does not move the tree', () => {
  const container = {scrollLeft: 50, scrollTop: 75, clientWidth: 240, clientHeight: 300,
    getBoundingClientRect: () => ({left: 20, top: 200})};
  revealTreeRow(container, {getBoundingClientRect: () => ({left: 22, top: 202, width: 100, height: 40})});
  assert.equal(container.scrollLeft, 50);
  assert.equal(container.scrollTop, 75);
});

test('replacing a branch removes old descendants and preserves siblings', () => {
  const tree = replaceBranch(root, '@sone:e1', {role: 'group', ref_id: '@sone:e4', children: [{role: 'button', ref_id: '@sone:e5'}]});
  assert.deepEqual(indexTree(tree).map(item => item.node.ref_id), [undefined, '@sone:e4', '@sone:e5', '@sone:e3']);
  assert.deepEqual(ancestors(indexTree(tree), '@sone:e5'), ['@sone:e4', 'node:0']);
  assert.equal(root.children[0].ref_id, '@sone:e1');
});

test('search never connects nodes by a shared name or ambiguous identity', () => {
  assert.equal(findIdentity(root, 'missing'), null);
  assert.equal(findIdentity({identity_key: 'same', children: [{identity_key: 'same'}]}, 'same'), null);
});

test('inspector rejects stale revisions and foreign refs before calling CLI', async () => {
  const calls = [];
  const cli = {capture: async args => { calls.push(args); return view(root); }, run: async args => { calls.push(args); return {}; }};
  const inspector = createInspector(cli);
  const loaded = await inspector.execute({operation: 'snapshot', app: 'TextEdit'});
  await assert.rejects(inspector.execute({operation: 'expand', revision: loaded.revision + 1, ref: '@sone:e1'}), {code: 'STALE_VIEW'});
  await assert.rejects(inspector.execute({operation: 'get', revision: loaded.revision, ref: '@sother:e1', property: 'value'}), {code: 'STALE_REF'});
  assert.equal(calls.length, 1);
});

test('root-scoped find uses CLI results and refreshes only that branch for reveal', async () => {
  const entry = {pid: 1, process_instance: 'one', source_window_id: 'w-1', path: [0, 0], role: 'button', name: 'Save'};
  const calls = [];
  let captures = 0;
  const cli = {
    capture: async args => {
      calls.push(args);
      return view(captures++ === 0 ? root : {role: 'group', ref_id: '@sone:e4', children: [{role: 'button', ref_id: '@sone:e5', identity_key: identityKey(entry)}]});
    },
    run: async args => { calls.push(args); return {matches: [{ref_id: '@sfind:e1', role: 'button', name: 'Save'}], total_matches: 1}; },
    entries: async () => new Map([['@sfind:e1', entry]]),
  };
  const inspector = createInspector(cli);
  const loaded = await inspector.execute({operation: 'snapshot', app: 'TextEdit'});
  const result = await inspector.execute({operation: 'find', revision: loaded.revision, ref: '@sone:e1', name: 'Save'});
  assert.deepEqual(calls[1], ['find', '--root', '@sone:e1', '--limit', '20', '--text=Save']);
  assert.deepEqual(calls[2].slice(0, 3), ['snapshot', '--root', '@sone:e1']);
  assert.equal(result.matches[0].treeRef, '@sone:e5');
  assert.equal(result.view.tree.children[1].ref_id, '@sone:e3');
  assert.equal(result.revision, 2);
});

test('find sends the selected field without changing the root, role, or exact flag', async () => {
  const calls = [];
  const cli = {
    capture: async () => view(root),
    run: async args => { calls.push(args); return {matches: [], total_matches: 0}; },
    entries: async () => new Map(),
  };
  const inspector = createInspector(cli);
  await inspector.execute({operation: 'snapshot', app: 'Finder'});
  for (const field of ['name', 'value', 'text']) {
    await inspector.execute({operation: 'find', revision: 1, ref: '@sone:e1', field, name: 'agent', role: 'textfield', exact: true});
    assert.deepEqual(calls.at(-1), ['find', '--root', '@sone:e1', '--limit', '20', `--${field}=agent`, '--role=textfield', '--exact']);
  }
});

test('an incomplete name search stays an error and allows an explicit value search', async () => {
  const calls = [];
  const inspector = createInspector({
    capture: async () => view(root),
    run: async args => {
      calls.push(args);
      if (args.includes('--name=agent')) throw Object.assign(new Error('Incomplete name evidence'), {code: 'TIMEOUT'});
      return {matches: [], total_matches: 0};
    },
    entries: async () => new Map(),
  });
  await inspector.execute({operation: 'snapshot', app: 'Finder'});
  const request = {operation: 'find', revision: 1, ref: '@sone:e1', field: 'name', name: 'agent'};
  await assert.rejects(inspector.execute(request), {code: 'TIMEOUT'});
  assert.equal(calls.length, 1);
  await inspector.execute({...request, field: 'value'});
  assert.equal(calls.length, 2);
  assert.ok(calls[1].includes('--value=agent'));
});

test('non-actionable groups are revealed by unique native geometry, never by name alone', async () => {
  const bounds = {x: 10, y: 20, width: 100, height: 30};
  const group = {role: 'group', bounds, children: [{role: 'textfield', value: 'Agent.pdf'}]};
  const tree = {role: 'list', ref_id: '@sone:e1', children: [group]};
  const cli = {
    capture: async () => view(tree),
    run: async () => ({matches: [{role: 'group', name: '(unnamed group)', bounds}], total_matches: 1}),
    entries: async () => new Map(),
  };
  const inspector = createInspector(cli);
  const loaded = await inspector.execute({operation: 'snapshot', app: 'AnyApp'});
  const request = {operation: 'find', revision: loaded.revision, ref: '@sone:e1', name: 'Agent', role: 'group'};
  const result = await inspector.execute(request);
  assert.equal(result.matches[0].treeRef, 'node:0.0');
  assert.equal(result.view.tree.children[0].ref_id, undefined);
  await assert.rejects(inspector.execute({operation: 'get', revision: result.revision, ref: 'node:0.0', property: 'value'}), {code: 'INVALID_ARGS'});

  tree.children.push({...group});
  const ambiguous = await inspector.execute({...request, revision: result.revision});
  assert.equal(ambiguous.matches[0].treeRef, null);
  tree.children = [{...group, bounds: {...bounds, x: 11}}];
  const moved = await inspector.execute({...request, revision: ambiguous.revision});
  assert.equal(moved.matches[0].treeRef, null);
});

test('a failed drill invalidates the view rather than leaving replaced refs usable', async () => {
  let captures = 0;
  const inspector = createInspector({capture: async () => { if (captures++) throw new Error('Capture failed'); return view(root); }});
  await inspector.execute({operation: 'snapshot', app: 'TextEdit'});
  await assert.rejects(inspector.execute({operation: 'expand', revision: 1, ref: '@sone:e1'}), {refreshRequired: true});
  await assert.rejects(inspector.execute({operation: 'get', revision: 1, ref: '@sone:e1', property: 'value'}), {code: 'STALE_VIEW'});
});
