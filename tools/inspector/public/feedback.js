const quote = value => /\s/.test(value) ? JSON.stringify(value) : value;
const shellQuote = value => /^[a-zA-Z0-9_@%+=:,./-]+$/.test(value) ? value : `'${value.replaceAll("'", "'\\''")}'`;

export function formatCommand({args, stateRoot, binary = 'agent-desktop'}) {
  const prefix = stateRoot ? `env -u AGENT_DESKTOP_SESSION ${shellQuote(`AGENT_DESKTOP_HOME=${stateRoot}`)} ` : '';
  return `${prefix}${[binary, ...args].map(shellQuote).join(' ')}`;
}

export function commandPreview(body) {
  const scope = body.ref ? ['--root', body.ref] : body.app ? [`--app=${body.app}`] : [];
  let args;
  switch (body.operation) {
    case 'apps': args = ['list-windows']; break;
    case 'windows': args = ['list-windows', ...scope]; break;
    case 'snapshot': args = ['snapshot', ...scope, ...(body.windowId ? [`--window-id=${body.windowId}`] : []), '--skeleton']; break;
    case 'expand': args = ['snapshot', ...scope, '--max-depth', '3']; break;
    case 'find': args = ['find', ...scope, ...(!body.ref && body.windowId ? [`--window-id=${body.windowId}`] : []), ...(body.name ? [`--${body.field || 'text'}=${body.name}`] : []), ...(body.role ? [`--role=${body.role}`] : []), ...(body.exact ? ['--exact'] : [])]; break;
    default: args = [body.operation, body.ref, '--property', body.property];
  }
  return `agent-desktop ${args.map(quote).join(' ')}`;
}

let timer;
export function startFeedback(body, message) {
  clearTimeout(timer);
  const toast = document.getElementById('toast');
  toast.hidden = false;
  toast.dataset.state = 'running';
  document.getElementById('toast-title').textContent = 'Running command';
  document.getElementById('toast-command').textContent = commandPreview(body);
  document.getElementById('toast-message').textContent = body.operation === 'find' ? 'Searching, then loading the matching region for display…' : message;
}

export function finishFeedback(ok, message) {
  const toast = document.getElementById('toast');
  toast.dataset.state = ok ? 'success' : 'error';
  document.getElementById('toast-title').textContent = ok ? 'Done' : 'Could not complete';
  document.getElementById('toast-message').textContent = message;
  timer = setTimeout(() => { toast.hidden = true; }, ok ? 3000 : 7000);
}
