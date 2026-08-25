#!/usr/bin/env node

const { execFileSync } = require('child_process');
const { readFileSync } = require('fs');
const { join } = require('path');

const root = join(__dirname, '..');
const npmDir = join(root, 'npm');
const pkg = require(join(npmDir, 'package.json'));
const releaseWorkflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8');
const { PLATFORMS, tarballName } = require(join(npmDir, 'lib', 'platform.js'));
const expectedFiles = [
  'bin/agent-desktop.js',
  'lib/platform.js',
  'package.json',
  'scripts/postinstall.js',
];

const NPM_PUBLISH_JOB = 'publish-npm';
const BUILD_JOB = 'build';
const BUILD_FFI_JOB = 'build-ffi';
const PUBLISH_GITHUB_JOB = 'publish-github';

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

function matrixIncludeTargets(jobText) {
  const lines = jobText.split('\n');
  const includeStart = lines.findIndex((line) => line.trim() === 'include:');
  if (includeStart === -1) {
    return [];
  }
  const targets = [];
  let baseIndent = null;
  for (let index = includeStart + 1; index < lines.length; index += 1) {
    const line = lines[index];
    if (line.trim() === '' || !line.startsWith(' ')) break;
    let match;
    if ((match = line.match(/^\s*-\s+target:\s*(\S+)/))) {
      baseIndent = baseIndent ?? line.indexOf('-');
      targets.push(match[1]);
      continue;
    }
    if (baseIndent !== null && line.trim().startsWith('target:')) {
      targets.push(line.trim().replace(/^target:\s*/, ''));
    }
  }
  return [...new Set(targets)];
}

function normaliseTarballConstruction(line) {
  const match = line.match(/agent-desktop-v[^"']*?\.tar\.gz/);
  if (!match) return null;
  return match[0]
    .replace(/\$\{\{[^}]*\}\}/g, '{T}')
    .replace(/\$\{[^}]+\}/g, '{V}');
}

// One tarball-name construction exists per shell dialect: the macOS legs build
// names in bash, the Windows legs in pwsh. A parser that reads only the bash
// construction would pass while the pwsh branch quietly built something else,
// so each branch's construction is extracted and compared independently.
function tarballConstructionsByOs(jobText) {
  const constructions = {};
  let currentOs = null;
  for (const line of jobText.split('\n')) {
    const branchMatch = line.match(/if: runner\.os == '(macOS|Windows)'/);
    if (branchMatch) {
      currentOs = branchMatch[1];
      continue;
    }
    if (/^\s+- name:/.test(line)) {
      currentOs = null;
      continue;
    }
    if (!currentOs) continue;
    const normalised = normaliseTarballConstruction(line);
    if (normalised && !constructions[currentOs]) {
      constructions[currentOs] = normalised;
    }
  }
  return constructions;
}

function expectedAssetCountFromMatrices(cliTargets, ffiJobText) {
  const lines = ffiJobText.split('\n');
  const includeStart = lines.findIndex((line) => line.trim() === 'include:');
  if (includeStart === -1 || cliTargets.length === 0) return null;
  const entryIndent = lines[includeStart].length - lines[includeStart].trimStart().length + 2;
  const entryPrefix = ' '.repeat(entryIndent);
  const archiveKinds = [];
  for (let index = includeStart + 1; index < lines.length; index += 1) {
    const line = lines[index];
    if (line.trim() === '') continue;
    if (!line.startsWith(entryPrefix)) break;
    if (/^\s+-\s/.test(line)) archiveKinds.push(null);
    const kindMatch = line.match(/^\s+archive:\s*(tar\.gz|zip)\s*$/);
    if (kindMatch) archiveKinds[archiveKinds.length - 1] = kindMatch[1];
  }
  if (archiveKinds.some((kind) => kind === null) || archiveKinds.length === 0) return null;
  const tarGzCount = archiveKinds.filter((kind) => kind === 'tar.gz').length;
  const zipCount = archiveKinds.filter((kind) => kind === 'zip').length;
  return cliTargets.length + tarGzCount + zipCount + 1;
}

// The single owner of the workflow rules. The real check and `selfTest` both
// call it, so the fixtures drive the shipped rules rather than a copy of them
// that can drift.
function workflowViolations(workflow, platforms = PLATFORMS, tarballPatternFor = tarballName) {
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

  const buildJob = jobBlock(workflow, BUILD_JOB);
  if (buildJob === null) {
    violations.push(`release workflow must define a ${BUILD_JOB} job for every released CLI target`);
    return violations;
  }

  const cliTargets = matrixIncludeTargets(buildJob);
  if (cliTargets.length === 0) {
    violations.push(
      `could not locate the ${BUILD_JOB} job's build matrix in release.yml — the gate fails closed`,
    );
    return violations;
  }

  const releasedTargets = Object.entries(platforms)
    .filter(([, entry]) => entry.released)
    .map(([, entry]) => entry.target);
  for (const target of releasedTargets) {
    if (!cliTargets.includes(target)) {
      violations.push(
        `npm installs ${target} but the release workflow builds none of: ${cliTargets.join(', ')}`,
      );
    }
  }

  const packagePattern = tarballPatternFor('{V}', '{T}');
  const constructions = tarballConstructionsByOs(buildJob);
  for (const osName of ['macOS', 'Windows']) {
    if (!constructions[osName]) {
      violations.push(
        `could not locate the ${osName} tarball-name construction in the ${BUILD_JOB} job — the gate fails closed`,
      );
    } else if (constructions[osName] !== packagePattern) {
      violations.push(
        `${osName} release legs construct "${constructions[osName]}" but the npm package constructs "${packagePattern}"`,
      );
    }
  }

  const publishGithub = jobBlock(workflow, PUBLISH_GITHUB_JOB);
  if (publishGithub === null) {
    violations.push(`release workflow must define a ${PUBLISH_GITHUB_JOB} job`);
    return violations;
  }
  const declaredCount = publishGithub.match(/EXPECTED_ASSETS:\s*(\d+)/);
  const computedCount = expectedAssetCountFromMatrices(
    cliTargets,
    jobBlock(workflow, BUILD_FFI_JOB) ?? '',
  );
  if (!declaredCount) {
    violations.push(
      `could not locate EXPECTED_ASSETS in the ${PUBLISH_GITHUB_JOB} job — the gate fails closed`,
    );
  } else if (computedCount === null) {
    violations.push(
      `could not locate the ${BUILD_FFI_JOB} matrix in release.yml — the gate fails closed`,
    );
  } else if (Number(declaredCount[1]) !== computedCount) {
    violations.push(
      `${PUBLISH_GITHUB_JOB} expects ${declaredCount[1]} assets but the matrices imply exactly ${computedCount}`,
    );
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
    '    env:',
    '      EXPECTED_ASSETS: 5',
    '  build:',
    '    strategy:',
    '      matrix:',
    '        include:',
    '          - target: x86_64-apple-darwin',
    '            runner: macos-latest',
    '          - target: x86_64-pc-windows-msvc',
    '            runner: windows-latest',
    '    steps:',
    '      - name: Create tarball (macOS)',
    "        if: runner.os == 'macOS'",
    '        run: |',
    '          TARBALL="agent-desktop-v${VERSION}-${{ matrix.target }}.tar.gz"',
    '      - name: Create tarball (Windows)',
    "        if: runner.os == 'Windows'",
    '        shell: pwsh',
    '        run: |',
    '          $tarball = "agent-desktop-v${env:VERSION}-${{ matrix.target }}.tar.gz"',
    '  build-ffi:',
    '    strategy:',
    '      matrix:',
    '        include:',
    '          - target: x86_64-apple-darwin',
    '            archive: tar.gz',
    '          - target: x86_64-pc-windows-msvc',
    '            archive: zip',
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

  const fixturePlatforms = {
    'darwin-x64': { target: 'x86_64-apple-darwin', released: true },
    'win32-x64': { target: 'x86_64-pc-windows-msvc', released: true },
    'linux-x64': { target: 'x86_64-unknown-linux-gnu', released: false },
  };
  const violationsFor = (workflow) => workflowViolations(workflow, fixturePlatforms, tarballName);

  const failures = [];
  const expectCaught = (name, workflow, needle) => {
    const found = violationsFor(workflow);
    if (!found.some((violation) => violation.includes(needle))) {
      failures.push(`self-test FAIL (missed): ${name} -> ${JSON.stringify(found)}`);
    }
  };

  if (violationsFor(sound).length !== 0) {
    failures.push(
      `self-test FAIL (false positive): a sound workflow was rejected -> ${JSON.stringify(violationsFor(sound))}`,
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

  expectCaught(
    'a released npm target has no CLI matrix leg',
    sound.replace('          - target: x86_64-pc-windows-msvc\n            runner: windows-latest\n', ''),
    'npm installs x86_64-pc-windows-msvc',
  );
  expectCaught(
    'the pwsh Windows branch constructs a different archive extension than the package',
    sound.replace('$tarball = "agent-desktop-v${env:VERSION}-${{ matrix.target }}.tar.gz"', '$tarball = "agent-desktop-v${env:VERSION}-${{ matrix.target }}.zip"'),
    'could not locate the Windows tarball-name',
  );
  expectCaught(
    'only the pwsh branch diverges while bash stays correct',
    sound.replace('$tarball = "agent-desktop-v${env:VERSION}-${{ matrix.target }}.tar.gz"', '$tarball = "agent-desktop-v${env:VERSION}-pwsh-${{ matrix.target }}.tar.gz"'),
    'Windows release legs construct',
  );
  expectCaught(
    'publish-github carries a stale asset count',
    sound.replace('EXPECTED_ASSETS: 5', 'EXPECTED_ASSETS: 6'),
    'expects 6 assets but the matrices imply exactly 5',
  );
  expectCaught(
    'the build matrix is restructured beyond recognition',
    sound.replace('    strategy:\n      matrix:\n        include:', '    strategy:\n      matrix:\n          legs:'),
    'could not locate the build job\'s build matrix',
  );

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
