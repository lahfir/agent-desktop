# Windows self-hosted E2E runner

This runbook is the registration and hardening policy for an interactive
Windows runner carrying the labels `[self-hosted, Windows, agent-desktop-e2e]`.
**No runner is registered, and no CI workflow targets one.** The live e2e
lane that named these labels was retired on the owner's decision: the
runner's labels are reachable from every `pull_request`-triggered workflow
in the repository rather than from one file, so a fork PR that edits any of
them is code execution on the owner's interactive desktop. That exposure is
the whole reason GitHub advises against self-hosted runners on public
repositories, and it is not worth automating a suite that already runs on
demand.

**The live Windows e2e suite is therefore run locally, not in CI.** It is
driven with `scripts/run-windows-e2e-ci.ps1` on a developer box holding the
interactive session, under the exclusive `DesktopLease`, and its result is
read from that run. CI runs the Windows unit tests, clippy, the contract
gates and the harness self-tests on GitHub-hosted `windows-latest` and
`windows-11-arm`; none of that needs a desktop.

This document is kept, unexecuted, for one reason: if a runner is ever
registered, the policy it must be registered against should already exist
and be reviewable, rather than being invented under time pressure. Read the
sections below as preconditions on that future decision, not as a procedure
anyone is expected to run today.

## Why an interactive session, not a service

`tests/e2e-windows`'s harness drives real UIA against a real desktop -
`snapshot`, `click`, window activation, occlusion checks, keyboard/mouse
synthesis. A Windows service (including the GitHub Actions runner installed
as a service, which is the default) runs in Session 0 with no interactive
desktop attached; UIA cannot see a window that has no desktop to be drawn
on. The runner process must therefore run inside a real interactive logon
session, not as a service.

The registration procedure is: install the runner in a normal (non-service)
mode, then schedule `run.cmd` (the runner's own launch script) via **Task
Scheduler, triggered "At log on"**, running as the account that owns the
interactive session - not `schtasks /RU SYSTEM`, which has no desktop
either. The box must stay logged in (auto-logon or a session left unlocked)
for the scheduled task to have a session to launch into; a locked or
disconnected session degrades UIA visibility in ways this corpus has not
measured (see "What §2.15 still owes" below), so the operational
expectation is an unattended box that stays logged in and unlocked, not one
that gets RDP'd into and left locked.

## Trigger policy - `workflow_dispatch` and branch-scoped `push` only, never `pull_request`

`windows-e2e.yml` triggers on `workflow_dispatch` and on `push` to
`feat/windows-adapter`. It never triggers on `pull_request`:

- **`pull_request` on a public repository is exactly the exposure GitHub's
  own self-hosted-runner guidance warns against.** A fork's PR would run
  workflow code chosen by an untrusted contributor against a box with real
  desktop access - this corpus's whole `DesktopLease`/exclusivity design
  exists to keep *trusted* concurrent processes from fighting over that
  desktop, not to sandbox an adversarial one.
- `workflow_dispatch` alone is not enough to reach this workflow at all
  during normal development: GitHub only offers a `workflow_dispatch` run
  for a workflow file present on the **default branch**, and every Windows
  sub-phase lands on `feat/windows-adapter`, never `main` (see
  `windows-capability-probe.yml`'s own header comment for the same
  mechanic). Without a second trigger, this file would sit dispatchable by
  nobody until the Windows adapter's eventual promotion to `main`.
- A `push` trigger scoped to `feat/windows-adapter` closes that gap without
  opening the fork-PR exposure: a `push` event fires at the pushed ref and
  **cannot be triggered by a fork** - only a contributor with write access
  to this repository can push to `feat/windows-adapter` in the first place.

**With no runner registered, this relaxation grants no live exposure yet.**
A `push` to `feat/windows-adapter` today queues a job that no runner
claims, and because `windows-e2e.yml` sets `cancel-in-progress: false`,
those queued runs accumulate rather than superseding each other. **§2.15
must re-ratify this trigger policy at registration time** - the exposure
becomes real the moment a runner starts claiming those jobs - and should
pair registration with a **queue flush** (cancelling whatever `push`-queued
runs accumulated while unregistered) so the runner's first live run is not
racing a backlog.

## Fork-PR approval policy

Independently of `windows-e2e.yml`'s own trigger scope, this repository's
Actions settings must have **"Require approval for all outside
collaborators"** (or stricter) set for workflow runs before any self-hosted
runner is registered - GitHub's platform-level gate that a maintainer must
approve before *any* workflow (not only this one) runs against a
self-hosted runner from a fork-originated pull request. This is defence in
depth beside the trigger-policy restriction above, not a substitute for it:
the trigger policy keeps `windows-e2e.yml` itself out of the fork-PR path
entirely, and the approval setting keeps a *different*, less carefully
scoped workflow from becoming the exposure instead.

## Ephemeral-versus-persistent: persistent, by requirement

**Decision: the runner is registered persistent, not ephemeral/JIT.**
Rationale: a just-in-time or ephemeral runner is provisioned per job and
torn down after, which is GitHub's recommended posture for self-hosted
runners on public repositories precisely because it limits how long a
compromised or leftover-state runner stays reachable. That posture assumes
the runner can be started **as a service** at job-claim time with no
pre-existing session - which is the one thing this workload cannot do (see
"Why an interactive session, not a service" above). An ephemeral runner
provisioned fresh per job would need an interactive logon session already
established before the job starts, which defeats the point of on-demand
provisioning. The box therefore stays a **persistent** interactive machine,
registered once, with the trigger policy above (never `pull_request`, no
fork exposure) carrying the security weight that ephemeral provisioning
would otherwise have carried.

## Workspace and credential hygiene between runs

`windows-e2e.yml`'s checkout step (mirroring `native-e2e.yml`) is pinned to
a specific `actions/checkout` commit SHA and sets **`persist-credentials:
false`**. On a *persistent* self-hosted runner this matters more than it
would on an ephemeral GitHub-hosted one: without it, the job's short-lived
`GITHUB_TOKEN` would be written into the checked-out workspace's
`.git/config` and stay readable there for as long as the workspace persists
- readable by anything else that runs in the same interactive session on
this box, not only by the job that wrote it. The token is short-lived and
scoped to `contents: read`, so this is defence in depth rather than a
close of a live hole, which is why it is recorded here beside the other
hardening decisions rather than in a risk register.

The consequence for the **workspace itself**: because the runner is
persistent, the default runner behavior is to reuse the same working
directory across runs rather than starting from a clean checkout each time
(the ephemeral/ JIT model's clean-slate guarantee does not apply here).
§2.15's registration should confirm the runner is configured to clean the
workspace between jobs (`clean: true` is `actions/checkout`'s default, and
Actions' own working-directory cleanup runs before each job) rather than
relying on `.gitignore` alone to keep one run's build artifacts from
leaking into the next run's observations.

## Labels

`[self-hosted, Windows, agent-desktop-e2e]` - already named by the
committed `windows-e2e.yml`. §2.15's registration allocates exactly these
labels to the interactive box; no other label is required or expected.

## Capture upload is gated before it runs

`windows-e2e.yml`'s live-run capture upload only runs after
`scripts/check-capture-redaction.ps1` succeeds against the run's own
capture directory, and only uploads the text captures that gate can
inspect - never a screenshot or other binary the gate cannot read. This is
the one path around R21's redaction gate that a committed-capture-only
scan (U15's scope) cannot reach: a live run against real applications
(File Explorer, Notepad, Task Manager, an Electron target) produces
captures that are never committed, only published as a workflow-run
artifact any reader of a public repository can download. §2.15's
registration must not remove or bypass this step when it wires in whatever
directory the live run's captures land under - see `windows-e2e.yml`'s own
comments for the exact ordering requirement (the gate step's outcome, not
merely job success, is what the upload step's `if:` condition reads).

## Teardown procedure

To decommission the runner (temporarily or permanently):

1. Cancel any queued or in-progress `windows-e2e.yml` runs for the labels
   above (`gh run list --workflow=windows-e2e.yml` /
   `gh run cancel <run-id>`).
2. Stop and remove the scheduled task that launches `run.cmd`.
3. Unregister the runner from the repository (Settings → Actions →
   Runners → Remove), which invalidates its registration token.
4. Delete the runner's work directory. Because the runner is persistent
   (not ephemeral), the work directory can carry residue from prior runs -
   staged binaries, a lock file under the box's own
   `ProgramData\agent-desktop\<SID>\` path (this corpus's `DesktopLease`,
   R16b/R16c) - none of which should survive decommissioning.
5. If the box is being repurposed rather than retired, confirm no
   auto-logon credential or scheduled task referencing the runner account
   remains configured.

## What is still owed, and by whom

Nothing in this document is owed by a sub-phase any more. The CI-green-on-a-
self-hosted-runner obligation that rode here was **retired, not deferred**:
the owner declined registration a second time and directed that the live e2e
lane be removed from CI, so there is no receiving sub-phase and no
infrastructure waiting to be provisioned. The live suite's evidence comes
from local exclusive runs on a developer box, which is how every scenario in
it has actually been verified.

One genuinely separate need survives and is unrelated to CI: multi-monitor
and session-degradation evidence (locked desktop, disconnected RDP session,
Session 0) requires a **second, disposable** interactive host. The box that
holds the working session cannot safely be locked or disconnected to measure
it, because a failed reattach strands the machine mid-run. That is recorded
as a ratification with its reason in `probes/windows/FINDINGS.md` rather
than as work waiting for a runner.
