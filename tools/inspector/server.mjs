import {access, mkdtemp, rm} from 'node:fs/promises';
import {constants, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {randomBytes} from 'node:crypto';
import {parseArgs} from 'node:util';
import {spawn} from 'node:child_process';
import {createCli} from './cli.mjs';
import {createInspector} from './inspector.mjs';
import {createBridge} from './bridge.mjs';
import {listenAvailable} from './http.mjs';

const {values} = parseArgs({options: {port: {type: 'string'}, bin: {type: 'string'}, 'no-open': {type: 'boolean'}}});
const port = Number(values.port ?? process.env.PORT ?? 4317);
const defaultBinary = fileURLToPath(new URL(`../../target/release/agent-desktop${process.platform === 'win32' ? '.exe' : ''}`, import.meta.url));
const binary = resolve(values.bin || process.env.AGENT_DESKTOP_BIN || defaultBinary);
let home;
try {
  await access(binary, constants.X_OK);
  home = await mkdtemp(join(tmpdir(), 'agent-desktop-inspector-'));
  process.on('exit', () => {
    try { rmSync(home, {recursive: true, force: true}); }
    catch (error) { console.error(`Temporary state cleanup failed: ${error.message}`); }
  });
  const token = randomBytes(32).toString('hex');
  const server = createBridge(createInspector(createCli(binary, home)), token);
  const actualPort = await listenAvailable(server, port);
  const url = `http://127.0.0.1:${actualPort}/#token=${token}`;
  console.log(`agent-desktop inspector: ${url}`);
  console.log('Read-only. Screenshots and labels are sensitive. Press Ctrl+C to stop.');
  let closing = false;
  const stop = () => {
    if (closing) return;
    closing = true;
    server.close();
    server.closeAllConnections();
  };
  process.on('SIGINT', stop);
  process.on('SIGTERM', stop);
  if (!values['no-open']) {
    const opener = process.platform === 'darwin' ? '/usr/bin/open' : process.platform === 'linux' ? 'xdg-open' : null;
    if (opener) {
      const child = spawn(opener, [url], {stdio: 'ignore', shell: false});
      child.on('error', () => console.error('Open the localhost URL above in your browser.'));
      child.unref();
    }
  }
} catch (error) {
  if (home) await rm(home, {recursive: true, force: true});
  console.error(error.code === 'ENOENT' ? 'Build the CLI first: cargo build --release -p agent-desktop (or pass --bin PATH).' : error.message);
  process.exitCode = 1;
}
