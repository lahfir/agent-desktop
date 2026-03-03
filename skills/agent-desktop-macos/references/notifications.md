# Notification Center

agent-desktop interacts with macOS Notification Center via the accessibility API.

## How It Works

1. **NcSession** opens Notification Center by clicking the clock in ControlCenter (if not already open)
2. Notifications are read from the AX tree under the NotificationCenter process
3. After operations, NcSession closes NC and restores the previously focused app
4. The `Drop` impl ensures cleanup even on errors

## Dismiss Strategy

Headless-first approach (no cursor movement unless needed):

1. **AXDismiss** / **AXRemoveFromParent** — native accessibility actions
2. **Close button** — find and press AXButton named "close", "clear", or "dismiss"
3. **Hover + close button** — move cursor to reveal hidden close button, then press it
4. If all fail, returns `ACTION_FAILED`

**Important:** `AXPress` is intentionally excluded from dismiss — it "clicks" the notification body (opening the source app) without actually dismissing it.

## Stacked Notifications

macOS groups notifications from the same app into stacks. Dismissing the top notification may reveal more underneath. `dismiss-all-notifications` iterates in reverse order but may need multiple rounds for deeply stacked groups.

## Troubleshooting

| Problem | Cause | Fix |
|---------|-------|-----|
| Script Editor opens during dismiss | Notification owned by osascript | Fixed — NcSession restores previous app focus |
| `dismiss-notification` reports success but notification stays | AXPress was firing instead of AXDismiss | Fixed — AXPress removed from dismiss chain |
| Calendar widget can't be dismissed | It's a system widget, not a notification | Expected behavior — not a real notification |
| Notifications disappear before listing | Banner-style notifications are transient | Use `wait --notification` to detect them |
