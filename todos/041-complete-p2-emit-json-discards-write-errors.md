---
status: pending
priority: p2
issue_id: "041"
tags: [code-review, data-integrity, correctness]
---

# emit_json Silently Discards Serialization and Write Errors

## Problem Statement

`emit_json` in `main.rs` discards both the serialization result and the write result with `let _`. The BufWriter is dropped without explicit flush, so flush errors are also silently swallowed. A calling agent receives no JSON on stdout and has no way to distinguish this from a successful empty response vs a pipe error. The process exits with code 0 despite producing no output.

## Findings

**File:** `src/main.rs:122-127`

```rust
fn emit_json(value: &serde_json::Value) {
    let stdout = std::io::stdout();
    let mut writer = BufWriter::new(stdout.lock());
    let _ = serde_json::to_writer(&mut writer, value);  // error discarded
    let _ = writer.write_all(b"\n");                     // error discarded
    // BufWriter drop: flush errors silently swallowed
}
```

On a broken pipe (agent killed the read end), this returns normally and `main()` exits 0. The agent gets no output and no error code.

## Proposed Solutions

### Option A: Flush explicitly and handle errors (Recommended)
```rust
fn emit_json(value: &serde_json::Value) {
    let stdout = std::io::stdout();
    let mut writer = BufWriter::new(stdout.lock());
    if serde_json::to_writer(&mut writer, value).is_err()
        || writer.write_all(b"\n").is_err()
        || writer.flush().is_err()
    {
        std::process::exit(3);  // distinct exit code for I/O failure
    }
}
```
- **Effort:** Tiny
- **Risk:** Low — explicit flush guarantees output or a known failure

### Option B: Use writeln! and propagate errors to main
Return `Result<(), io::Error>` from `emit_json`, handle in `main()`.
- **Effort:** Small
- **Risk:** Low

## Recommended Action

Option A: explicit flush with a distinct exit code (3) for stdout I/O failure.

## Technical Details

- **File:** `src/main.rs`
- **Lines:** 122–127
- **Component:** JSON output path

## Acceptance Criteria

- [ ] `emit_json` flushes the BufWriter explicitly
- [ ] Serialization/write errors result in non-zero exit code
- [ ] Broken pipe does not produce exit code 0 with no output

## Work Log

- 2026-02-19: Finding identified by data-integrity-guardian review agent
