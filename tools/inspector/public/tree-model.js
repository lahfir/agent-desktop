export function indexTree(tree) {
  const items = [];
  function visit(node, path, parentKey) {
    const key = node.ref_id || `node:${path}`;
    items.push({node, key, parentKey});
    (node.children || []).forEach((child, index) => visit(child, `${path}.${index}`, key));
  }
  if (tree) visit(tree, '0', null);
  return items;
}

export function replaceBranch(tree, ref, replacement) {
  if (tree.ref_id === ref) return replacement;
  return {...tree, children: (tree.children || []).map(child => replaceBranch(child, ref, replacement))};
}

export function findIdentity(tree, identity) {
  if (!identity) return null;
  const matches = indexTree(tree).filter(item => item.node.identity_key === identity);
  return matches.length === 1 ? matches[0] : null;
}

export function findContextMatch(tree, match) {
  const fields = ['x', 'y', 'width', 'height'];
  if (!match.bounds || !fields.every(field => Number.isFinite(match.bounds[field])) || match.bounds.width <= 0 || match.bounds.height <= 0) return null;
  const matches = indexTree(tree).filter(({node}) => node.role === match.role && node.bounds && fields.every(field => node.bounds[field] === match.bounds[field]));
  return matches.length === 1 ? matches[0] : null;
}

export function highlightItems(items, selection, matchRefs) {
  return items.filter(item => {
    if (item.node.states?.some(state => state === 'hidden' || state === 'offscreen')) return false;
    if (selection !== null) return item.key === selection;
    return matchRefs === null || matchRefs.has(item.node.ref_id || item.key);
  });
}

export function ancestors(items, key) {
  const lookup = new Map(items.map(item => [item.key, item]));
  const result = [];
  let item = lookup.get(key);
  while (item?.parentKey) { result.push(item.parentKey); item = lookup.get(item.parentKey); }
  return result;
}
