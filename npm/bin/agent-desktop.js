#!/usr/bin/env node

const { spawn } = require('child_process');
const { existsSync, accessSync, chmodSync, constants } = require('fs');
const { isAbsolute, join } = require('path');
const { platform, arch } = require('os');
const { resolve } = require('../lib/platform');

const binDir = __dirname;
const MACOS_HELPER_NAME = 'agent-desktop-macos-helper';
const MACOS_HELPER_PATH_ENV = 'AGENT_DESKTOP_MACOS_HELPER_PATH';

function getBinaryName() {
  const entry = resolve(platform(), arch());
  return entry ? entry.binaryName : null;
}

function main() {
  const binaryName = getBinaryName();

  if (!binaryName) {
    console.error(`Error: Unsupported platform: ${platform()}-${arch()}`);
    console.error('agent-desktop currently supports: macOS (ARM64, x64)');
    console.error('Windows and Linux support is coming in Phase 2.');
    console.error('See: https://github.com/lahfir/agent-desktop');
    process.exit(1);
  }

  const binaryPath = join(binDir, binaryName);

  if (!binaryPath || !existsSync(binaryPath)) {
    console.error(`Error: Native binary not found for ${platform()}-${arch()}`);
    console.error(`Expected: ${binaryPath}`);
    console.error('');
    console.error('Try reinstalling:');
    console.error('  npm install -g agent-desktop');
    console.error('');
    console.error('Or download directly from:');
    console.error('  https://github.com/lahfir/agent-desktop/releases');

    if (typeof process.versions.bun !== 'undefined') {
      console.error('');
      console.error('Bun detected — postinstall scripts require --trust:');
      console.error('  bun install -g --trust agent-desktop');
    }

    process.exit(1);
  }

  if (platform() !== 'win32') {
    try {
      accessSync(binaryPath, constants.X_OK);
    } catch {
      try {
        chmodSync(binaryPath, 0o755);
      } catch (err) {
        console.error(`Error: Cannot make binary executable: ${err.message}`);
        console.error('Try running: chmod +x ' + binaryPath);
        process.exit(1);
      }
    }
  }

  if (platform() === 'darwin') {
    const override = process.env[MACOS_HELPER_PATH_ENV];
    if (override && !isAbsolute(override)) {
      console.error(`Error: ${MACOS_HELPER_PATH_ENV} must be an absolute path`);
      process.exit(1);
    }
    const helperPath = override || join(binDir, MACOS_HELPER_NAME);
    if (!existsSync(helperPath)) {
      console.error(`Error: macOS helper not found: ${helperPath}`);
      console.error('Reinstall agent-desktop so the CLI and helper come from the same release.');
      process.exit(1);
    }
    try {
      accessSync(helperPath, constants.X_OK);
    } catch {
      try {
        chmodSync(helperPath, 0o755);
      } catch (err) {
        console.error(`Error: Cannot make macOS helper executable: ${err.message}`);
        process.exit(1);
      }
    }
  }

  const child = spawn(binaryPath, process.argv.slice(2), {
    stdio: 'inherit',
    windowsHide: false,
  });

  child.on('error', (err) => {
    console.error(`Error executing binary: ${err.message}`);
    process.exit(1);
  });

  child.on('close', (code) => {
    process.exit(code ?? 0);
  });
}

main();
