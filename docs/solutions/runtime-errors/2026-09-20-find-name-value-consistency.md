---
module: agent-desktop-core
date: 2026-09-20
problem_type: runtime_error
tags: [find, naming, numbers, consistency]
---

# Find output invented names from editable content

The owned Numbers editor has no accessible name and value `seed-target`.
Role-only find returned `name: seed-target`, but exact name search returned
zero matches. Raw AX confirmed AXTitle was absent and AXValue was seed-target.

Both preliminary match formatting and selected-match hydration substituted
value, description or synthetic placeholder text into the output name. The
matcher correctly kept these fields separate. The repair removes those output
fallbacks and omits empty names through the existing match data serializer.
Values remain available under `value` and searchable with `--value`. Matching,
ref identity, recovery and native mutation paths are unchanged.

The Rust field remains String; JSON consumers must now tolerate an omitted name
for unnamed or unknown-name matches. This deliberately changes the misleading
output contract. No extra display-label field or name-matching fallback is added.
The test-only snapshot matcher now uses the same match data serializer.

Regression checks cover known, absent, empty and unknown names while preserving
values; selected hydration also verifies omitted unknown names. Workspace and
focused find tests, formatting and Clippy pass. An initial sandbox core run had
13 loopback-permission failures; the escalated workspace run passed all tests.

Live candidate `f04ade8e00bac484be1a150b5c01fcc2110b7d6d58f396e3ae71e1cc42fd5a0e`
returned one Numbers editor for table-rooted exact value search, with value
seed-target and no name. Qualified ref: `@s1msk6byapmamx:e1`.
This repairs output/query consistency; Numbers text mutation remains unresolved.
