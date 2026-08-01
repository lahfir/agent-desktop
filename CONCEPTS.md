# Concepts

Shared domain vocabulary for this project -- entities, named processes, and status concepts with project-specific meaning. Seeded with core domain vocabulary, then accretes as ce-compound and ce-compound-refresh process learnings; direct edits are fine. Glossary only, not a spec or catch-all.

## Desktop Observation

### Accessibility Tree
A structured representation of an application's user interface exposed by the operating system accessibility APIs and used by agent-desktop as the source of truth for observation and semantic interaction.

### Snapshot
An observation of an accessibility tree at a point in time, persisted with the element refs allocated from that observation.

### Snapshot ID
A compact identifier for one persisted snapshot. Lookup is confined to the selected session namespace, so an ID created in a session is not a cross-session handle.

### Surface
A scoped UI layer that can be observed separately from the whole window, such as an open menu, sheet, popover, alert, or focused area.

### Drill-down
A snapshot operation that starts from an existing ref to observe that element's subtree instead of re-reading the entire window.

## Vocabulary

The four platform-neutral vocabularies every adapter produces and core consumes. They were single-platform code types until two adapters produced them; they are shared contracts now, and an adapter that emits a token outside one of them is emitting something no consumer can act on.

### Role
The canonical kind of a control, drawn from a closed set core owns.

Each platform maps its own taxonomy onto it — macOS from `AXRole` plus its subrole fold, Windows from UIA's `ControlType` refined by pattern availability — and never invents a token. The platform taxonomies are not parallel: UIA's `Tab` is the container and its `TabItem` is the page selector, which is the inverse of the ARIA naming core follows, and several canonical roles (`switch`, `colorwell`) have no control type at all and are reachable only through refinement or not at all. A role core does not recognise is `unknown`, which is a positive statement about the element and is distinct from a read that failed.

### State Vocabulary
The closed set of state tokens a node may carry, defined by core's `STATE_VOCABULARY`.

Adapters emit only members of it, and a membership assertion is paired with a negative control so it cannot pass vacuously. A token is emitted only where the platform evidenced it: where a platform has no source for a reserved token, the token stays unproduced rather than defaulted. Emitting from an ungated source is the characteristic failure here — a property that reports a plausible value on an element whose provider never implemented the underlying pattern will decorate every inert node in the tree with states it does not have.

A role mapping can put a reserved token permanently out of reach on one platform without the token itself being wrong. The same logical control — a toggle button — surfaces as `role: button` with state `pressed` on macOS, because macOS keeps the control's role as `button` and reads its toggle value as `pressed`. Windows resolves the identical control to `role: switch` with state `checked` instead, because Windows reclassifies any `Button` control type that advertises toggle support to `switch` before states resolve, so the `role == button` precondition a `pressed` arm would need can never hold there. `pressed` therefore stays unproduced on Windows, deliberately, and the two adapters disagree on both the role and the state token for the same UI. This is a known, deliberate divergence for the current phase, not a bug — the cross-platform convergence question is owned by the Hardening & Integration Review sub-phase in `docs/phases.md`.

### Name Evidence
The raw slots an adapter supplies so that **core**, not the adapter, computes the accessible name.

The slots are ranked by one precedence shared across platforms, and each slot carries its own read status, so uncertainty travels: when a source that would have outranked the winner failed to read, the name is unknown rather than the weaker source's value. A platform folds its own gating — which slots apply to which roles, whether children were fully enumerated — into those statuses before calling, so the shared computation never sees a platform-specific token. An adapter that computes its own name is a second precedence, and two precedences drift.

### Native ID
The strongest developer-assigned identifier a platform exposes for an element, carried in `native_id`.

Windows supplies UIA's `AutomationId`, macOS `AXIdentifier` or `AXDOMIdentifier`, Linux AT-SPI's `accessible-id`. It is typed rather than bare: an identifier whose kind is unknown is rejected at persistence, so the kind travels with the value. A blank value is no identifier at all — publishing one would give every unidentified element the same key. A read that failed is incomplete evidence rather than an absent identifier, because "absent" satisfies completeness gating and a target that never answered must not. Coverage varies by an order of magnitude across UI stacks, so it is a strong hint for re-identification and never a sufficient key alone.

## Refs And Identity

### Ref
A short element identifier assigned by agent-desktop to an actionable or drillable node in a snapshot.

Refs are deterministic inside one snapshot but are not stable across UI changes. Snapshot and find output qualify each ref with its snapshot ID. Legacy bare refs require the producing snapshot ID as a separate argument.

### RefMap
The persisted mapping from refs to the identity evidence needed to re-identify elements later.

### Stable Text Identity
The role-conditional text evidence used during strict ref resolution.

Names and descriptions can identify a ref when they are stable labels. Mutable control values, including text field content and value text promoted into an accessibility name, are volatile and do not identify the element by themselves. Core owns this policy so macOS, Windows, Linux, CLI, and FFI consumers share the same semantics.

### Stale Ref
A ref whose stored identity no longer matches a live element strongly enough to act safely.

### Strict Ref Resolution
The fail-closed process of re-identifying a ref from stored identity evidence before a command acts on it.

Strict ref resolution rejects missing, stale, and ambiguous matches instead of guessing. It is the boundary between an old observation and a live desktop mutation.

## Coordination

### Session
An on-disk container under `~/.agent-desktop/sessions/<id>/` that owns snapshot refmaps, an optional trace directory, and a `session.json` manifest.

`session start` writes the manifest (`trace: on` unless `--no-trace`) and pre-creates `trace/` when tracing is on. It returns the new ID but does not activate it for later processes. Explicit `--session` takes precedence over `AGENT_DESKTOP_SESSION`; with neither, commands use the global, non-session namespace. Bare `--session <id>` without a manifest remains snapshot-namespace-only for backward compatibility.

Use sessions when callers want a coordinated snapshot namespace and trace sink. Every lookup is confined to its selected namespace, so a snapshot created under a session requires that same `--session` or `AGENT_DESKTOP_SESSION` scope later. Qualified refs remain the deterministic path for pinned actions inside that namespace.

### Session Manifest
The `session.json` file describing one session: id, optional name, created/ended timestamps, and `trace: on|off`.

Structured file tracing activates only when the manifest has `trace: on`. FFI adapters and bare `--session` ids without this manifest do not write trace segments.

### Trace Segment
One append-only JSONL file per OS process under `<session>/trace/<pid>-<procStartTs>.jsonl`, written lazily with atomic lines. Each new segment opens with a `trace.meta` header (`schema`, binary version, `os`, `pid`, `proc_start_ms`, `session_id`). Older traces without meta read as schema 0. Explicit `--trace <path>` overrides to a single file.

### Trace Timeline
The merged, deterministic ordering of all events from every segment in a session, produced by `trace show` and `trace export`. Merge key is `(ts_ms, writer pid, in-file position)`; the reader tolerates truncated tails, corrupt lines, and foreign files with counted warnings rather than hard errors.

### Trace Schema
Additive-only evolution contract: new event types and optional fields may appear; existing meanings never change. Readers ignore unknown content. Segments declare their schema in the leading `trace.meta` line; unknown future schemas warn and parse best-effort.

### Replay Artifacts
Opt-in capture mode (`session start --screenshots`, manifest `artifacts: full`) that stores pre/post-action PNGs under `<session>/trace/screens/` and refmap copies under `<session>/trace/refmaps/`. Event-mode traces (`artifacts: events`, the default) record JSONL only. Artifacts are unredacted and may appear in exported HTML — treat them like screenshots.

### Protected Process
A session-critical operating-system process that agent-desktop refuses to close on every surface, because terminating it would break the user's desktop session.

The refusal is enforced where the close happens, so CLI, FFI, and any future consumer behave identically. Matching is exact — a process name or a bundle-identifier component, never a substring — so lookalike applications that merely contain a protected name stay closable.

## Action Reliability

### Actionability
The pre-dispatch judgement that a resolved element is safe to act on, based on native evidence such as visibility, stability, enabled state, supported action, policy, and editability.

### Capability Vocabulary
The platform-neutral set of supported action names that core uses to compare command intent with native adapter evidence.

Each adapter maps native primitives into this shared vocabulary before core evaluates actionability. New commands should extend the central vocabulary first, then reuse it from actionability, ref allocation, predicates, FFI tests, and platform adapters.

### Interaction Policy
The side-effect contract attached to an action request, controlling whether the command may steal focus, move the cursor, or use physical input. Ref commands expose exactly two modes: **headless** (the default — accessibility-only, no cursor, fails closed when the semantic path is unavailable) and **headed** (opt-in via the global `--headed` flag — authorizes the action's declared focus and cursor preconditions).

Core owns those preconditions through `HeadedRequirement`: `FocusedWindow` for keyboard or focus-sensitive work, and `FocusedWindowAndCursor` for pointer delivery. For ref actions, core focuses the exact source window before dispatch and resolves a verified target point for pointer work; the platform adapter owns the OS-specific focus primitive and delivery mechanism. Raw coordinate input has no ref identity, so it never infers or focuses a window. On macOS, headed `click`, `right-click`, `type`, `clear`, and `scroll` are physical-first; double/triple-click, hover, and drag are physical-only; expand/collapse and the remaining semantic actions stay semantic after the core focus precondition.

### Headless Ref Action
A ref-based action that uses semantic accessibility operations without implicit focus stealing, cursor movement, synthetic keyboard input, or pasteboard use. This is the default mode.

Headless ref actions may still fail when the native accessibility API cannot perform the requested semantic operation; they fail closed with structured actionability or policy errors rather than silently substituting physical input. The broader **headed** policy must be selected explicitly with `--headed`.

### Action Chain
The ordered ladder of strategies a ref action walks to perform one intent, with each step gated by policy and its delivery evidence recorded. The order is action-specific: natural input may put a headed physical step first, while semantic state changes use accessibility actions or settable attributes only.

The chain pins one execution deadline at its start (distinct from the Resolver Deadline, which budgets re-identification) and every step observes it. Expiry while a step may have partially mutated the element surfaces as a structured timeout carrying the observed state, never as a plain step failure — the caller must be able to tell "nothing happened" from "something may have happened".

### Wait Predicate
The condition a wait command polls for before returning, such as element actionability, text presence, window appearance, menu state, or notification arrival.

### Resolver Deadline
The remaining time budget carried through strict ref resolution so every native read can fail with a structured timeout instead of using an unrelated platform default timeout.

### Coordinate Fallback
An explicit opt-in path that uses screen coordinates or physical input when semantic accessibility operations cannot perform the requested action.

Ref-targeted physical input lands on the topmost window at the resolved point, so core first ensures the target element's exact window is frontmost — the app being frontmost is not sufficient when the element lives in a background window of that app. Raw `--xy` input carries no window identity and therefore moves/clicks at the requested coordinates without focusing any application.

### FFI Ref-Action Parity
The requirement that language bindings using refs follow the same strict resolution, actionability, and interaction-policy semantics as CLI ref commands.

## Platform Evidence

### Probe Corpus
The committed, re-runnable set of raw platform scripts under `probes/<platform>/` that a platform exploration sub-phase (2.0, 3.0) uses to observe real OS behavior before any adapter code exists. Probes capture their outputs beside the scripts; they never modify product code.

### Findings Ledger
The `FINDINGS.md` file inside a probe corpus mapping every experiment to observed behavior and a doc-alignment verdict (confirms / contradicts / new edge) against `docs/phases.md`. A contradicts verdict obligates a same-PR correction of `docs/phases.md`; the ledger being complete is the gate that unblocks the platform's adapter sub-phases.

## Relationships

A session owns one latest-snapshot pointer, an optional manifest-gated trace directory, and persisted snapshot refmaps. A snapshot persists a ref map and can be selected by ID within that same namespace. A ref resolves through strict ref resolution into live native evidence, then actionability decides whether the action can safely dispatch. In headed mode, core applies the action's focus/cursor requirement before the platform adapter executes the action-specific chain under its own deadline. FFI ref-action parity keeps that same relationship true for language bindings.
