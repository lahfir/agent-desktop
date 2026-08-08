#!/usr/bin/env node

const { execFileSync } = require('child_process');
const { readFileSync } = require('fs');
const { join } = require('path');

const root = join(__dirname, '..');
const npmDir = join(root, 'npm');
const pkg = require(join(npmDir, 'package.json'));
const releaseWorkflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8');
const expectedFiles = [
  'bin/agent-desktop.js',
  'package.json',
  'scripts/postinstall.js',
];

const NPM_PUBLISH_JOB = 'publish-npm';

// Returns the YAML block for one job, so a requirement can be asserted about
// the job that needs it rather than about the file that contains it.
//
// `release.yml` grants `id-token: write` twice: once to `publish-github` for
// the Sigstore OIDC exchange, and once to `publish-npm` for npm trusted
// publishing. A whole-file substring test is satisfied by the Sigstore grant
// alone, so deleting npm's grant - which breaks publishing outright - left
// this script reporting success. Every positive requirement below is therefore
// scoped to the job it is a requirement of.
// Line endings are normalised because a Windows clone with `core.autocrlf`
// checks this workflow out with CRLF, which leaves every line ending in `\r`
// and no job name ever matching. The rules must read the same on every clone.
function jobBlock(workflow, jobName) {
  const lines = workflow.replace(/\r\n/g, '\n').split('\n');
  const start = lines.findIndex((line) => line === `  ${jobName}:`);
  if (start === -1) {
    return null;
  }
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((line) => /^ {2}\S/.test(line));
  return (end === -1 ? rest : rest.slice(0, end)).join('\n');
}

// The single owner of the workflow rules. The real check and `selfTest` both
// call it, so the fixtures drive the shipped rules rather than a copy of them
// that can drift.
function workflowViolations(workflow) {
  const violations = [];

  // Negative rules stay whole-file: a long-lived token anywhere in the release
  // workflow is the thing being banned, whichever job carries it.
  if (workflow.includes('secrets.NPM_TOKEN') || workflow.includes('NODE_AUTH_TOKEN')) {
    violations.push('release workflow must use npm trusted publishing, not long-lived npm tokens');
  }

  const publishNpm = jobBlock(workflow, NPM_PUBLISH_JOB);
  if (publishNpm === null) {
    violations.push(`release workflow must define a ${NPM_PUBLISH_JOB} job`);
    return violations;
  }

  const required = [
    ['id-token: write', `${NPM_PUBLISH_JOB} must grant id-token: write for npm trusted publishing`],
    ['package-manager-cache: false', `${NPM_PUBLISH_JOB} must disable package manager caching`],
    ['npm publish --access public', `${NPM_PUBLISH_JOB} must publish through the trusted-publishing command`],
  ];
  for (const [needle, message] of required) {
    if (!publishNpm.includes(needle)) {
      violations.push(message);
    }
  }
  return violations;
}

// Both directions are driven. The must-catch fixture is the exact shape that
// defeated the previous whole-file rules: a second job legitimately carrying
// `id-token: write` while the npm job has lost it.
function selfTest() {
  const sound = [
    'jobs:',
    '  publish-github:',
    '    permissions:',
    '      id-token: write        # Sigstore OIDC exchange',
    '    steps:',
    '      - run: echo sign',
    '  publish-npm:',
    '    permissions:',
    '      contents: read',
    '      id-token: write',
    '    steps:',
    '      - uses: actions/setup-node@v6',
    '        with:',
    '          package-manager-cache: false',
    '      - run: npm publish --access public',
    '  publish-skills:',
    '    steps:',
    '      - run: echo skills',
    '',
  ].join('\n');

  const failures = [];
  const expectCaught = (name, workflow, needle) => {
    const found = workflowViolations(workflow);
    if (!found.some((violation) => violation.includes(needle))) {
      failures.push(`self-test FAIL (missed): ${name} -> ${JSON.stringify(found)}`);
    }
  };

  if (workflowViolations(sound).length !== 0) {
    failures.push(
      `self-test FAIL (false positive): a sound workflow was rejected -> ${JSON.stringify(workflowViolations(sound))}`,
    );
  }
  expectCaught(
    'npm job loses id-token while another job keeps it',
    sound.replace('      contents: read\n      id-token: write\n', '      contents: read\n'),
    'id-token: write',
  );
  expectCaught(
    'npm job loses package-manager-cache',
    sound.replace('          package-manager-cache: false\n', ''),
    'package manager caching',
  );
  expectCaught(
    'npm job loses the publish command',
    sound.replace('      - run: npm publish --access public\n', ''),
    'trusted-publishing command',
  );
  expectCaught('the npm job is gone entirely', sound.replace('  publish-npm:', '  publish-other:'), 'must define a');
  expectCaught('a long-lived token appears', `${sound}\n          NODE_AUTH_TOKEN: x\n`, 'long-lived npm tokens');

  if (failures.length > 0) {
    throw new Error(`The npm release-workflow rules do not behave as documented:\n${failures.join('\n')}`);
  }
}

selfTest();

if (pkg.bin?.['agent-desktop'] !== 'bin/agent-desktop.js') {
  throw new Error('npm bin path must be bin/agent-desktop.js');
}

if (pkg.repository?.url !== 'git+https://github.com/lahfir/agent-desktop.git') {
  throw new Error('npm repository URL must be normalized for npm publish');
}

const workflowProblems = workflowViolations(releaseWorkflow);
if (workflowProblems.length > 0) {
  throw new Error(workflowProblems.join('\n'));
}

const output = execFileSync('npm', ['pack', '--dry-run', '--json'], {
  cwd: npmDir,
  encoding: 'utf8',
  env: {
    ...process.env,
    npm_config_cache: process.env.npm_config_cache || '/tmp/agent-desktop-npm-cache',
  },
});

const pack = JSON.parse(output)[0];
const actualFiles = pack.files.map((file) => file.path).sort();
const expected = [...expectedFiles].sort();

if (JSON.stringify(actualFiles) !== JSON.stringify(expected)) {
  throw new Error(`Unexpected npm package contents: ${actualFiles.join(', ')}`);
}

if (pack.bundled && pack.bundled.length > 0) {
  throw new Error(`npm package unexpectedly bundles dependencies: ${pack.bundled.join(', ')}`);
}

if (pack.unpackedSize > 25_000) {
  throw new Error(`npm package is unexpectedly large: ${pack.unpackedSize} bytes`);
}

console.log(`OK: npm package contains ${actualFiles.length} files, ${pack.unpackedSize} bytes unpacked`);
