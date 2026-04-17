---
title: Stamp build artifacts at a deterministic path for CI and scripts
date: 2026-04-16
category: best-practices
module: crates/ffi
problem_type: best_practice
component: tooling
severity: medium
applies_when:
  - A build script (cbindgen, bindgen, codegen) writes an artifact to $OUT_DIR whose path includes a cargo-generated hash
  - CI or a developer script needs to locate that artifact after the build finishes
  - The obvious shell-out (`find target -path '*/out/<artifact>' | head -1`) silently picks the wrong file when multiple cached build dirs coexist
  - You want a drift check (`diff committed_copy generated_copy`) that reliably fails when the committed copy is stale
tags:
  - build-rs
  - cbindgen
  - ci
  - reproducibility
  - determinism
  - cargo
  - rust-patterns
---

## Problem

Cargo puts build-script artifacts under a hash-randomized directory:

```
target/<profile>/build/<crate-name>-<hash>/out/<artifact>
```

The `<hash>` is not documented, not stable across rebuilds of the same
code on different toolchain minor versions, and you can end up with
several `<crate-name>-<hash>/` dirs in the same warm `target/` cache
(e.g. after a `cargo clean -p` and rebuild, or rustup flip). Any CI
step or developer script that resolves the artifact with
`find target -path '.../<artifact>' | head -1` is picking arbitrarily
among those dirs. Under drift check:

- If `head -1` picks the current build's artifact → drift correctly
  surfaces when the committed copy is stale.
- If `head -1` picks a stale leftover → you either self-heal (false
  green) or report stale-vs-stale (useless).

The failure mode is silent: a CI step says "OK: header in sync" while
the committed header is actually out of date, and the bad ABI ships.

## Solution

Have `build.rs` write a stable marker file containing the absolute path
of the just-generated artifact. Downstream consumers read the marker
instead of guessing:

```rust
// crates/<your-crate>/build.rs
fn main() {
    // ... generate $OUT_DIR/artifact ...

    if let Some(target_root) = target_root_from_out_dir(Path::new(&out_dir)) {
        let stamp = target_root.join("ffi-header-path.txt");
        let _ = std::fs::write(&stamp, out_path.to_string_lossy().as_bytes());
    }
}

/// OUT_DIR = {target}/{profile}/build/{pkg-hash}/out
fn target_root_from_out_dir(out_dir: &Path) -> Option<PathBuf> {
    let mut current = out_dir;
    for _ in 0..4 {
        current = current.parent()?;
    }
    Some(current.to_path_buf())
}
```

CI and scripts read the marker:

```yaml
- name: Drift check
  run: |
    STAMP=target/ffi-header-path.txt
    test -f "$STAMP" || { echo "FAIL: stamp missing"; exit 1; }
    GENERATED=$(cat "$STAMP")
    test -f "$GENERATED" || { echo "FAIL: stamped path missing"; exit 1; }
    diff -u crates/ffi/include/artifact.h "$GENERATED"
```

## Why this works

- **One writer**: the build script's own invocation knows exactly which
  `OUT_DIR` it ran in. Stamping the path at that moment captures it
  authoritatively; no later tool has to reconstruct it.
- **Stable path**: `target/<marker>.txt` lives one directory above the
  hashed build dirs and is overwritten each build. CI's cache system
  sees it as content of `target/` — no special allowlisting needed.
- **Fail-fast wrapper**: the marker file's absence is itself a signal
  that the build script didn't run (e.g. `cargo check` instead of
  `cargo build`). CI should fail rather than fall back to a wrong
  default.

## When NOT to use this

- If your build script generates artifacts deterministically **at a
  fixed location outside `OUT_DIR`** (e.g. directly into a committed
  dir), you don't need a marker — the path itself is stable.
- If the artifact lifecycle is driven by `cargo metadata` queries
  (e.g. `cargo metadata --format-version=1 | jq .target_directory`),
  that's already deterministic. Stamping is only needed when the
  specific pkg-hash subdirectory matters.

## Sibling anti-pattern to avoid

Do **not** have `build.rs` copy the generated artifact into the source
tree (e.g. `fs::copy(&out_path, &committed_path)`). That mutates the
working tree during every build, which means:

- `git diff` is polluted by invisible copies.
- The drift check (`git diff --exit-code committed_path`) can
  self-heal a stale committed copy instead of catching it.

The committed copy is the ABI contract; updating it should be an
explicit developer action (dedicated script) or CI-only step.

## References

- `crates/ffi/build.rs` — the stamping logic
- `.github/workflows/ci.yml` — "FFI header drift check" step
- `scripts/update-ffi-header.sh` — developer-facing refresh script
- Todo `006-ready-p2-deterministic-ffi-header-drift-path` — the bug this
  pattern resolves
