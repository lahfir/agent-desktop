import {test} from 'node:test';
import assert from 'node:assert/strict';
import {highlightItems} from '../public/tree-model.js';

const items = [
  {key: 'window', node: {role: 'window'}},
  {key: 'a', node: {ref_id: '@sone:e1'}},
  {key: 'b', node: {ref_id: '@sone:e2'}},
  {key: 'hidden', node: {states: ['hidden']}},
];

test('selecting one node isolates only that highlight, even with search results', () => {
  assert.deepEqual(highlightItems(items, 'a', new Set(['@sone:e1', '@sone:e2'])).map(item => item.key), ['a']);
  assert.deepEqual(highlightItems(items, 'window', null).map(item => item.key), ['window']);
  assert.deepEqual(highlightItems(items, 'hidden', null), []);
});

test('search highlights only matches and explicit overview shows all visible nodes', () => {
  assert.deepEqual(highlightItems(items, null, new Set(['@sone:e2'])).map(item => item.key), ['b']);
  assert.deepEqual(highlightItems(items, null, new Set()), []);
  assert.deepEqual(highlightItems(items, null, null).map(item => item.key), ['window', 'a', 'b']);
});
