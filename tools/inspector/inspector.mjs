import {fail, validateRequest} from './http.mjs';
import {identityKey} from './cli.mjs';
import {indexTree, replaceBranch, findIdentity, findContextMatch} from './public/tree-model.js';

export function createInspector(cli) {
  let view = null;
  let revision = 0;
  let searchRefs = new Set();

  function requireCurrent(body) {
    if (!view || body.revision !== revision) throw fail('This view is out of date. Refresh the window.', 409, 'STALE_VIEW');
    if (body.ref && !indexTree(view.tree).some(item => item.node.ref_id === body.ref) && !searchRefs.has(body.ref)) {
      throw fail('Ref does not belong to the current inspector view', 409, 'STALE_REF');
    }
  }

  function scopeArgs(ref) {
    return ref ? ['--root', ref] : [`--app=${view.app}`, `--window-id=${view.window.id}`];
  }

  async function observe(ref, depth, log) {
    if (ref && !indexTree(view.tree).some(item => item.node.ref_id === ref)) throw fail('Expand a node in the loaded tree', 409);
    const args = ['snapshot', ...scopeArgs(ref), '--max-depth', String(depth)];
    try {
      const next = await cli.capture(args, log);
      if (next.window.id !== view.window.id) throw fail('Source window changed. Refresh.', 409, 'STALE_VIEW');
      view = {...view, ...next, tree: ref ? replaceBranch(view.tree, ref, next.tree) : next.tree};
      searchRefs.clear();
      revision++;
      return next.tree.ref_id || null;
    } catch (error) {
      view = null;
      revision++;
      error.refreshRequired = true;
      throw error;
    }
  }

  async function execute(body) {
    validateRequest(body);
    const log = [];
    try {
      if (body.operation === 'apps') {
        const windows = await cli.run(['list-windows'], log);
        const names = [...new Set(windows.filter(window => window.accessible !== false).map(window => window.app_name).filter(Boolean))];
        return {apps: names.map(name => ({name})), commands: log};
      }
      if (body.operation === 'windows') return {windows: await cli.run(['list-windows', `--app=${body.app}`], log), commands: log};
      if (body.operation === 'snapshot') {
        const args = ['snapshot', `--app=${body.app}`, '--skeleton'];
        if (body.windowId) args.push(`--window-id=${body.windowId}`);
        const next = await cli.capture(args, log);
        view = {...next, app: body.app};
        revision++;
        searchRefs.clear();
        return {view, revision, commands: log};
      }
      requireCurrent(body);
      if (body.operation === 'expand') {
        const selectedRef = await observe(body.ref, 3, log);
        return {view, revision, selectedRef, commands: log};
      }
      if (body.operation === 'find') {
        const args = ['find', ...scopeArgs(body.ref), '--limit', '20'];
        if (body.name) args.push(`--${body.field || 'text'}=${body.name}`);
        if (body.role) args.push(`--role=${body.role}`);
        if (body.exact) args.push('--exact');
        const result = await cli.run(args, log);
        const matches = result.matches || [];
        const saved = await cli.entries(matches.flatMap(match => match.ref_id ? [match.ref_id] : []));
        let selectedRef = body.ref || null;
        if (matches.length) selectedRef = await observe(body.ref, 10, log);
        searchRefs = new Set(saved.keys());
        const mapped = matches.map(match => {
          const entry = saved.get(match.ref_id);
          const target = match.ref_id ? (entry ? findIdentity(view.tree, identityKey(entry)) : null) : findContextMatch(view.tree, match);
          return {...match, treeRef: target?.key || null};
        });
        return {view, revision, selectedRef, matches: mapped,
          totalMatches: result.total_matches, truncated: result.truncated, commands: log};
      }
      const result = await cli.run([body.operation, body.ref, '--property', body.property], log);
      return {result, revision, commands: log};
    } catch (error) { error.commands = log; throw error; }
  }

  return {execute};
}
