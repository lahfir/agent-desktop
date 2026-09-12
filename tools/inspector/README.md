# agent-desktop inspector

For an interactive tree and screenshot, run from the repository root with Node 22+:

```bash
cargo build --release -p agent-desktop
npm --prefix tools/inspector run dev
```

Choose an app and select **Inspect app**. Tree arrows load children through the CLI; selecting a row isolates its highlight. **Show all** restores the overview. The right panel provides root-scoped find and property/state reads, with a progress toast while commands run.

No npm dependencies or global tools are needed. The bridge opens a localhost URL, retries busy ports, and keeps separate temporary snapshot state. Use the full launch URL: its fragment carries the capability token, which is not served in public HTML. Accessibility permission is required; without Screen Recording permission the inspector falls back to the accessibility tree and shows a warning. It is read-only and uses the same screenshot styling as saved debug HTML. Keep one active view per server and refresh after app changes or a failed drill, which may already have replaced stored refs. Normal shutdown removes private state; `SIGKILL` or a system crash can leave temporary files. Run tests with `npm --prefix tools/inspector test`.

This is a tool for human developers working on agent-desktop. It is not part of
the agent-facing command surface, and an AI agent driving the CLI has no use for
it. The agent-invocable equivalent is `snapshot --debug --screenshot PATH.html`,
documented in `skills/agent-desktop/references/commands-observation.md`.
