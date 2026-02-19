# Test Fixtures

Golden JSON snapshots for regression testing.

## Populating Fixtures

Run the following on macOS with Accessibility permissions to capture real snapshots:

```bash
cargo build --release
./target/release/agent-desktop snapshot --app Finder > tests/fixtures/finder-snapshot.json
./target/release/agent-desktop snapshot --app TextEdit > tests/fixtures/textedit-snapshot.json
./target/release/agent-desktop list-apps > tests/fixtures/list-apps.json
```

## Usage in Tests

Fixture files are loaded in tests to assert serialization stability:

```rust
let expected: Value = serde_json::from_str(
    include_str!("../fixtures/finder-snapshot.json")
).unwrap();
```

Any change to the JSON output contract must be accompanied by updated fixtures.
