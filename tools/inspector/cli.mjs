import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {readFile, stat, unlink} from 'node:fs/promises';
import {join} from 'node:path';
import {randomUUID} from 'node:crypto';
import {fail} from './http.mjs';

const execute = promisify(execFile);

export function identityKey(entry) {
  return JSON.stringify([entry.pid, entry.process_instance, entry.source_window_id,
    entry.source_surface, entry.path || [], entry.role, entry.name || '', entry.native_id || null, entry.bounds || null]);
}

export function createCli(binary, home) {
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('AGENT_DESKTOP_')));
  env.AGENT_DESKTOP_HOME = home;

  async function run(args, log) {
    const started = Date.now();
    let stdout;
    let processError;
    try {
      ({stdout} = await execute(binary, args, {env, timeout: 20000, killSignal: 'SIGKILL', maxBuffer: 4 * 1024 * 1024, shell: false}));
    } catch (error) { stdout = error.stdout; processError = error; }
    let envelope;
    try { envelope = JSON.parse(stdout); } catch {
      const message = processError?.code === 'ENOENT' ? 'CLI binary not found. Run cargo build --release -p agent-desktop first.' : 'CLI did not return a valid JSON response (timeout, output limit, or incompatible binary).';
      throw fail(message, 502, 'CLI_FAILED');
    }
    log?.push({stateRoot: home, binary, args: args.map((arg, index) => args[index - 1] === '--screenshot' ? '<temporary-capture.html>' : arg), command: envelope.command, ok: envelope.ok, elapsedMs: Date.now() - started});
    if (!envelope.ok) {
      throw Object.assign(fail(envelope.error?.message || 'CLI command failed', 422, envelope.error?.code || 'CLI_FAILED'), {detail: envelope.error});
    }
    if (processError) throw fail('CLI exited unsuccessfully after emitting a response', 502, 'CLI_FAILED');
    return envelope.data;
  }

  async function entries(refs) {
    const maps = new Map();
    const result = new Map();
    for (const ref of refs) {
      const match = /^@(s[a-z0-9]+):(e[1-9][0-9]*)$/.exec(ref);
      if (!match) throw fail('CLI returned an invalid ref', 502);
      if (!maps.has(match[1])) {
        const file = join(home, 'snapshots', match[1], 'refmap.json');
        if ((await stat(file)).size > 1048576) throw fail('Refmap exceeds its size limit', 502);
        maps.set(match[1], JSON.parse(await readFile(file, 'utf8')).inner);
      }
      const entry = maps.get(match[1])?.[`@${match[2]}`];
      if (entry) result.set(ref, entry);
    }
    return result;
  }

  async function capture(args, log) {
    const path = join(home, `capture-${randomUUID()}.html`);
    try {
      let data;
      try { data = await run([...args, '--include-bounds', '--debug', '--screenshot', path], log); }
      catch (error) {
        if (error.code !== 'PERM_DENIED') throw error;
        data = await run([...args, '--include-bounds'], log);
        data.debug = {warning: 'Screenshot permission unavailable. Showing the accessibility tree only.'};
      }
      let artifact = {};
      if (!data.debug?.warning) {
        if ((await stat(path)).size > 128 * 1024 * 1024) throw fail('Capture exceeds its size limit', 502);
        const html = await readFile(path, 'utf8');
        const island = html.split('<script id="debug-data" type="application/json">')[1]?.split('</script>')[0];
        if (!island) throw fail('CLI debug format is incompatible; rebuild the repository binary', 502);
        artifact = JSON.parse(island);
      }
      const nodes = [];
      const walk = node => { nodes.push(node); for (const child of node.children || []) walk(child); };
      walk(data.tree);
      const saved = await entries(nodes.flatMap(node => node.ref_id ? [node.ref_id] : []));
      const visuals = artifact.after?.nodes || [];
      if (artifact.after && (visuals.length !== nodes.length || nodes.some((node, index) =>
        node.role !== visuals[index].role || node.ref_id !== visuals[index].ref_id))) {
        throw fail('CLI debug tree is incompatible with the snapshot; rebuild the repository binary', 502);
      }
      nodes.forEach((node, index) => {
        if (saved.has(node.ref_id)) node.identity_key = identityKey(saved.get(node.ref_id));
        node.kind = visuals[index]?.kind || (node.ref_id ? 'action' : 'context');
        node.reason = visuals[index]?.reason || '';
      });
      return {tree: data.tree, window: data.window, snapshotId: data.snapshot_id,
        complete: data.complete, frame: artifact.after || null, warning: artifact.warning || data.debug?.warning || null};
    } finally { await unlink(path).catch(error => {
      if (error.code !== 'ENOENT') console.error(`Temporary capture cleanup failed: ${error.message}`);
    }); }
  }

  return {run, entries, capture};
}
