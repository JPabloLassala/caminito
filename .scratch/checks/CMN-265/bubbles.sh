#!/usr/bin/env bash
# CMN-265's check of a grouped bubble: the real caminito-notifications on X11, where there's no
# layer shell, so its bubbles are a plain window in the screen's corner, on a session bus of
# its own, with its schemas compiled from data/ over GSettings' memory backend and its
# history in a scratch directory. With no window manager, the window is in the top left-hand
# corner, which the shots are of. bubbles-session.sh sends Signal's notifications as an app
# would, with no timeout, so that each shot waits for nothing:
#
#   01-two-bubbles      two notifications: a bubble each
#   02-merged           the third merges them into one grouped bubble: the newest in full with
#                       its actions and the count, 3, in its header, the two before it on a line
#                       each, and "Show all 3"
#   03-grouped          two more join it: the count is 5, the two before the newest have a
#                       line each, and "+2 more"
#   04-opened           a click on "+2 more": all four before the newest inside the bubble,
#                       and "Show less"
#   05-another-app      another app's notification has a bubble of its own beside the group
#
# and writes report.txt: whether the daemon lived through each, and its warnings.
#
#   bubbles.sh [<caminito-notifications binary>] [<output directory>]
#
# Without a binary it builds target/debug/caminito-notifications. It needs Xvfb, python-xlib
# and ImageMagick. CAMINITO_DISPLAY names an X display to use instead of Xvfb.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-265.XXXXXX)
trap 'rm -rf "${work:?}"' EXIT
binary=${1:-}
out=$(realpath -m "${2:-$here/out}")
if [ -z "$binary" ]; then
  cargo build --manifest-path "$root/Cargo.toml" -p caminito-notifications >/dev/null 2>&1
  binary=$root/target/debug/caminito-notifications
fi
binary=$(realpath "$binary")
mkdir -p "$work/schemas" "$out"
rm -f "$out"/bubble-*.png "$out/bubbles-report.txt"
cp "$root/data/org.mate.Caminito.gschema.xml" "$work/schemas/"
glib-compile-schemas "$work/schemas"

xserver=
if [ -z "${CAMINITO_DISPLAY:-}" ]; then
  for n in $(seq 90 99); do [ -e /tmp/.X11-unix/X$n ] || break; done
  Xvfb ":$n" -screen 0 800x900x24 >/dev/null 2>&1 &
  xserver=$!
  export DISPLAY=:$n
  for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done
else
  export DISPLAY=$CAMINITO_DISPLAY
fi

mkdir -p "$work/config/gtk-4.0"
printf '[Settings]\ngtk-theme-name=%s\n' "${CAMINITO_GTK_THEME:-Adwaita}" \
  >"$work/config/gtk-4.0/settings.ini"
GSETTINGS_SCHEMA_DIR=$work/schemas GSETTINGS_BACKEND=memory XDG_STATE_HOME=$work/state \
  XDG_CONFIG_HOME=$work/config GDK_BACKEND=x11 WAYLAND_DISPLAY= GTK_A11Y=none \
  timeout 120 dbus-run-session -- bash "$here/bubbles-session.sh" "$root" "$work" "$out" \
  "$binary" 2>&1 | grep -vE '^(dbus-daemon\[|fusermount)' >>"$out/bubbles-report.txt" || true
[ -n "$xserver" ] && kill "$xserver"
cat "$out/bubbles-report.txt"
echo "screenshots in $out"
