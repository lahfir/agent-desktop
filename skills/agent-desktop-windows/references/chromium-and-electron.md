# Chromium and Electron Trees

Chromium-based apps (Electron or browser) are the most common source of
false "the tree is empty" conclusions on Windows. The tree is real; it is
built asynchronously after the first UIA client connects.

## Exposure Needs No Flag on Modern Chromium

Chromium 138+ exposes a full UIA tree to any UIA client with no
`--force-renderer-accessibility` flag. Measured against an Electron 39 app
bundling Chromium 142: the settled tree was identical with and without the
flag, managed-stack and COM clients agreed, and no client divergence existed
(A1-4). Do not relaunch an already-running app just to add accessibility
flags — connect to what is there.

## First Contact Is Thin; Wait for Settle

The first read after connecting triggers the renderer's accessibility build,
so it returns a fraction of the settled tree:

- First contact measured **13** nodes against a settled **172** — a 13.2x
  understatement, deterministic across runs (A1-5).
- On a cold launch, first contact was **12** descendants against a settled
  **165** (13.75x), and the settle took **10–25 seconds**, non-deterministic
  (A16-11).

Consequences:

- Pass `--timeout-ms` to `snapshot` so its budget covers the settle instead of
  concluding the tree is empty; the adapter runs a connection-plus-settle pass
  and re-walks within that budget.
- A grown tree is visible to a **fresh** read; the triggering client can keep
  serving the shell it captured first (A16-11). If one command still reports a
  thin tree after settling, run a new snapshot rather than retrying in-process.
- Adding the renderer flag changes nothing about eventual exposure and does
  not remove the need to settle: with the flag, first contact became a race
  landing anywhere from a fraction of settled to nearly all of it (A1-5).

## Covered Windows Hold First-Contact Counts Indefinitely

A Chromium window completely covered by other windows can hold its
first-contact node count through the whole settle: an earlier measurement held
first-contact counts through an 8 s settle and a 16 s instrumented hold across
three consecutive runs, and minimizing the covering windows fixed it (A1-6).
While an agent works, do not park another window over the target during
launch/settle; if counts look frozen at a small number, bring the target to
the foreground and re-snapshot.

## WPF: Reading Too Early Binds the Wrong Provider

Distinct from Electron's under-then-grow behavior: a UIA client that reads a
WPF window before its automation peer exists binds the generic HWND provider
**permanently**. The window then reports `ClassName HwndWrapper[...]` with zero
children, and a 30 s in-process poll — including fresh `FromHandle` calls and a
forced full-descendants search — never recovered it; the same fixture read 8 s
after launch reported `ClassName=Window` with 8 children immediately (A1-7).

Recovery is re-resolution from outside: take a new snapshot so the adapter
re-resolves from a new handle/new client. Retrying the same read cannot fix
it. Practically: after launching a WPF application, prefer `wait --event
window-opened` or a short `wait` before the first snapshot.

## Unnamed Web Content Is Unresolved Through the Semantic Tier Today

Content elements inside web documents that expose **no accessible name**
currently fail strict ref resolution: against real web content staged in a
notes app, positive-area unnamed checkboxes produced a 0.75 stale rate across
eight interleaved reads, and a semantic click surfaced as `TIMEOUT` wrapping
the underlying `STALE_REF` (A24-11). Named content elements resolve normally.
When a target must be an unnamed element, fall back to headed coordinate input
from a parent's bounds, or use a CDP client (`launch --cdp`) for the web
contents while keeping agent-desktop for native surfaces.
