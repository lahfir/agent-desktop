---
title: An enforcement gate must cover everything the binary embeds
date: 2026-08-08
category: best-practices
module: scripts, skills, crates/core/src/commands/skills.rs
problem_type: best_practice
component: tooling
severity: medium
applies_when:
  - "A repository rule is enforced by a script that scans a file set"
  - "Content outside that file set is compiled or embedded into the shipped artifact"
  - "Adding a new kind of shipped content (docs, templates, schemas, prompts)"
tags: [enforcement, scanning, shipped-source, include_str, gates]
---

# An enforcement gate must cover everything the binary embeds

## Context

This repository forbids delivery-plan references — phase numbers, sub-phase
numbers, plan decision ids like `KTD8`, plan unit ids like `U3` — in shipped
source. The reasoning is durable: they answer *when this was written*, which
stops being true the moment the roadmap moves, and they mean nothing to a
reader who does not have the plan open. Probe ledger row ids (`A4-1`) are
exempt and encouraged, because they cite a measurement that stays true.

`scripts/check-no-phase-references.sh` enforced it over `crates/` and `src/`,
matching `*.rs`. That is a precise definition of "shipped source" — and it
was the wrong one.

## Problem

The skill documentation under `skills/` is not reference material sitting
beside the code. `crates/core/src/commands/skills.rs` pulls it into the
binary:

```rust
const SKILL_DESKTOP_INTERACTION: &str =
    include_str!("../../../../skills/agent-desktop/references/commands-interaction.md");
```

So `agent-desktop skills` serves that text to an agent at runtime. Eight
plan references had accumulated there — `KTD8`, `KTD10`, `§2.8`, `§2.9` —
and the gate reported clean every time, because the file's extension was
`.md` and its directory was not in the list. An agent asking the binary how
`type` behaves on Windows was told it behaves that way "(A4-1, KTD8)", and
that `press --app` "stays not-supported until §2.9" — a promise about a
delivery schedule, shipped inside the product, to a reader who cannot
resolve either token.

Fixing the eight instances would have left the gate exactly as blind as it
was. The defect was the scope, not the instances.

## Root cause

The gate's scope was written from the *form* of shipped code (a `.rs` file
under `crates/` or `src/`) rather than from what actually reaches the user.
`include_str!` erases that distinction at compile time: embedded content is
as shipped as any expression around it. Any mechanism with the same
property — a build script that bakes in a template, a schema compiled into a
validator, a prompt embedded in a tool — moves content across the same line
without moving the file.

## Solution

Extend the scan to the embedded set and say why in the script, so the next
reader does not narrow it back:

```bash
# Scope is shipped source only - crates/, src/, and the skill markdown under
# skills/. The skill docs are `include_str!`d into the binary
# (crates/core/src/commands/skills.rs), so a plan id written there is served
# to an agent by `agent-desktop skills` exactly as if it had been typed into
# a .rs file.
```

The widened scan immediately failed on a ninth instance nobody had reported —
`SKILL.md` describing "the macOS Phase 1 adapter" — which is the expected
result of fixing a scope rather than a list.

Invert-verify the widened gate the same way as any other: plant a violation
in the newly covered area, confirm the gate fails, remove it, confirm it
passes. A scope extension that was never observed failing on the new set has
not been shown to cover it.

## Prevention

- When a rule says "shipped source", enumerate what the binary embeds, not
  what has a source extension. Grep for `include_str!`, `include_bytes!`,
  and build-script emissions to find the real boundary.
- When adding an embedding, ask which existing gates assumed it was not
  there. Embedding is a scope change to every scanner in the repo.
- Fix the scope, then let it find the instances. If a gate is widened and
  reports nothing new, be suspicious that the widening did not take effect.

## Related

- [Fix the class, not the reported instance](fix-the-class-not-the-reported-instance.md)
- [A verification gate is code and needs its own test](a-verification-gate-is-code-and-needs-its-own-test.md)
- [Never ship platform code that CI cannot execute](never-ship-platform-code-that-ci-cannot-execute.md)
