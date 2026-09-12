import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {authorize, fail, readBody} from './http.mjs';

const files = new Map([
  ['/', ['public/index.html', 'text/html']],
  ...['app.js', 'tree-model.js', 'tree-view.js', 'screenshot.js', 'feedback.js'].map(name => [`/${name}`, [`public/${name}`, 'text/javascript']]),
  ['/styles.css', ['public/styles.css', 'text/css']],
  ['/shared/viewer.css', ['../../src/visual_debug/viewer.css', 'text/css']],
  ['/shared/viewer_filter.js', ['../../src/visual_debug/viewer_filter.js', 'text/javascript']],
]);

export function createBridge(inspector, token) {
  let busy = false;
  const server = createServer(async (request, response) => {
    const port = server.address().port;
    const host = `127.0.0.1:${port}`;
    response.setHeader('Cache-Control', 'no-store');
    response.setHeader('X-Content-Type-Options', 'nosniff');
    response.setHeader('Referrer-Policy', 'no-referrer');
    response.setHeader('Content-Security-Policy', "default-src 'none'; script-src 'self'; style-src 'self'; img-src data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'");
    function send(status, body) {
      response.writeHead(status, {'Content-Type': 'application/json; charset=utf-8'});
      response.end(JSON.stringify(body));
    }
    try {
      if (request.headers.host !== host) throw fail('Invalid local host', 403, 'FORBIDDEN');
      const url = new URL(request.url, `http://${host}`);
      if (url.pathname === '/api' && request.method === 'POST') {
        authorize(request, port, token);
        const body = await readBody(request);
        if (busy) throw fail('Another operation is running; wait for it to finish.', 409, 'BUSY');
        busy = true;
        try { send(200, {ok: true, ...await inspector.execute(body)}); }
        finally { busy = false; }
        return;
      }
      if (request.method !== 'GET') throw fail('Method not allowed', 405);
      if (request.headers['sec-fetch-site'] === 'cross-site') throw fail('Open the inspector from its localhost URL', 403, 'FORBIDDEN');
      const asset = files.get(url.pathname);
      if (!asset) throw fail('Not found', 404);
      const content = await readFile(fileURLToPath(new URL(asset[0], import.meta.url)), 'utf8');
      response.writeHead(200, {'Content-Type': `${asset[1]}; charset=utf-8`});
      response.end(content);
    } catch (error) {
      send(error.status || 500, {ok: false, error: {
        code: error.code || 'INTERNAL', message: error.status ? error.message : 'Inspector failed to process the request.',
        ...(error.detail ? {detail: error.detail} : {}),
        ...(error.refreshRequired ? {refreshRequired: true} : {}),
      }, commands: error.commands || []});
    }
  });
  server.headersTimeout = 5000;
  server.requestTimeout = 10000;
  server.maxConnections = 20;
  return server;
}
