---
name: agent-desktop-macos
version: 0.8.4
tags: macos-automation, accessibility, ax, tcc, ai-agent, gui-automation, cli
requirements:
  - agent-desktop
description: >
  macOS platform guide for the agent-desktop CLI. Use for desktop automation
  on macOS: granting Accessibility and Screen Recording permission (TCC),
  what headed mode does on macOS (physical-first click, type and scroll),
  surfaces (menu, menubar, sheet, popover, alert), Notification Center,
  Chromium and Electron apps including launch --cdp, and fixing PERM_DENIED,
  empty accessibility trees, ACTION_FAILED and APP_UNRESPONSIVE.
---

# agent-desktop on macOS

Read the core skill first (`agent-desktop skills get desktop`) for the observe, act, verify loop. This skill covers only what is different on macOS. Symptoms and fixes are in [references/troubleshooting.md](references/troubleshooting.md).

## Requirements

- macOS 13 or later, Intel or Apple Silicon.
- Accessibility permission for the app that launches agent-desktop (Terminal, iTerm2, Warp, VS Code, Codex). If System Settings lists the `agent-desktop` binary separately, grant it too.
- Screen Recording permission, only for `screenshot` and for `session start --screenshots` artifacts.
- Automation permission for System Events, only where a command reports it missing.

## First contact

1. Run `agent-desktop status`. It includes the permission report and the state root.
2. Run `agent-desktop permissions`. It reports `accessibility`, `screen_recording` and `automation`. Each is `granted`, `denied`, `not_required` or `unknown`. A denied entry carries a `suggestion`.
3. If a permission is denied, run `agent-desktop permissions --request`. It shows the system prompt.
4. Open System Settings > Privacy & Security > Accessibility. Turn on the launching app.
5. Restart the launching app. A running process does not pick up a new grant.
6. Run `agent-desktop snapshot --app Finder -i` to confirm.

`permissions` does not prompt. `permissions --request` can prompt, including for Automation. Automation reports `unknown` when macOS would need to prompt.

## What differs on macOS

**Headless by default.** Ref actions use accessibility actions only. They do not steal focus, move the cursor, send keys, or write the pasteboard. Add the global `--headed` flag only when you want physical input.

**Headed is physical first** for `click`, `right-click`, `type`, `clear` and `scroll`. Core focuses the exact window of the ref. The adapter then sends a real click, key or wheel event to the verified point. `expand`, `collapse`, `set-value`, `select`, `toggle`, `check`, `uncheck`, `focus` and `scroll-to` stay semantic under `--headed`. Double-click, triple-click, `hover` and `drag` need `--headed`. Raw `--xy` input never changes focus.

**Judge delivery by observation.** AX return codes do not prove an effect. Finder returns success from an `AXConfirm` that does nothing. Snapshot again or use `get` and `is` to check the result.

**Surfaces.** Run `list-surfaces --app "App"` to see what is open. When a menu, sheet, alert or popover is open, snapshot it with `--surface menu|sheet|alert|popover`. Refs from the full window tree behind an overlay are the wrong target. Use `--surface menubar` for File, Edit and View menus. Menu bar apps (status items) also use `menubar`. After the overlay closes, snapshot the window again.

**Menus.** `right-click` headless uses `AXShowMenu`. If the menu opened but the app returned `APP_UNRESPONSIVE` with `retry: unsafe`, do not retry. Inspect the menu with `--surface menu`. For combo boxes and menu buttons, use `select @ref "Option"`. `select` needs a closed-to-open menu transition, so dismiss any open menu first.

**Notification Center.** `list-notifications`, `dismiss-notification`, `dismiss-all-notifications` and `notification-action` drive the system Notification Center through accessibility.
- Every mutation needs `--headed`. Listing works headless only if the center is already open.
- When agent-desktop opens the center, it closes it and restores the previous app focus afterwards.
- Pass `--expected-app` and `--expected-title` on a single dismiss or action, because indexes shift when the stack changes.
- Use `wait --notification --app "App"` for banners that vanish before you list them.

**Chromium and Electron.** Accessibility is the default path and the only path for native surfaces (menus, dialogs, window chrome) and for apps that are already running. agent-desktop switches on the web accessibility of a Chromium renderer automatically when it needs it. For a fresh launch, `launch "App" --cdp` exposes a DevTools port that a CDP client reads faster than a deep tree walk. For a dense running app, use `snapshot --skeleton -i --compact`, then drill with `--root @ref`.

**Identity.** `launch` accepts a display name (`"System Settings"`) or a bundle ID (`com.apple.systempreferences`). Other commands take the display name through `--app`. Window IDs such as `w-4521` stay valid for the life of the window. Find them with `list-windows`.

**Keys.** The primary modifier is `cmd`: `press cmd+c`, `press cmd+s`. Quit shortcuts such as `cmd+q`, `cmd+shift+q`, `cmd+alt+esc`, `ctrl+cmd+q` and `cmd+shift+delete` return `POLICY_DENIED` unless you pass `--force`. To quit an app, use `close-app`. Punctuation keys work in combos, as the symbol (`,` `.` `/` `;` `'` `[` `]` `\` `-` `=` `` ` ``) or by name (`comma`, `period`, `slash`, `semicolon`, `quote`, `leftbracket`, `rightbracket`, `backslash`, `minus`, `equal`, `grave`). Each one is sent with the key that types it in the active keyboard layout, so `press cmd+,` opens Settings on ANSI and JIS keyboards alike.

**Cursor overlay.** `cursor-overlay enable` draws an agent cursor in a click-through window. It does not move the OS pointer, activate an app or change how a command is delivered. It honours Reduce Motion. `--image` and `--pointer-image` accept PNG only, up to 2 MiB and 1024 pixels per side. `--size` scales images at the display backing scale; missing or invalid images fall back to the arrow.

## Next step

When a command fails, find the symptom in [references/troubleshooting.md](references/troubleshooting.md).
