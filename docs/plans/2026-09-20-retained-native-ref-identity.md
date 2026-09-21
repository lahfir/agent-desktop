# Retain native identity across commands

Status: core and native retention implemented; persistent CLI host pending; unqualified.

## Decision

Keep the native accessibility object observed for each ref alive in a headless
command host. Start the host automatically for the selected snapshot namespace;
explicit sessions use their existing namespace. Ordinary snapshot/action usage
must not require users to manage a daemon. This deliberately advances the small
part of the deferred daemon phase needed for ref identity, not the entire phase.

The user delegated this architecture decision on September 20 after the Xcode
and isolated fixture reproductions. No further architecture approval is needed.

The current resolver mistakes a repeated AXIdentifier plus reused position for
identity. The retained-object Xcode probe preserved the original object while
the CLI returned the replacement. Neither matching mutable values nor searching
more broadly solves this general case. The fixture reproduces it without AXRow.
Evidence: ../solutions/runtime-errors/2026-09-20-xcode-filter-ref-alias.md.

## Smallest implementation boundary

1. First prove retained identity and current-tree membership on the existing
   RefChurnView, including replacement, restoration and ordinary same-ref edits.
   A retained object can remain readable after removal; readability alone must
   never authorize an action. Use native equality, not pointer addresses or
   hashes, to compare a retained object with candidates in its original scope.
   Also test native object reuse for different backing items: retention is not
   assumed to solve virtualization by itself.
2. Capture handles during the observation that creates refs. Retaining an object
   found later through the existing heuristic resolver would preserve the bug.
   Keep all native objects on their owning thread. Core carries opaque evidence;
   platform code owns native objects and validates live process/window/scope.
   Use the shared resolver for get, is, actions, snapshot roots and find roots.
3. Keep CLI as a thin transport into the existing typed command dispatcher,
   preflight, leases, deadlines, delivery dispositions and JSON envelopes. Batch
   uses that same path. FFI already has a thread-local owned-handle registry;
   reuse its ownership pattern and consolidate only the genuinely shared part.
   Do not route existing in-process FFI native handles through a second IPC hop.

Use stdlib local sockets and existing private endpoint/spawn patterns from the
cursor overlay where applicable; do not couple identity retention to the overlay
GUI or its lifecycle. No new dependency, service framework, alternate action
engine, private AX serialization API, or app-specific identity rule.

## Failure and lifecycle contract

- Bind retained tokens to host generation, snapshot namespace, process instance
  and source window. Never serialize a raw native pointer as identity.
- A dead/restarted host invalidates its refs. Return STALE_REF with fresh-snapshot
  recovery; never silently downgrade a retained ref to heuristic matching.
- A removed target cannot resolve to another object with identical attributes.
  Recovery may find the same object after movement; it cannot choose a lookalike.
- Scoped re-drill releases invalidated descendants and preserves sibling refs.
  Session end releases its objects; idle expiry bounds implicit-host lifetime.
  Capacity exhaustion must be explicit, not silent identity weakening.
- Validate private endpoint ownership, namespace and protocol version. Bound
  request size, connection/read/write time and queue time within command budgets.
  Serialize execution on the owner thread; concurrent startup needs one winner.
- After a request may have reached the host, a lost reply is uncertain delivery.
  Never automatically resubmit a mutation. Preserve the existing unsafe-retry
  disposition rather than pretending transport failure means not delivered.
- No app activation, physical input, cursor movement or clipboard insertion.
  Retention changes identity, not interaction policy or platform capabilities.

## Required evidence before enabling by default

The first gate is the existing isolated churn fixture, with visible backing-value
guards. Original refs must never read or write Beta after Alpha is removed.
Same-ref stable-field edits must still work. Inspect screenshots and raw public
responses; raw AX is a separate oracle. Test restoration and native object reuse
explicitly before claiming stale recovery.

Add deterministic checks at existing seams for host loss, cross-namespace refs,
process/window replacement, scoped invalidation, concurrent startup, deadline
exhaustion and lost mutation replies. Assert exact target and dispatch count,
including zero mutation calls on identity failure. No sleep-based pass criteria.

Then repeat the Xcode filter case headlessly without writing project labels.
Run existing workspace, semantic fixture, CLI/batch/FFI conformance, dependency
isolation, format, Clippy, release-size and performance gates. Measure cold host
startup separately from warm commands. Finally run acceptance-v1 across Xcode
and SF Symbols on one immutable candidate; D1 remains an unmet independent task.

Retention does not establish universal identity for apps that recycle native
objects, nor create semantic drag support. The provisional rating stays 8/10;
only the declared acceptance evidence can justify a higher scoped rating.

## First native gate results

`tests/real-apps/retained-identity-probe.swift` compiled and ran against owned
fixture PID 80361, window w-20092. It performs reads only; mutations used raw
Agent Desktop CLI responses and refs from snapshot s3o796sfka9h13.

All five explicit assertion stages passed: initial, filtered, restored, edited,
and cleared. Initially the retained Alpha object occurs once in the live tree.
After filtering it occurs zero times and reads return InvalidUIElement (-25202).
Live Beta is never CFEqual to retained Alpha. Restoring Alpha creates a different
native object; the original stays invalid. The stable editor remains the same
native object through filtering, restoration and both text edits.

This establishes the required distinction for this fixture: same-object edits
remain addressable; destruction/recreation requires a fresh ref. It does not
prove identity for virtualized controls or implement the product correction.
The old CLI still returned Beta through the saved Alpha ref during filtering.

The first trial's headless monitor was contaminated: target activation, several
other foreground changes, physical input and pointer movement occurred. These
session-wide events cannot be attributed by the monitor; this is not a headless
pass. The second assertion trial was unmonitored and carries no headless claim.
The viewed screenshot /tmp/ad-native-eval/retained-identity-restored.png showed
the first trial's original backing values restored. Both trials restored values.

## Core implementation checkpoint

Observation identifier evidence now carries a separate opaque retained-object
token. Snapshot projection preserves it internally without exposing it in the
public tree; snapshot allocation and find materialization persist it in ref
identity. Native IDs remain unchanged for queries. Saved retained identity takes
precedence over repeated native IDs, while mutable values remain volatile.
Hydration cannot substitute geometry when retained identity is missing.

Four focused checks pass: replacement/host-generation rejection with editable
values, equivalent snapshot/find persistence and serialization, empty-token
refusal, and hydration without identity downgrade. Existing constructor changes
initialize the optional field to None. No adapter produces these tokens yet;
the live wrong-target bug therefore remains unresolved.

Workspace tests passed with required local socket permissions; the earlier
sandbox run failed 13 endpoint tests because local binding was prohibited.
The final hydration guard passed its focused check after the workspace run.
Clippy initially caught test-module placement; it was corrected and the final
all-target check passed. Format and diff whitespace checks passed. Logs are in
/tmp/ad-native-eval/retained-evidence-*.log. Native gates and performance comparison
remain required after the adapter and host are connected.

## Adapter and real Xcode checkpoint

RetainedRefSession now owns a bounded thread-local native object store. Traversal
captures actual observed objects; the resolver and live pre-dispatch reads use
the same evidence. Native equality distinguishes objects within hash buckets;
hashes are not identities. Dropping the owner invalidates its tokens. Tests cover
owner replacement, cross-thread refusal, null objects, hash collision and capacity
exhaustion. The store still needs snapshot/session lifecycle pruning and the CLI
host. Default CLI behavior has not changed to use retention.

The persistent adapter fixture test passed all five stages, including same-object
edits. Its initial direct-adapter observation failed the existing accessibility
readiness requirement; the test was corrected to use public core resolve_query.
Workspace tests passed, seven retained-focused checks passed, and all-target
Clippy passed. Fixture values were visually verified restored before closing it.
The fixture monitor had no prohibited events but did not cover the complete
67-second assertion trial, so it is not a full headless qualification.

Real Xcode: PID 24741, w-19740, user-opened cursor-cam project. The ignored
retained_xcode_identity diagnostic performs read-only core/adapter calls;
filter mutations use raw public CLI responses from the immutable baseline.
First, Strict + FullRefMap timed out after five seconds, with a matching label
observed but incomplete reads. This failed trial remains an open broad-query
reliability issue. First + SelectedMatches then captured the original label.

During that second trial:

- Initial retained ref read CursorCamApp.swift.
- Public CLI set-value on navigator filter e246 applied CursorCamAppDelegate.
- Retained ref returned STALE_REF; baseline public get on e267 incorrectly
  returned CursorCamAppDelegate.swift in that same filtered state.
- Public clear restored the navigator. The retained ref remained STALE_REF,
  consistent with destruction/recreation; it never attached to the replacement.
- The three-stage adapter diagnostic passed. No file-label mutation was attempted.
- Viewed /tmp/ad-native-eval/retained-xcode-filtered.png and
  /tmp/ad-native-eval/retained-xcode-restored.png: filtered list and full restored
  list agree with expected effects; the navigator filter is empty again.

The first observation monitor was contaminated by session-wide physical activity.
A separate 120-second monitor was started before the second trial's initial read
and filter mutation. It recorded no Xcode activation, but session-wide input,
pointer changes and other-app activations later in its interval. Therefore it
does not qualify as a clean headless trial. This is a real-app adapter safety result, not a CLI repair or an
overall reliability score. Performance and final CLI acceptance remain pending.

## Internal command host checkpoint

CLI execution now returns its result independently of output emission. An internal
stdio host, enabled only by AGENT_DESKTOP_INTERNAL_COMMAND_HOST=1, holds the native
owner and accepts newline-delimited JSON arrays of CLI arguments. It invokes the
same parser, policies, dispatcher and envelope construction as normal commands.
Requests cannot switch session namespaces; incomplete/oversized requests do not
dispatch. Reply failure stops the stream without replaying a command.

This is a testable transport integration step, not the automatic host lifecycle.
Normal invocations still require automatic connection/startup, deadline and idle
handling, pruning, session-end cleanup, and the remaining acceptance gates.
# Scoped drill and internal-host Xcode checkpoint

Performance follow-up (supersedes the three-round regression signal below):
ten alternating rounds before optimization measured full Xcode snapshots at
574.2ms vs baseline 622.0ms and depth-30 at 595.1ms vs 574.3ms. The large earlier
slowdown did not reproduce. Diagnostics identified a duplicate AXValue
settability query: state observation and action discovery queried it separately.
Action discovery now reuses the same observation's known readonly result; an
unknown result still performs the native probe. No cross-command cache is used.
Observed settability reads fell from 390 to 354 with 210 nodes and 143 refs.

The benchmark now retains immutable binaries, isolates AGENT_DESKTOP_HOME instead
of HOME, retains every timing, and requires every snapshot to be complete and
shape-stable before reporting a comparable shape. Tests cover inconsistent and
incomplete samples. Ten alternating optimized-versus-previous rounds passed:
depth-30 medians 276.1/282.7ms, full interactive 271.0/272.6ms. A separate ten-round
optimized-versus-merge-base run passed with full interactive 599.7/588.3ms,
depth-30 577.4/583.7ms, skeleton 262.0/257.2ms, first-button find 384.8/373.4ms.
All snapshots in both runs were complete with stable, matching shapes. Absolute
latencies varied between runs; these results do not establish a universal latency
bound or identify the cause of the original short-run spike.

Final comparison: `/tmp/ad-native-eval/perf-final-baseline/report.html`.
Raw before/after: `/tmp/ad-native-eval/perf-settable-ab.json`.
Candidate SHA256: `4def83f565ae1375e9d28476eebfbb68afc3c2036b641ea045893d95ad53b831`.
Baseline SHA256: `dbc6036607681ee73b0e66a23d895864b793ba0be2278237cf76bc7984b1cbe0`.
Eleven action-list tests and benchmark tests passed; all-target clippy passed.

Input lifecycle: the internal stdio host now bounds idle input at 15 minutes and
an in-progress frame at five seconds using poll under an absolute deadline.
Buffered requests remain separate; input expiry stops before dispatch and returns
TIMEOUT/not_delivered. Existing lost-reply handling still prevents replay.
Eight host tests cover buffered frames, partial-frame expiry, expired idle input,
size limits, session isolation, and lost replies. A real process with stdin held
open after one `[` exited in 5.783 seconds without executing a command. That probe
first exposed INTERNAL/unknown classification; the subsequent fix and envelope
test require TIMEOUT/not_delivered. At this checkpoint output backpressure was still unbounded, and
automatic socket startup/routing remains pending. No desktop state was changed
by this input-lifecycle probe.

Host retention lifecycle follow-up: before the next request after each 4,096
new captures, the internal host reads retained tokens from all saved snapshots
in its selected namespace under the existing ref-store lock. It drops objects
with no saved token. Inventory/read failures abort pruning and prevent that
request from dispatching; they never turn a partial inventory into eviction.
Tokens use a monotonically increasing counter and explicit live membership, so
pruned tokens cannot alias subsequently captured objects. The 65,536-object
capacity remains a fail-closed bound. Six focused core and eight native tests,
including namespace isolation, corrupt inventory, pruning and token non-reuse,
passed; all-target clippy passed. This is still internal-host lifecycle work,
not automatic CLI transport.

The release performance comparison finished before this pruning change:
`/tmp/ad-native-eval/retained-recovery-perf/report.html` (three read-only Xcode
rounds; fixture intentionally skipped). Every case succeeded. Full snapshot
median was 2,374.6ms vs 977.0ms baseline, depth-30 828.5ms vs 658.7ms, skeleton
273.1ms vs 275.0ms, first-button find 365.6ms vs 365.2ms. Full-snapshot regression
signal is unresolved: full-snapshot shapes differed by one node/ref, so the
report marks that comparison non-equivalent. Depth-30 shapes matched and its
median increased 25.8%. Three rounds cannot establish the cause. This run does not
exercise the internal host or qualify its memory lifecycle.

Follow-up: the post-write AXRole failure originated in `ax_helpers::element_role`
inside `set_dynamic_verified`, after the setter had completed. The existing
capability read retry loop is now shared through `tree/read_recovery.rs`; role
reads use it too. Only kAXErrorCannotComplete retries, at most three reads under
the original deadline. Setter delivery and mutation retry behavior are unchanged.
Deterministic tests check recovery, terminal error classes, attempt limits, and
expired budgets. Workspace library tests and all-target clippy passed. The first
sandboxed macOS run had six socket-permission failures; the authorized workspace
run passed. Both logs are retained under `/tmp/ad-native-eval/read-recovery-*`.

New internal-host candidate `/tmp/ad-native-eval/read-recovery-candidate`:
Xcode snapshot `s1go8vsir5pm7h`, filter `e3`; set-value CursorCamAppDelegate and
clear both returned delivered_verified (149ms and 54ms lease holds). Screenshot
`/tmp/ad-native-eval/read-recovery-restored.png` was viewed and confirms restoration.
This live run did not expose retry counts and therefore does not prove a transient
failure occurred during it. Host closed normally; no files were edited in Xcode.

Candidate `/tmp/ad-native-eval/scoped-drill-candidate`, SHA256
`6cfcd744c8e0fda3d0ca6b70180e9f4896623ca869d605b70c3149f2892a877d`.
The previous internal-host navigator drill failed with a CoreGraphics inventory
timeout. Inspection found that `resolve_result_window` enumerated all apps despite
having a source app. It now supplies the existing app filter while retaining exact
window-ID and PID checks. All 14 snapshot-ref tests pass, including the source-app
filter assertion. Formatting and diff checks pass.

Live Xcode window `w-19740`: skeleton `s1ww1okjjddk91`, navigator root `e2`
drilled successfully. Its `e39` represented CursorCamApp.swift. Setting navigator
filter `e3` to CursorCamAppDelegate returned APP_UNRESPONSIVE for AXRole (-25204),
with the requested value in post-state and delivered_unverified/unsafe disposition.
The write was not retried. Reading old `e39` returned STALE_REF instead of the
remaining Delegate label. Clearing `e3` returned delivered_verified; a screenshot
at `/tmp/ad-native-eval/scoped-drill-restored.png` was viewed and confirmed the
empty filter, restored navigator, and unchanged editor selection.

Two real-app value-wait checks: empty value matched in 15ms; an absent value
returned TIMEOUT under a 200ms budget with matched=false. These are single-run
checks, not qualification of all waits. The 60-second monitor contained unrelated
input and activation of PID 35000, so it cannot qualify headless execution.
The internal host was closed after restoration. Automatic ordinary-CLI hosting,
repeated acceptance runs, and the new performance baseline remain pending.

## Output backpressure process gate

A child-process probe reproduced a shutdown hang: the host detected its write
timeout, but global StdoutLock buffered bytes blocked process exit until the caller
drained stdout. The previous release remained alive after eight seconds; draining
65,768 bytes released it. The host now owns an unbuffered File cloned from stdout,
with a local BufWriter above the deadline writer, so buffer flushing stays bounded.

The process regression limits the output socket buffer, leaves output unread and
stdin open, and queues a second request. It requires exit 1, timeout diagnostics,
and an unfinished first frame without a second response. The final implementation
passes. Ten host unit tests pass. Logs are under /tmp/ad-native-eval/host-process-fix.log,
host-unit-final.log and host-clippy-final.log. Automatic ordinary CLI routing and
real-app acceptance remain incomplete; this check does not raise the rating.

## Separate-connection server checkpoint

The internal host can now listen on a private Unix socket while keeping the
same RetainedRefSession on its owner thread across connections. Each connection
accepts exactly one request; malformed input or a lost client does not terminate
the owner. The socket requires an absolute path under an owner-only directory,
checks peer UID, and bounds idle, frame and output waits. It uses the same parser,
dispatcher, retention pruning and output envelope as the stdio host.

Eleven host unit tests passed. Two child-process checks passed in 5.69 seconds:
unread-output termination and malformed-client recovery followed by separate
version requests, including rejection of a second queued request on one connection.
The initial sandboxed socket-binding run failed; the authorized local-socket run
passed. Logs: /tmp/ad-native-eval/host-socket-tests.log and host-socket-process.log.
Test children were reaped. Automatic client routing, endpoint discovery, startup
serialization, request-context transport and queue-budget enforcement are still
pending. The socket mode remains internal and is not enabled for normal commands.

## Experimental automatic client checkpoint

With AGENT_DESKTOP_INTERNAL_HOST_CLIENT=1, separate CLI processes now discover or
start a private socket host and forward requests through the existing parser and
dispatcher. Startup and host ownership use the existing FileLock. Endpoint identity
includes protocol, executable metadata, state root, namespace and helper path;
the server compares the full identity, not merely its socket-name hash. Requests
carry working directory and agent identity. Startup failures are not replayed;
lost replies return delivery_uncertain/unsafe. No default routing is enabled yet.

Twelve focused unit checks passed, including rejection of wrong identity or
namespace before changing working directory. Three process checks passed in
5.68 seconds, including two simultaneous initial CLI invocations and confirmed
same-PID reuse by a subsequent invocation. Test hosts were terminated. Logs:
/tmp/ad-native-eval/host-client-final-tests.log, host-client-process.log and
host-client-clippy.log. The first log intentionally filters unit tests; the second
is the unfiltered three-process-test run. All-target Clippy passed.

Before default routing: replace the experimental fixed response timeout with
command-aware queue and execution budgets, bound connection establishment, test
restart/stale refs and namespace isolation through the client, and run real Xcode
acceptance. Batch namespace changes, session-end release, cancellation, diagnostic
forwarding and per-command environment behavior also need review. These are
unfinished integration requirements, not a claim that the product ref bug is fixed.

## Closeout at user request

Work stopped after finishing the in-progress deadline boundary. The experimental
client derives a total budget from typed command arguments and optional post-action
wait, carries an absolute monotonic expiry, and uses remaining time for startup
and response waiting. The server rejects expired requests before dispatch and
passes remaining time into the existing CommandContext deadline. Unix connection
establishment is nonblocking and deadline bounded. Lost replies remain uncertain
and are never replayed. Sleep commands retain their requested duration plus the
standard preflight allowance; response delivery has a bounded grace period.

Final focused results: 15 host unit tests and three process tests pass, including
an expired session-start request that returns TIMEOUT/not_delivered and creates
no session artifact. Concurrent startup and same-PID reuse still pass. All-target
Clippy, formatting and diff checks pass. Logs: /tmp/ad-native-eval/host-budget-tests.log,
host-budget-process.log, host-budget-clippy.log and final-workspace.log.

The client is still experimental behind AGENT_DESKTOP_INTERNAL_HOST_CLIENT=1.
Do not enable default routing based on this checkpoint. Remaining work includes
restart/stale-ref tests through the client, real Xcode acceptance, batch namespace
handling, session-end release, cancellation and diagnostics/environment parity.
Post-action wait shares the total transport budget; independent primary-phase
queue accounting needs review. The final host integration has not received a new
performance baseline or full real-app qualification. Prior performance measurements
do not qualify it. No commit, push or release was made. Provisional rating remains
8/10; the requested 10/10 objective was not achieved.
