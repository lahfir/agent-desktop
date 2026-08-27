# Dogfood: FFI, npm, Release (Sub-phase 2.13)

- **Date:** 2026-08-25
- **Branch:** `feat/windows-2.13-ffi-npm-release`
- **Channels exercised:** npm (as a user), FFI cdylib (as an embedder), documentation (as an agent)
- **Release substrate:** the npm and FFI legs consumed artifacts produced by the
  U9 dry run ([release-dry-run.json](2026-08-24-001-captures/release-dry-run.json)),
  not hand-made archives.

## Channel 1 — npm, as a user

Installed `npm/agent-desktop-0.8.3.tgz` into a scratch global prefix; the
postinstall ran with `AGENT_DESKTOP_BINARY_PATH` pointing at a binary extracted
from the dry-run's real release archive by `installArchive`. Drove **Paint**
(mspaint — its Ribbon framework produces the densest tree available on this box;
no Chromium/Electron host is installed here, confirming A24-12) entirely through
the npm shim:

- `snapshot --app mspaint.exe -i --timeout-ms 30000` → `ok:true`, 45 refs,
  `complete:true`; ribbon toolbar, tabs, and Quick Access Toolbar buttons all
  ref'd.
- `click` on the `File tab` menubutton ref → `ok:true`,
  `delivery=delivered_unverified` (honest SendInput disposition).
- Re-snapshot → 61 refs, and `New / Open / Save / Save as` file-menu items now
  visible in the tree — the click's effect confirmed by the application's own
  state, never by the envelope.
- Earlier in U9, the same shim drove Notepad end to end:
  snapshot (`ref_count=17`) → `set-value` on the textfield ref
  (`delivered_verified`) → `get --property value` read back the exact string.

## Channel 2 — FFI cdylib, as an embedder

Extracted `agent-desktop-ffi-v0.8.4-dryrun.1-x86_64-pc-windows-msvc.zip` from
the dry run and loaded `lib/agent_desktop_ffi.dll` from Python 3.14 via
`ctypes.CDLL` against the **real** adapter (no stub feature). First outside-Rust
exercise of the Windows cdylib. Round trip on live Notepad:

- `ad_adapter_create()` non-null; `ad_snapshot(adapter, "notepad.exe", Window)`
  returned `ok=true`, 17 refs.
- Parsed the envelope for the textfield ref, built the C `AdAction` struct
  (`kind=4`, SetValue), `ad_execute_by_ref` → `ok=true`,
  `delivery=delivered_verified`.

The MSVC import library (`agent_desktop_ffi.dll.lib`) ships beside the DLL in
the same archive for build-time linkers, per R14.

## Channel 3 — documentation, as an agent

Read `skills get agent-desktop-windows` from the built binary and followed its
capability table literally:

| Claim in the skill | Observed |
|---|---|
| `permissions`: UIA reads need no grant | `automation: "not_required"` exactly as documented |
| `list-surfaces` unavailable | `PLATFORM_NOT_SUPPORTED` |
| notification commands unavailable | `PLATFORM_NOT_SUPPORTED` |
| cursor-overlay records but renders nothing | `cursor-overlay enable` inside a session → `ok:true`, setting recorded; nothing rendered |

## Findings

### F1 — npm 12's install-scripts allowlist blocks postinstall even when allowed

npm 12.0.1 (bundled with node 24.18) skips install scripts for packages not on
its `allowScripts` allowlist. On this box neither `--allow-scripts=agent-desktop`
nor user-config allowlisting admitted the script; the wrapper then failed loudly,
naming `--ignore-scripts`/failed-postinstall as the likely cause — which is the
KTD3-accepted design doing its job, but increasingly as the *first* experience a
Windows user has. **Disposition: owned elsewhere** — written into §2.15's
inherited-risk item in `docs/phases.md` in this PR (weigh publishing the
allowScripts configuration before Phase 2 closes).

### F2 — Elevated/non-elevated mixing surfaces as bare INTERNAL without guidance

An elevated shell driving a data directory created by a non-elevated session
gets `INTERNAL: private file parent is owned by a foreign principal…`. The
refusal is correct (the ownership check protecting refmaps/traces), but nothing
told the agent it is an elevation-boundary symptom or that pointing `HOME` at a
fresh directory resolves it. **Disposition: fixed here** — new
"INTERNAL: private file parent…" entry in
`skills/agent-desktop-windows/references/troubleshooting.md` naming cause and
fix. Verification: the entry is served through the binary
(`skills get agent-desktop-windows references/troubleshooting.md`); the
core-crate skill coverage test fails if the file stops being served, which is
the test that guards this fix's reachability.

### F3 — Headless click on a plain EDIT textfield refuses closed while set-value delivers

Notepad's edit area advertises no semantic Click action, so headless `click`
returns `POLICY_DENIED` ("direct semantic delivery is unavailable and physical
fallback is denied") with a suggestion naming `available_actions` and `--headed`;
`set-value` (ValuePattern) delivers verified. This is the policy floor working
as designed rather than a defect. **Disposition: accepted** — the refusal is
fail-closed with actionable recovery text, matching §2.8's shipped semantics.

## Verdict

Three channels consumed this sub-phase's own outputs against real applications;
three findings judged with one disposition each (owned elsewhere / fixed here /
accepted). The distribution claim this sub-phase exists to make — Windows
reachable through the channels macOS already ships — is evidenced by observation
rather than assertion.
