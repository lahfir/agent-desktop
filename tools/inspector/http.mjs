import {timingSafeEqual} from 'node:crypto';

const operations = {
  apps: [], windows: ['app'], snapshot: ['app', 'windowId'],
  expand: ['revision', 'ref'], find: ['revision', 'ref', 'name', 'field', 'role', 'exact'],
  get: ['revision', 'ref', 'property'], is: ['revision', 'ref', 'property'],
};
const properties = {
  get: ['text', 'value', 'title', 'bounds', 'role', 'states'],
  is: ['visible', 'enabled', 'checked', 'focused', 'expanded', 'selected'],
};

export function fail(message, status = 400, code = 'INVALID_ARGS') {
  return Object.assign(new Error(message), {status, code});
}

export function validateRequest(body) {
  if (!body || typeof body !== 'object' || Array.isArray(body) || !Object.hasOwn(operations, body.operation)) {
    throw fail('Unsupported inspector operation');
  }
  const allowed = operations[body.operation];
  if (Object.keys(body).some(key => key !== 'operation' && !allowed.includes(key))) throw fail('Unknown request field');
  for (const key of ['app', 'windowId', 'ref', 'name', 'role', 'property']) {
    if (body[key] !== undefined && (typeof body[key] !== 'string' || body[key].length > 512 || /[\x00-\x1f]/.test(body[key]))) {
      throw fail(`Invalid ${key}`);
    }
  }
  if (['windows', 'snapshot'].includes(body.operation) && (!body.app?.trim() || body.app.startsWith('-'))) throw fail('Choose an application');
  if (body.windowId !== undefined && !body.windowId) throw fail('Choose a window');
  if (body.ref !== undefined && !/^@s[a-z0-9]+:e[1-9][0-9]*$/.test(body.ref)) throw fail('Expected a qualified snapshot ref');
  if (['expand', 'get', 'is'].includes(body.operation) && !body.ref) throw fail('Select a ref-addressable element');
  if (allowed.includes('revision') && (!Number.isSafeInteger(body.revision) || body.revision < 1)) throw fail('Refresh the inspector first');
  if (body.exact !== undefined && typeof body.exact !== 'boolean') throw fail('Invalid exact flag');
  if (body.field !== undefined && !['name', 'value', 'text'].includes(body.field)) throw fail('Choose Name, Value, or Text');
  if (body.operation === 'find' && !body.name?.trim() && !body.role?.trim()) throw fail('Enter a search term or role to find');
  if (body.role && !/^[a-z][a-z0-9-]{0,63}$/.test(body.role)) throw fail('Use a canonical role such as button or textfield');
  if (properties[body.operation] && !properties[body.operation].includes(body.property)) throw fail('Unsupported property');
  return body;
}

export function authorize(request, port, token) {
  const host = `127.0.0.1:${port}`;
  const supplied = Buffer.from(request.headers['x-inspector-token'] || '');
  const expected = Buffer.from(token);
  if (request.headers.host !== host || request.headers.origin !== `http://${host}` ||
      supplied.length !== expected.length || !timingSafeEqual(supplied, expected)) {
    throw fail('Request not authorized for this inspector', 403, 'FORBIDDEN');
  }
}

export async function readBody(request) {
  if (request.headers['content-type']?.split(';')[0] !== 'application/json') throw fail('Expected application/json', 415);
  let size = 0;
  const chunks = [];
  for await (const chunk of request) {
    size += chunk.length;
    if (size > 8192) throw fail('Request too large', 413);
    chunks.push(chunk);
  }
  try { return validateRequest(JSON.parse(Buffer.concat(chunks).toString('utf8'))); }
  catch (error) { if (error.status) throw error; throw fail('Invalid JSON'); }
}

export async function listenAvailable(server, startingPort = 4317) {
  if (!Number.isInteger(startingPort) || startingPort < 0 || startingPort > 65535) throw fail('Invalid port');
  for (let attempt = 0; attempt <= 20; attempt++) {
    const port = attempt === 20 ? 0 : startingPort === 0 ? 0 : Math.min(startingPort + attempt, 65535);
    try {
      await new Promise((resolve, reject) => {
        const onError = error => { server.off('listening', onListening); reject(error); };
        const onListening = () => { server.off('error', onError); resolve(); };
        server.once('error', onError).once('listening', onListening).listen(port, '127.0.0.1');
      });
      return server.address().port;
    } catch (error) {
      if (error.code !== 'EADDRINUSE' || attempt === 20) throw error;
    }
  }
}
