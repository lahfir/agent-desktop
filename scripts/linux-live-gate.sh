#!/usr/bin/env bash
# The Linux live gate: isolated library suite, or the attended owner-window registry.
# Usage: scripts/linux-live-gate.sh [--tier all|openbox|mutter|wayland] [--jobs N] [--output DIR]
#
# --dry-run prints every leg id and exits without touching a display. A run
# without --owner refuses a live session, because the legs read the owner's
# desktop apps, shell windows, clipboard and consent dialogs.
set -euo pipefail

case "${1:-}" in
  --owner|--dry-run) ;;
  *) exec python3 "$(dirname "$0")/linux-live-gate.py" "$@" ;;
esac

legs() {
  cat <<'TABLE'
OWNER-01	restore-atspi	L24-31
OWNER-02	live-ignored-3.5	L25-1 L25-2 L25-3 L25-4 L25-5 L25-6 L25-7 L25-8 L25-9 L25-10 L20-3
OWNER-03	chrome-password-field	L25-6
OWNER-04	firefox-offscreen	L16-15 L27-44
OWNER-05	wayland-press-type-refusal	L5-3
OWNER-06	portal-consent-approve	L29-25 L29-29
OWNER-07	portal-consent-reject	L29-25 L29-29
OWNER-08	portal-consent-close	L29-25 L29-29
OWNER-09	portal-consent-timeout	L29-25 L29-29
OWNER-10	portal-consent-repeat	L29-25 L29-29
OWNER-11	portal-consent-new-process	L29-25 L29-29
OWNER-12	screencast-session-sources-start	L9-17 L13-21 L29-28
OWNER-13	screencast-pipewire-frame	L9-17 L13-21 L29-28
OWNER-14	screencast-restore-token	L29-28
OWNER-15	screenshot-screen-one-scale	L29-30
OWNER-16	screenshot-screen-mixed-scale	L29-30
OWNER-17	wayland-clipboard-paste	L13-25
OWNER-18	menu-item-click	L32-21 L32-39
OWNER-19	second-monitor-scale-2-click	L32-46
OWNER-20	non-english-shell-landmarks	L32-40
OWNER-21	tray-corner-cases	L32-16
OWNER-22	gecko-menus	L32-16
OWNER-23	vscode-menus	L32-16
OWNER-24	gtk3-header-bar-popover	L32-16
TABLE
}

dry_run=0
owner=0
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) dry_run=1; shift ;;
    --owner) owner=1; shift ;;
    -h|--help)
      echo "usage: scripts/linux-live-gate.sh [--dry-run] [--owner]" >&2
      exit 0
      ;;
    *)
      echo "linux-live-gate: unknown argument $1" >&2
      exit 2
      ;;
  esac
done

if [ "$dry_run" = 1 ]; then
  printf 'leg\tname\tsources\n'
  legs
  exit 0
fi

if [ "$owner" != 1 ]; then
  echo "linux-live-gate: these legs need the owner's live session; pass --owner" >&2
  exit 2
fi

echo "linux-live-gate: the leg bodies are not implemented in this stub" >&2
legs >&2
exit 3
