---
title: An AT-SPI error name is not proof an object is gone
date: 2026-10-18
category: runtime-errors
module: crates/linux/src/tree/properties/absence.rs, crates/linux/src/tree/resolve/resolve_errors.rs, crates/linux/src/tree/observe/node_evidence.rs, crates/linux/src/tree/resolve/mod.rs, crates/linux/src/tree/properties/toolkit_trust.rs
problem_type: runtime_error
component: atspi-resolver
symptoms:
  - "A live element is reported destroyed because one read returned UnknownObject, UnknownMethod or UnknownInterface."
  - "A destroyed element still resolves because a read of its stored object path succeeded."
  - "An exited application is reported as a destroyed element, or a stopped one as an exited one."
tags: [linux, at-spi, d-bus, stale-ref, resolution, error-names, toolkits]
---

## Problem

Re-identifying a stored ref on Linux needs a verdict on whether the element still exists. The obvious rule reads the stored object path and treats the D-Bus error names `UnknownObject` / `UnknownMethod` as "gone". Measured against live GTK3, GTK4, Qt 6 and Chromium (`probes/linux/FINDINGS.md` L17-1 to L17-12), that rule is wrong in both directions.

## What the toolkits actually do

- **The same names mean "not implemented" on live objects** (L17-2). Qt answers `UnknownObject` to `Text.GetText` on 46 of 48 live nodes and `UnknownInterface` to `Properties.Get` of `Value`; GTK4 answers `UnknownMethod`; GTK3 answers `Failed`. Qt also answers `UnknownInterface` to `Properties.GetAll(Accessible)` on every live node (L17-3), so a `GetAll` failure is no absence signal.
  The same holds on the application interface: GTK3 and Qt refuse `Properties.GetAll(org.a11y.atspi.Application)` while answering `Get(ToolkitName)` and `Get(Version)` (L27-20, L27-32), so `toolkit_trust.rs` reads the toolkit one property at a time and a refused group read leaves nothing to conclude.
- **Bulk reads can crash the bridge.** A later raw Qt probe killed the app with `Properties.GetAll(Accessible)` while individual `Properties.Get` calls and an interface-guarded walk survived (L41-7). The reader therefore uses concurrent individual properties and checks advertised interfaces before optional reads. Earlier refusal results do not make a bulk read safe.
- **Destroyed objects keep answering.** GTK3 and Chromium objects return their old values for about 16 s after destruction, then `UnknownObject` (L17-6 to L17-8). A successful read of a stored path proves nothing in that window. GTK4 and Qt go quiet at once, with `UnknownMethod` and `UnknownObject`/`UnknownInterface` respectively (L17-1, L17-4).
- **A write to a destroyed GTK3 widget answers success** with a zeroed `GetState` and an empty name (L30-2), so a successful write is no more proof of presence than a successful read. `an-atspi-write-that-returns-success-has-not-changed-anything.md` covers the write path.
- **An exited application is a different error.** SIGKILL and clean quit both give `org.freedesktop.DBus.Error.ServiceUnknown` on the call and `NameHasNoOwner` on a bus-daemon query; never `UnknownObject` (L17-9).
- **A call in flight when the app is killed returns `NoReply`, not `ServiceUnknown`**, for the 27 to 45 ms before the next call sees the app gone (L33-12). `NoReply` is not an `AppGone` name, so a root `GetChildren` that straddles a kill is a transient non-verdict: retry it, and let the next read return `ServiceUnknown`.
- **A closed or hidden Qt dialog stays in the tree.** It keeps `ENABLED`, `RESIZABLE` and `SENSITIVE` and loses `SHOWING` and `VISIBLE` (L33-15), so a window list built from the registry reports a window that is gone. Minimized windows are not that shape: GTK 3, GTK 4, Qt and Chromium keep `SHOWING` or gain `ICONIFIED` (L33-55). A window with neither is a husk, and `crates/linux/src/system/window/ghost_window.rs` leaves it out. A state census that filters by flags can hide the husk or hide a real minimized window, so count by the full state set.
- **A timeout carries no D-Bus error name** (L17-10): a stopped process fails every read with a local `TIMED_OUT` after the client limit, and the registry still lists it. Registry presence is no liveness evidence.
- **A hidden GTK4 tab page** leaves the tree while its objects still answer every universal read (L17-5), so a direct-path read can find what a descent cannot.

## Solution

Classify in layers, as `crates/linux/src/tree/properties/absence.rs` does.

1. `Failure` sorts one failed call: `AppGone` (`ServiceUnknown`, `NameHasNoOwner`), `Suspect` (the three names above), `TimedOut`, `Other`. `Suspect` is never a verdict on its own.
2. `Liveness` is read from the node's own `Accessible.GetRole`, which every live node of every toolkit answers (L17-11). A field read that fails as `Suspect` becomes "object gone" only when `GetRole` also fails in the suspect family; otherwise the failure stays on its field (`read_failure`). `GetAll` and `GetChildAtIndex` are not valid corroboration: Qt fails the first and GTK4 childless nodes fail the second.
3. Presence is decided by the parent's child list, not by the object's own reads (L17-12): the resolver follows the stored child-index path down from the verified window frame (`path_descent.rs`), and a landing is accepted only if its evidence still matches the stored identity. A node that merely answers is not a positive. The `Parent` property is never climbed (L2-2).
4. Map to settled answers in `resolve_errors.rs`: an app that left the bus is `STALE_REF` kind `resolve_owning_process_exited` only when `/proc` shows the process gone, and `resolve_owner_unreachable` otherwise (`owner_unreachable`), because a missing bus owner does not prove an exit; a destroyed object is `resolve_target_destroyed`; a full search with no match is `resolve_no_candidate`. After that mapping, `resolve/mod.rs` checks whether `/proc` shows the owning process stopped when the error is a timeout or an incomplete identity observation (`resolution_identity_unknown`). With stopped-process evidence, both become `APP_UNRESPONSIVE` (`stopped_aware`). Without it, a timeout stays unchanged, and an incomplete observation becomes a retryable `STALE_REF`. Other errors pass through.

### Process state needs independent evidence

The lifecycle path in `crates/linux/src/system/process/process_state.rs` asks a different question from element resolution. `system/process/liveness.rs` checks the expected process-instance token against `/proc` before and after the accessibility probes: a missing, zombie, dead or replaced process is `Exited`; an unreadable stat is conservatively alive. `kill(pid, 0)` succeeds on a zombie and cannot replace that check.

`process_state_classify.rs` keeps an answered probe, a timed-out probe and an unavailable probe distinct. A stopped process is `Unresponsive` unless it answered. A live process becomes `Unresponsive` after two consecutive accessibility probes time out (`process_state.rs::observe`); one timeout or unavailable evidence leaves it `Running`. The rule was measured against a blocked main loop and healthy applications under load (ledger L31-17, L31-18), not inferred from registry presence. Returning an error for an unavailable probe would let core mistake uncertainty for exit.

## Prevention

- Do not branch on a D-Bus error name alone; ask what else the same name means on a live object, and measure it per toolkit.
- A read that succeeds against a stored path is not positive evidence of presence. Positive evidence is reaching the node by walking from a verified ancestor.
- Keep "app gone", "object gone", "timed out" and "not found" as separate answers; collapsing them turns a native failure into a confident verdict.
- Probe every toolkit before encoding a rule from one: Qt, GTK3, GTK4 and Chromium disagreed on every name here.
