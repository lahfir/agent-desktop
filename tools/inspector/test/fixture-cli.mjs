import {mkdir, writeFile} from 'node:fs/promises';
import {join} from 'node:path';

const args = process.argv.slice(2);
if (args.includes('--cleanup-failure')) await mkdir(args[args.indexOf('--screenshot') + 1]);
if (args.includes('--deny-screen') && args.includes('--debug')) {
  console.log(JSON.stringify({ok: false, command: 'snapshot', error: {code: 'PERM_DENIED', message: 'Screen Recording required'}}));
  process.exitCode = 1;
} else if (args.includes('--fail')) {
  console.log(JSON.stringify({ok: false, command: 'snapshot', error: {code: 'STALE_REF', message: 'Refresh the snapshot'}}));
  process.exitCode = 1;
} else if (args.includes('--capture')) {
  const tree = {role: 'window', children: [{role: 'button', ref_id: '@sone:e1', name: 'Save'}]};
  const data = {tree, window: {id: 'w-1'}, snapshot_id: 'sone', complete: true};
  const directory = join(process.env.AGENT_DESKTOP_HOME, 'snapshots', 'sone');
  await mkdir(directory, {recursive: true});
  await writeFile(join(directory, 'refmap.json'), JSON.stringify({inner: {'@e1': {pid: 1, role: 'button', name: 'Save', path: [0]}}}));
  if (args.includes('--warning')) {
    data.debug = {warning: 'Screenshot unavailable'};
  } else if (args.includes('--debug')) {
    const nodes = [{role: 'window', kind: 'root'}, {role: 'button', ref_id: '@sone:e1', kind: 'action', reason: 'Addressable'}];
    const artifact = {after: {window: data.window, nodes: args.includes('--mismatch') ? [] : nodes}};
    const path = args[args.indexOf('--screenshot') + 1];
    await writeFile(path, `<script id="debug-data" type="application/json">${JSON.stringify(artifact)}</script>`);
  }
  console.log(JSON.stringify({ok: true, command: 'snapshot', data}));
} else {
  console.log(JSON.stringify({ok: true, command: 'fixture', data: {args, stateRoot: process.env.AGENT_DESKTOP_HOME, session: process.env.AGENT_DESKTOP_SESSION || null}}));
}
