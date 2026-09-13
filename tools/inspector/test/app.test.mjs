import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {runInNewContext} from 'node:vm';

const source = await readFile(new URL('../public/app.js', import.meta.url), 'utf8');
const performSource = source.slice(source.indexOf('async function perform('), source.indexOf('\nfunction select('));

function client(overrides = {}) {
  const elements = new Map();
  const context = {
    state: {busy: false, invalid: false, view: null}, token: 'test',
    byId: id => { if (!elements.has(id)) elements.set(id, {}); return elements.get(id); },
    startFeedback() {}, finishFeedback() {}, render() {}, showCommands() {},
    AbortSignal, fetch: async () => ({json: async () => ({ok: true})}), ...overrides,
  };
  runInNewContext(performSource, context);
  return context;
}

test('feedback initialization failures always release the busy guard', async () => {
  const context = client({startFeedback() { throw new Error('feedback failed'); }});
  await context.perform({operation: 'apps'}, 'Loading');
  assert.equal(context.state.busy, false);
  context.startFeedback = () => {};
  await context.perform({operation: 'apps'}, 'Retry');
  assert.equal(context.byId('status').textContent, 'Choose an app with an open window.');
});

test('numeric DOMException codes are treated as transport failures, not CLI error codes', async () => {
  const context = client({fetch: async () => { throw new DOMException('signal timed out', 'TimeoutError'); }});
  await context.perform({operation: 'apps'}, 'Loading');
  assert.equal(context.state.invalid, true);
  assert.doesNotMatch(context.byId('error').textContent, /^23:/);
  assert.equal(context.state.busy, false);
});

test('pane dragging releases capture on pointerup, cancellation, and lost capture', () => {
  for (const stop of ['onpointerup', 'onpointercancel', 'onlostpointercapture']) {
    let width = '280px';
    let captured = false;
    const handle = {classList: {add() {}, remove() {}}, setPointerCapture() { captured = true; },
      hasPointerCapture: () => captured, releasePointerCapture() { captured = false; }};
    const context = {MIN_PANE: 180, MAX_PANE: 640,
      byId: id => id === 'resize-left' ? handle : {getBoundingClientRect: () => ({width: 280})},
      document: {body: {style: {}}}, localStorage: {setItem() {}},
      workspace: {style: {setProperty: (_, value) => { width = value; }}},
      getComputedStyle: () => ({getPropertyValue: () => width})};
    runInNewContext(source.slice(source.indexOf('function setupResize('), source.indexOf('\napplyPanes();')), context);
    context.setupResize('resize-left', 'left');
    handle.onpointerdown({button: 0, pointerId: 1, clientX: 0, preventDefault() {}});
    handle.onpointermove({pointerId: 1, clientX: 20});
    assert.equal(width, '300px');
    handle[stop]();
    assert.equal(captured, false);
    assert.equal(context.document.body.style.userSelect, '');
    handle.onpointermove({pointerId: 1, clientX: 60});
    assert.equal(width, '300px');
  }
});

test('structured CLI errors preserve a healthy view unless refresh is required', async () => {
  const context = client({fetch: async () => ({json: async () => ({ok: false, error: {code: 'TIMEOUT', message: 'Read timed out'}})})});
  await context.perform({operation: 'apps'}, 'Loading');
  assert.equal(context.state.invalid, false);
  assert.match(context.byId('error').textContent, /^TIMEOUT:/);
});
