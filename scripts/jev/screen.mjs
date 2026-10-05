/**
 * Reading a screen: which elements exist, which of them a command can reach,
 * and how each one is described. Shared by the single-step resolver and the
 * loop, so both judge the same screen the same way.
 */

const quoted = (s) => (s ? ` "${s}"` : "");

export const collect = (tree) => {
  const found = [];
  const walk = (node, path, parentRole) => {
    const self = `${node.role}${quoted(node.name)}`;
    if (node.ref_id) found.push({ ...node, path, parentRole });
    const next = node.children?.length ? [...path, self] : path;
    for (const child of node.children ?? []) walk(child, next, node.role);
  };
  walk(tree, [], null);
  return found;
};

/** A sheet, menu or alert owns input while it is up, and the window tree marks
 *  its elements offscreen. Reading the surface is the only way to act on it. */
export const outsideWindow = (data, windowId) => {
  const ids = [data?.window?.id, data?.surface?.window_id].filter((id) => id !== undefined && id !== null);
  return ids.length === 0 || ids.some((id) => id !== windowId);
};

export const overlayRole = (tree) => {
  let found = null;
  const walk = (n) => {
    if (!found && ["sheet", "alert", "menu", "popover"].includes(n.role)) found = n.role;
    for (const c of n.children ?? []) walk(c);
  };
  walk(tree);
  return found;
};

/**
 * The only elements withheld are ones no command can reach. Everything else is
 * offered: the docs are explicit that a Choice does better with the full list
 * than a shortlist, and an unnamed row is still distinguishable by the value it
 * holds.
 *
 * An element that advertises no action is the one that must go. A sidebar
 * category in Numbers publishes its label on a cell and its behaviour on the
 * row around it, and both carry the same name, so the inert cell wins the
 * choice about half the time and every such win ends in POLICY_DENIED. Nothing
 * is lost by withholding it: the row beside it is the element that acts.
 */
export const offerable = (refs) =>
  refs.filter((n) => {
    const s = n.states ?? [];
    if (s.includes("disabled") || s.includes("hidden")) return false;
    if (!n.available_actions?.length) return false;
    return !(n.role === "cell" && n.parentRole === "treeitem");
  });

/**
 * Structured criteria disambiguate better than a flat sentence. Position is
 * spent only on an element with no name, no description and no value: a form
 * field in a PDF carries none of those, and where it sits is the only thing
 * that tells one from the next.
 */
export const describe = (node, rich) => {
  const d = {
    what: `${node.role}${quoted(node.name ?? node.description)}`,
    where: node.path.length ? node.path.join(" > ") : "top level",
  };
  if (node.value != null && node.value !== "") d.holds = String(node.value).slice(0, 120);
  if (node.states?.length) d.state = node.states.join(", ");
  if (!d.what.includes('"') && node.bounds) {
    d.at = `x ${Math.round(node.bounds.x)}, y ${Math.round(node.bounds.y)}`;
  }
  if (rich) {
    if (node.available_actions?.length) d.supports = node.available_actions.join(", ");
    if (node.children_count) d.contains = `${node.children_count} items not shown`;
  }
  return d;
};

export const label = (n) => `${n.role}${quoted(n.name ?? n.description)}`;
