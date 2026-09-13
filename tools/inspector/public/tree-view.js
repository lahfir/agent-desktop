export function revealTreeRow(container, row) {
  if (!row || !container.clientWidth) return;
  const viewport = container.getBoundingClientRect();
  const bounds = row.getBoundingClientRect();
  const left = bounds.left - viewport.left - 2;
  const top = bounds.top - viewport.top - 2;
  const right = left + Math.min(bounds.width, container.clientWidth - 4) - container.clientWidth + 4;
  const bottom = top + bounds.height - container.clientHeight + 4;
  container.scrollLeft += left < 0 ? left : Math.max(0, right);
  container.scrollTop += top < 0 ? top : Math.max(0, bottom);
}

export function renderTree(container, state, handlers) {
  const scrollTop = container.scrollTop;
  const scrollLeft = container.scrollLeft;
  container.replaceChildren();
  if (!state.view) return;
  const matched = new Set(state.matches.flatMap(match => match.treeRef ? [match.treeRef] : []));
  function branch(node, path) {
    const key = node.ref_id || `node:${path}`;
    const name = node.name || node.description || node.role;
    const li = document.createElement('li');
    const row = document.createElement('div');
    row.className = 'tree-row';
    row.dataset.key = key;
    row.classList.toggle('selected', state.selectedKey === key);
    row.classList.toggle('matched', matched.has(key));
    const children = node.children || [];
    const truncated = Boolean(node.children_count || node.subtree_truncated);
    if (children.length || truncated) {
      const toggle = document.createElement('button');
      toggle.type = 'button';
      toggle.className = 'tree-toggle';
      toggle.textContent = state.open.has(key) ? '▾' : '▸';
      toggle.setAttribute('aria-label', `${state.open.has(key) ? 'Collapse' : 'Expand'} ${name}`);
      toggle.setAttribute('aria-expanded', String(state.open.has(key)));
      toggle.disabled = state.busy || state.invalid || (!children.length && !node.ref_id);
      if (!children.length && !node.ref_id) toggle.title = 'No ref for further reads. Select a ref-addressable ancestor.';
      toggle.onclick = () => {
        if (state.open.has(key)) state.open.delete(key);
        else if (truncated && node.ref_id) { handlers.expand(node.ref_id); return; }
        else state.open.add(key);
        handlers.render();
        container.querySelectorAll('.tree-row').forEach(row => {
          if (row.dataset.key === key) row.querySelector('.tree-toggle')?.focus({preventScroll: true});
        });
      };
      row.append(toggle);
    } else {
      const spacer = document.createElement('span');
      spacer.className = 'tree-spacer';
      spacer.textContent = '·';
      spacer.setAttribute('aria-hidden', 'true');
      row.append(spacer);
    }
    const select = document.createElement('button');
    select.type = 'button';
    select.className = 'tree-select';
    select.disabled = state.busy;
    select.setAttribute('aria-pressed', String(state.selectedKey === key));
    select.title = `${name} (${node.role})`;
    const title = document.createElement('strong');
    title.textContent = name;
    const subtitle = document.createElement('small');
    subtitle.textContent = `${node.role}${node.children_count ? ` · ${node.children_count} children to load` : node.subtree_truncated ? ' · incomplete' : ''}`;
    select.append(title, subtitle);
    select.onclick = () => handlers.select(key);
    row.append(select);
    li.append(row);
    if (state.open.has(key) && children.length) {
      const list = document.createElement('ul');
      list.className = 'tree-list';
      children.forEach((child, index) => list.append(branch(child, `${path}.${index}`)));
      li.append(list);
    }
    return li;
  }
  const list = document.createElement('ul');
  list.className = 'tree-list';
  list.append(branch(state.view.tree, '0'));
  container.append(list);
  container.scrollTop = scrollTop;
  container.scrollLeft = scrollLeft;
}
