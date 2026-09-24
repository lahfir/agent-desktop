# Background input (`--background`, macOS)

`--background` is an explicit opt-in, best-effort synthetic-input mode. It posts synthetic events straight to the process that owns one exact window instead of driving the shared OS pointer. It is macOS only and is rejected together with `--headed`. The default paths are unchanged: without a flag, ref actions stay semantic and headless; with `--headed`, cursor commands drive the real cursor.

Use it only when the semantic path cannot reach a control, for example hover-only UI in an Electron app on a hidden workspace. Prefer semantic ref actions everywhere else.

What it does and does not promise:

- **The real cursor stays put** and the target window is not raised.
- **Focus preservation is best effort.** The target process decides how to react to posted events and may activate itself. A focus guard restores the user's frontmost app when the *target* takes the front, but it never fights a switch to any other app (that is treated as the user switching). Always read `focus_change` and `focus_guard` in the result.
- **Private macOS SPI.** Delivery uses private SkyLight entry points (`SLEventPostToPid`, `SLPSPostEventRecordTo`, `_SLPSSetFrontProcessWithOptions`, `CGEventSetWindowLocation`) resolved at runtime with `dlsym`. A missing symbol degrades that technique (reported in `degraded`) instead of failing to load; posting then falls back to the public `CGEventPostToPid`.
- **Delivery is unverified.** The app may drop the event. Confirm the effect with a fresh `snapshot`.

## Pointer: `hover`, `mouse-move`, `mouse-click`

```bash
agent-desktop hover @s8f3k2p9:e5 --background
agent-desktop hover --background --window-id w-9555 --xy 500,300
agent-desktop mouse-click --background --window-id w-9555 --xy 500,300
agent-desktop mouse-move --background --window-id w-9555 --xy 500,300
```

- **Target.** A ref hover takes the process, process instance, and exact window from the ref and aims at the element's live center. `--xy` has no identity of its own, so it requires `--window-id` (from `list-windows`); `--window-id` without `--background`, or alongside a ref, is `INVALID_ARGS`. Batch entries use `"background": true` and `"window_id"`.
- **Geometry.** The point must lie inside the target window's bounds (`INVALID_ARGS`, `not_delivered` otherwise). The window may be offscreen, on another workspace, or covered: pid-targeted delivery skips the window server's hit test, so there is no occlusion check.
- **Identity.** The window is re-verified against its pid and process instance under the interaction lease immediately before posting; a mismatch fails as `STALE_REF` with nothing delivered.
- **`--wait-for`.** A post-action wait observes the target window (ref or `--window-id`), never the user's frontmost app.

VS Code on a hidden workspace: `snapshot --app Code --window-id w-9555 -i`, then `hover <explorer-header-ref> --background`, re-snapshot, and `click <new-file-ref>` (semantic `AXPress`).

## Result

Success carries `disposition: { delivery: "delivered_unverified", retry: "unsafe" }` and `background: { pid, window_id, focus_change, layers, frontmost_pid_before?, frontmost_pid_after?, degraded?, focus_guard? }`.

| Field | Meaning |
|-------|---------|
| `focus_change` | `unchanged`; `restored` (the target briefly took the front and the user's app came back; top-level `warning`); `changed` (the frontmost app differs afterwards; `warning`); `unknown` (a frontmost sample was unreadable) |
| `layers` | The delivery techniques that were requested. Whether each one worked is reported by `degraded` |
| `degraded` | Requested techniques that were unavailable or failed, e.g. `skylight:SLEventPostToPid_unavailable` |
| `focus_guard` | `{ interventions, restored, max_steal_ms, yielded }`. `yielded: true` means a third app became frontmost, so the guard stopped without switching back |

A repeated hover is harmless, but the contract still says `unsafe`: observe the effect with `snapshot` instead of retrying.

## Deadline and partial delivery

The command deadline, including an enclosing batch deadline, bounds the whole delivery. It is checked before every event that starts something new (a move or a button down). A button that is already down is always released. When the budget runs out, the command stops there and returns `TIMEOUT`:

- nothing posted yet: `not_delivered`, `retry: safe`;
- some events posted: `delivered_unverified`, `retry: unsafe`, with `details.delivered_events` and `details.planned_events`. Snapshot before deciding what to do next.

## Limits

The app decides what to do with the event. Sandboxed or hardened apps may drop it. Chromium/Electron honors a background `mouseMoved` only while the window is still its app's main window and may swallow a first click in an inactive window. The cursor overlay is not shown.

## Diagnosis

`AGENT_DESKTOP_BG_LAYERS` narrows the recipe for live diagnosis and is deliberately not a CLI flag: unset selects `route,skylight,activate,guard`; `none` selects bare `CGEventPostToPid`; otherwise list any of `route`, `skylight`, `activate`, `guard`, `primer`. See `docs/solutions/best-practices/background-pointer-delivery-2026-09-23.md`.
