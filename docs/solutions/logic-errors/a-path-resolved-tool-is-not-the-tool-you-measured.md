---
module: npm
component: postinstall
problem_type: platform-portability
applies_when:
  - "Shelling out to a tool by bare name on Windows"
  - "Depending on an in-box Windows utility that a developer toolchain also ships"
  - "Writing a probe that measures one binary and code that invokes another"
tags: [windows, path, tar, packaging, install, probe-evidence]
---

# A PATH-resolved tool is not the tool you measured

## Problem

`npm install -g agent-desktop` failed on Windows for anyone with Git for
Windows installed — which is most people who install a developer CLI. The
install downloaded its release archive, then died before unpacking it:

```
tar (child): Cannot connect to C: resolve failed
tar: Child returned status 128
```

The decision to ship the Windows CLI as a `.tar.gz` rather than a `.zip` rested
on a measured probe row: Windows has shipped `tar.exe` in `System32` since
build 17063, it is bsdtar, and it round-trips a gzip tarball containing an
absolute path. That measurement was correct. The code did not use the binary it
measured.

## Root cause

`postinstall.js` called `execFileSync('tar', ['-xzf', tarballPath, ...])`. A
bare name is resolved through `PATH`, and Git for Windows, MSYS2 and Cygwin all
place a **GNU** tar earlier on `PATH` than `System32`. GNU tar keeps the
historical `host:path` syntax for remote archives, so it reads

```
C:\Users\...\AppData\Roaming\npm\...\release.tar.gz
```

as *host `C`, path `\Users\...`* and tries to open a network connection before
it ever looks at the filesystem. The failure is not a corrupt archive or a
missing file; it is a well-formed request to the wrong program.

The probe and the product disagreed about what `tar` meant, and nothing
connected them: the probe invoked `C:\Windows\System32\tar.exe` by absolute
path, because that is what it was measuring, while the product invoked whatever
`PATH` offered. Both were internally consistent. The gap was invisible in
review because the two spellings look like the same call.

## Solution

Resolve the in-box tool by absolute path, and fall back to `PATH` only when it
is genuinely absent:

```js
function tarCommand(osPlatform, environment) {
  if (osPlatform !== 'win32') return 'tar';
  const systemRoot = environment.SystemRoot || environment.windir;
  if (!systemRoot) return 'tar';
  const inBox = join(systemRoot, 'System32', 'tar.exe');
  return existsSync(inBox) ? inBox : 'tar';
}
```

Taking the platform and environment as arguments rather than reading the host
makes every branch assertable from any machine, so the Windows resolution is
covered by tests that run on macOS CI.

The test helpers that build fixture archives were pointed at the same resolver.
A helper that builds archives with a different tar than the code unpacks them
with is a test that cannot observe this class of defect.

## Prevention

- **Invoking a tool by bare name is a `PATH` lookup, not a choice of program.**
  On Windows, assume a developer toolchain has shadowed every common Unix
  utility name. `tar`, `find`, `sort`, `link` and `more` all resolve to
  something other than the in-box tool on a normal developer machine.
- **When a decision rests on a measurement of a specific binary, invoke that
  binary.** If a probe cites an absolute path and the product uses a bare name,
  the evidence does not cover the code. Cite the row next to the call, or make
  the call match the row.
- **Test helpers must use the same resolution the product uses.** Otherwise the
  suite proves the archive format works and says nothing about whether the
  product can read it.
- A related shape worth the same suspicion: `npm` is `npm.cmd` on Windows.
  `execFileSync` will not find it by bare name, and since Node's batch-file
  argument hardening it will not launch it directly either — ENOENT, then
  EINVAL. Anything shelling out to a package manager needs a Windows path.
