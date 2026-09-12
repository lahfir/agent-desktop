import {highlightItems} from './tree-model.js';

export function renderScreenshot(view, selection, items, matchRefs, onSelect) {
  const byId = id => document.getElementById(id);
  const frame = view?.frame;
  const svg = byId('overlay');
  svg.replaceChildren();
  byId('stage').hidden = !frame;
  byId('preview-empty').hidden = Boolean(frame);
  byId('capture-warning').textContent = view?.warning || (view?.complete === false ? 'Partial snapshot: some elements were not captured. Refresh to try again.' : '');
  if (!view) {
    byId('window-title').textContent = 'Window preview';
    byId('preview-empty').textContent = 'Choose an app, then select Inspect app.';
    return;
  }
  byId('window-title').textContent = view.window.title || view.app;
  if (!frame) {
    byId('preview-empty').textContent = 'Screenshot unavailable. You can still explore the tree.';
    return;
  }
  const image = byId('capture');
  const source = DebugViewerFilter.imageSource(frame.image);
  if (image.getAttribute('src') !== source) image.src = source;
  image.alt = `${view.app}: ${frame.window.title || 'window'}`;
  if (!byId('highlights').checked) return;
  const bounds = frame.window.bounds;
  svg.setAttribute('viewBox', `0 0 ${bounds.width} ${bounds.height}`);
  const nodes = highlightItems(items, selection, matchRefs);
  nodes.sort((a, b) => ((b.node.bounds?.width || 0) * (b.node.bounds?.height || 0)) - ((a.node.bounds?.width || 0) * (a.node.bounds?.height || 0)));
  for (const item of nodes) {
    const node = item.node;
    const rect = DebugViewerFilter.visibleRect(node.bounds, bounds);
    if (!rect) continue;
    const group = document.createElementNS('http://www.w3.org/2000/svg', 'g');
    group.setAttribute('class', `selectable ${node.kind || 'context'}${item.key === selection ? ' selection' : ''}${matchRefs?.has(node.ref_id) ? ' search-match' : ''}`);
    group.setAttribute('role', 'button');
    group.setAttribute('tabindex', '0');
    group.setAttribute('aria-label', `Inspect ${node.name || node.role}`);
    group.dataset.key = item.key;
    const box = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
    for (const [key, value] of Object.entries(rect)) box.setAttribute(key, value);
    const title = document.createElementNS('http://www.w3.org/2000/svg', 'title');
    title.textContent = `${node.name || node.role} · ${node.role}`;
    group.append(title, box);
    group.onclick = () => onSelect(item.key);
    group.onkeydown = event => {
      if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); onSelect(item.key); }
    };
    svg.append(group);
  }
}
