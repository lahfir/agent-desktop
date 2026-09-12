import {indexTree, ancestors} from './tree-model.js';
import {renderTree, revealTreeRow} from './tree-view.js';
import {renderScreenshot} from './screenshot.js';
import {startFeedback, finishFeedback, formatCommand} from './feedback.js';

const byId = id => document.getElementById(id);
const token = new URLSearchParams(location.hash.slice(1)).get('token') || '';
const state = {view: null, revision: 0, selectedKey: null, open: new Set(), matches: [], busy: false, invalid: false};
const history = [];
let resultInfo = '';
let highlightMatches = false;
const properties = {get: ['value', 'text', 'title', 'bounds', 'role', 'states'], is: ['visible', 'enabled', 'checked', 'focused', 'expanded', 'selected']};

function selected() { return indexTree(state.view?.tree).find(item => item.key === state.selectedKey); }
function scoped() {
  const item = selected();
  return item?.node.ref_id ? {ref: item.node.ref_id} : {};
}
function canSearch() { const item = selected(); return Boolean(item && (item.node.ref_id || item.parentKey === null)); }
function canExpand(node) { return Boolean(node?.ref_id && (node.children_count || node.subtree_truncated || node.children?.length)); }

function showCommands(commands) {
  history.push(...commands);
  if (history.length > 50) history.splice(0, history.length - 50);
  byId('commands').replaceChildren();
  for (const command of [...history].reverse()) {
    const li = document.createElement('li');
    const code = document.createElement('code');
    code.textContent = formatCommand(command);
    const status = document.createElement('span');
    status.textContent = `${command.ok ? 'Succeeded' : 'Failed'} · ${command.elapsedMs} ms`;
    li.append(code, status);
    byId('commands').append(li);
  }
  byId('command-summary').textContent = `Command history (${history.length})`;
}

async function perform(body, label, apply) {
  if (state.busy) return;
  state.busy = true;
  try {
    byId('error').hidden = true;
    byId('status').textContent = label;
    startFeedback(body.operation === 'find' && !body.ref ? {...body, app: state.view.app, windowId: state.view.window.id} : body, label);
    render();
    const response = await fetch('/api', {method: 'POST', headers: {'Content-Type': 'application/json', 'X-Inspector-Token': token}, body: JSON.stringify(body), signal: AbortSignal.timeout(60000)});
    const payload = await response.json();
    showCommands(payload.commands || []);
    if (!payload.ok) throw Object.assign(new Error(payload.error.message), payload.error);
    if (payload.view) state.view = payload.view;
    if (payload.revision) state.revision = payload.revision;
    apply?.(payload);
    byId('status').textContent = state.view ? 'Ready. Select an element or explore a branch.' : byId('window').value ? 'Window ready. Select Inspect app to begin.' : 'Choose an app with an open window.';
    finishFeedback(true, body.operation === 'find' ? resultInfo : 'Response received.');
  } catch (error) {
    const code = typeof error.code === 'string' ? error.code : null;
    if (!code || error.refreshRequired || ['STALE_VIEW', 'STALE_REF', 'WINDOW_NOT_FOUND', 'APP_NOT_FOUND', 'ELEMENT_NOT_FOUND', 'AMBIGUOUS_TARGET'].includes(code)) state.invalid = true;
    byId('error').textContent = `${code ? `${code}: ` : ''}${error.message}${error.detail?.suggestion ? ` ${error.detail.suggestion}` : ''}${state.invalid && state.view ? ' Refresh the app before continuing.' : ''}`;
    byId('error').hidden = false;
    byId('status').textContent = 'Could not complete the read. Your app was not clicked or edited.';
    finishFeedback(false, error.message);
  } finally { state.busy = false; render(); }
}

function select(key) {
  if (state.busy) return;
  state.selectedKey = key;
  highlightMatches = false;
  byId('highlights').checked = true;
  for (const parent of ancestors(indexTree(state.view.tree), key)) state.open.add(parent);
  byId('read-result').textContent = '';
  render();
  const row = [...byId('tree').querySelectorAll('.tree-row')].find(row => row.dataset.key === key);
  revealTreeRow(byId('tree'), row);
  row?.querySelector('.tree-select')?.focus({preventScroll: true});
}

async function expand(ref) {
  await perform({operation: 'expand', ref, revision: state.revision}, 'Loading children from the app…', payload => {
    state.matches = [];
    highlightMatches = false;
    resultInfo = '';
    state.selectedKey = payload.selectedRef || indexTree(state.view.tree)[0].key;
    state.open.add(state.selectedKey);
    for (const parent of ancestors(indexTree(state.view.tree), state.selectedKey)) state.open.add(parent);
  });
}

function renderSelection(item) {
  const node = item?.node;
  const available = !state.busy && !state.invalid;
  byId('selected-name').textContent = node ? node.name || node.description || node.role : 'Select an element';
  byId('selected-role').textContent = node ? `Role: ${node.role}` : 'Its details and available reads will appear here.';
  byId('selected-ref').textContent = node?.ref_id || (node ? 'Context node · no ref' : '');
  byId('selected-states').textContent = node?.states?.length ? node.states.join(' · ') : '';
  byId('expand').disabled = !available || !canExpand(node);
  byId('expand').textContent = node?.children_count || node?.subtree_truncated ? 'Load children' : 'Reload branch';
  byId('scope-note').textContent = !node ? 'Select a node in the tree or screenshot.' : state.invalid ? 'Refresh to restore live reads.' : node.ref_id ? 'Find searches inside this element. Reads target this ref.' : item.parentKey === null ? 'Find searches this window. Select a child ref for property reads.' : 'This context node has no ref. Select a ref-addressable parent to search or load more.';
  byId('find').disabled = !available || !canSearch();
  byId('read').disabled = !available || !node?.ref_id;
  const {children, identity_key, ...details} = node || {};
  byId('element-details').textContent = node ? JSON.stringify(details, null, 2) : 'No selection';
}

function renderResults() {
  byId('result-count').textContent = resultInfo;
  byId('results').replaceChildren();
  for (const match of state.matches) {
    const button = document.createElement('button');
    button.className = 'result';
    button.type = 'button';
    button.textContent = match.name || match.role;
    const detail = document.createElement('small');
    detail.textContent = `${match.role}${match.treeRef ? ' · reveal in tree' : ' · not present in the loaded tree'}`;
    button.append(detail);
    button.disabled = state.busy || !match.treeRef;
    button.onclick = () => select(match.treeRef);
    if (!match.treeRef) button.title = 'No unique identity match in the captured tree; no guessed highlight is shown.';
    byId('results').append(button);
  }
}

function render() {
  const items = indexTree(state.view?.tree);
  const item = items.find(item => item.key === state.selectedKey);
  byId('tree-empty').hidden = Boolean(state.view);
  byId('node-count').textContent = state.view ? `${items.length} loaded` : '';
  for (const id of ['app', 'window', 'reload-apps', 'find-name', 'find-field', 'find-role', 'find-exact', 'read-kind', 'property']) byId(id).disabled = state.busy;
  byId('load').disabled = state.busy || !byId('app').value || !byId('window').value;
  byId('load').textContent = state.view && state.view.app === byId('app').value ? 'Refresh app' : 'Inspect app';
  renderTree(byId('tree'), state, {select, expand, render});
  renderSelection(item);
  renderResults();
  byId('show-all').disabled = state.busy || !state.view;
  document.querySelector('.workspace').setAttribute('aria-busy', String(state.busy));
  byId('roles').replaceChildren(...[...new Set(items.map(item => item.node.role))].sort().map(role => new Option(role, role)));
  byId('preview-hint').textContent = highlightMatches ? 'Showing search matches. Select a result to isolate it.' : state.selectedKey ? 'Showing only the selected element. Show all restores the overview.' : 'Select a highlight to inspect it. This never clicks the app.';
  renderScreenshot(state.view, highlightMatches ? null : state.selectedKey, items, highlightMatches ? new Set(state.matches.map(match => match.treeRef).filter(Boolean)) : null, select);
}

function setProperties() {
  byId('property').replaceChildren(...properties[byId('read-kind').value].map(name => new Option(name, name)));
}

async function loadApps() {
  const previous = byId('app').value;
  await perform({operation: 'apps'}, 'Reading running applications…', payload => {
    const names = [...new Set((payload.apps || []).filter(app => app.presentation !== 'background').map(app => app.name))].sort((a, b) => a.localeCompare(b));
    byId('app').replaceChildren(new Option('Choose a running app…', ''), ...names.map(name => new Option(name, name)));
    if (names.includes(previous)) byId('app').value = previous;
  });
}

byId('app').onchange = async () => {
  if (state.busy) return;
  state.view = null;
  state.selectedKey = null;
  state.matches = [];
  resultInfo = '';
  state.open.clear();
  byId('window').replaceChildren();
  byId('window-field').hidden = true;
  if (!byId('app').value) { render(); return; }
  await perform({operation: 'windows', app: byId('app').value}, 'Reading app windows…', payload => {
    const windows = (payload.windows || []).filter(window => window.accessible !== false);
    windows.sort((a, b) => Number(b.is_focused) - Number(a.is_focused));
    byId('window').replaceChildren(...windows.map(window => new Option(window.title || window.id, window.id)));
    byId('window-field').hidden = windows.length <= 1;
    if (!windows.length) throw new Error('This app has no accessible window. Open a window, then choose the app again.');
  });
};
byId('reload-apps').onclick = loadApps;
byId('app-form').onsubmit = async event => {
  event.preventDefault();
  const app = byId('app').value;
  await perform({operation: 'snapshot', app, windowId: byId('window').value}, 'Capturing the skeleton and window…', payload => {
    state.invalid = false;
    state.matches = [];
    highlightMatches = false;
    resultInfo = '';
    state.selectedKey = indexTree(payload.view.tree)[0].key;
    state.open = new Set([state.selectedKey]);
  });
};
byId('expand').onclick = () => { const ref = selected()?.node.ref_id; if (ref) expand(ref); };
byId('find-form').onsubmit = async event => {
  event.preventDefault();
  if (!canSearch() || state.invalid) return;
  await perform({operation: 'find', revision: state.revision, ...scoped(), name: byId('find-name').value.trim(), field: byId('find-field').value, role: byId('find-role').value.trim(), exact: byId('find-exact').checked}, 'Finding matches and revealing the selected region…', payload => {
    state.matches = payload.matches;
    highlightMatches = true;
    resultInfo = payload.matches.length ? `${payload.matches.length} of ${payload.totalMatches} matches${payload.truncated ? ' · first 20 shown' : ''}` : 'No matches in this selection. Try a different search field, term, or role.';
    state.selectedKey = payload.selectedRef || indexTree(state.view.tree)[0].key;
    const items = indexTree(state.view.tree);
    for (const match of state.matches) for (const parent of ancestors(items, match.treeRef)) state.open.add(parent);
  });
};
byId('read-kind').onchange = setProperties;
byId('read-form').onsubmit = async event => {
  event.preventDefault();
  const ref = selected()?.node.ref_id;
  if (!ref || state.invalid) return;
  await perform({operation: byId('read-kind').value, revision: state.revision, ref, property: byId('property').value}, 'Reading the element from the app…', payload => {
    byId('read-result').textContent = JSON.stringify(payload.result, null, 2);
  });
};
byId('highlights').onchange = render;
byId('show-all').onclick = () => {
  state.selectedKey = null;
  state.matches = [];
  highlightMatches = false;
  resultInfo = '';
  byId('highlights').checked = true;
  render();
};
byId('toast-dismiss').onclick = () => { byId('toast').hidden = true; };

const workspace = document.querySelector('.workspace');
const MIN_PANE = 180;
const MAX_PANE = 640;

function applyPanes() {
  const leftHidden = localStorage.getItem('inspector-left') === 'hidden';
  const rightHidden = localStorage.getItem('inspector-right') === 'hidden';
  workspace.classList.toggle('left-hidden', leftHidden);
  workspace.classList.toggle('right-hidden', rightHidden);
  byId('tree-pane').hidden = leftHidden;
  byId('inspect-pane').hidden = rightHidden;
  byId('resize-left').hidden = leftHidden;
  byId('resize-right').hidden = rightHidden;
  byId('show-left').hidden = !leftHidden;
  byId('show-right').hidden = !rightHidden;
  for (const key of ['left-w', 'right-w']) {
    const stored = Number(localStorage.getItem(`inspector-${key}`));
    if (stored >= MIN_PANE) workspace.style.setProperty(`--${key}`, `${stored}px`);
  }
}

function setupToggle(hideId, showId, side) {
  byId(hideId).onclick = () => { localStorage.setItem(`inspector-${side}`, 'hidden'); applyPanes(); };
  byId(showId).onclick = () => { localStorage.setItem(`inspector-${side}`, 'open'); applyPanes(); };
}

function setupResize(handleId, side) {
  const handle = byId(handleId);
  let startX = 0;
  let startWidth = 0;
  let pointerId = null;
  const move = event => {
    if (event.pointerId !== pointerId) return;
    const delta = side === 'left' ? event.clientX - startX : startX - event.clientX;
    const width = Math.min(MAX_PANE, Math.max(MIN_PANE, startWidth + delta));
    workspace.style.setProperty(`--${side}-w`, `${width}px`);
  };
  const stop = () => {
    if (pointerId === null) return;
    const captured = pointerId;
    pointerId = null;
    if (handle.hasPointerCapture(captured)) handle.releasePointerCapture(captured);
    handle.classList.remove('dragging');
    document.body.style.userSelect = '';
    localStorage.setItem(`inspector-${side}-w`, parseInt(getComputedStyle(workspace).getPropertyValue(`--${side}-w`), 10) || '');
  };
  handle.onpointermove = move;
  handle.onpointerup = stop;
  handle.onpointercancel = stop;
  handle.onlostpointercapture = stop;
  handle.onpointerdown = event => {
    if (pointerId !== null || event.button !== 0) return;
    event.preventDefault();
    handle.setPointerCapture(event.pointerId);
    pointerId = event.pointerId;
    startX = event.clientX;
    startWidth = byId(side === 'left' ? 'tree-pane' : 'inspect-pane').getBoundingClientRect().width;
    handle.classList.add('dragging');
    document.body.style.userSelect = 'none';
  };
  handle.onkeydown = event => {
    const step = event.shiftKey ? 40 : 16;
    const current = parseInt(getComputedStyle(workspace).getPropertyValue(`--${side}-w`), 10) || (side === 'left' ? 280 : 310);
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault();
    const next = Math.min(MAX_PANE, Math.max(MIN_PANE, current + (event.key === 'ArrowLeft' ? -step : step) * (side === 'left' ? 1 : -1)));
    workspace.style.setProperty(`--${side}-w`, `${next}px`);
    localStorage.setItem(`inspector-${side}-w`, String(next));
  };
}

applyPanes();
setupToggle('hide-left', 'show-left', 'left');
setupToggle('hide-right', 'show-right', 'right');
setupResize('resize-left', 'left');
setupResize('resize-right', 'right');
setProperties();
render();
await loadApps();
