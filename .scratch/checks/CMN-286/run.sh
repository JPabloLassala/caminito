#!/usr/bin/env bash
# CMN-286's check of the line of a bubble's time left: caminito-notifications on X11, where
# the bubbles are a plain window at the screen's top left, on a session bus of its own and with
# its settings in memory. Each shot is of the whole screen:
#
#   01-part-way  a bubble with a 12 s timeout, 5 s after it opened: its line along its bottom
#                edge, in the theme's accent, about half its width; under it a critical bubble,
#                which has no line
#   02-hovered   the pointer on that bubble for 4 s: its line as long as it was when the
#                pointer came, and the close button showing
#   03-left      the pointer gone again for 3 s: the line shorter again, from where it stopped
#
# and writes report.txt with the daemon's warnings.
#
#   run.sh [<output directory>]
#
# It needs cargo, Xvfb, ImageMagick, python-xlib and glib-compile-schemas.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
out=$(realpath -m "${1:-$here/out}")
work=$(mktemp -d -t caminito-cmn-286.XXXXXX)
trap 'rm -rf "${work:?}"' EXIT
mkdir -p "$out" "$work/schemas"
rm -f "${out:?}"/*.png "${out:?}/report.txt"

cargo build --quiet --manifest-path "$root/Cargo.toml" -p caminito-notifications
binary=$root/target/debug/caminito-notifications
cp "$root/data/org.mate.Caminito.gschema.xml" "$work/schemas/"
glib-compile-schemas "$work/schemas"

for n in $(seq 90 99); do [ -e /tmp/.X11-unix/X$n ] || break; done
Xvfb ":$n" -screen 0 480x560x24 >/dev/null 2>&1 &
xserver=$!
trap 'kill $xserver; rm -rf "${work:?}"' EXIT
for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done

cat >"$work/session.sh" <<'SESSION'
set -euo pipefail
binary=$1 out=$2
"$binary" 2>"$out/daemon.log" &
daemon=$!
for _ in $(seq 100); do
  gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
    --method org.freedesktop.DBus.NameHasOwner org.freedesktop.Notifications | grep -q true && break
  sleep 0.1
done
notify() {
  gdbus call --session --dest org.freedesktop.Notifications \
    --object-path /org/freedesktop/Notifications --method org.freedesktop.Notifications.Notify \
    "$1" 0 "$2" "$3" "$4" '[]' "{'urgency': <byte $5>}" "$6" >/dev/null
}
pointer() {
  python3 -c "from Xlib import display, X; from Xlib.ext import xtest
d = display.Display(); xtest.fake_input(d, X.MotionNotify, x=$1, y=$2); d.sync()"
}
pointer 470 550
notify Battery battery-caution "Battery low" "10% left: plug in the charger." 2 12000
notify Mail mail-unread "Invoice 2026-10" "The invoice for October is attached." 1 12000
sleep 5
import -window root "$out/01-part-way.png"
# The newest bubble is under the header, at the window's top.
pointer 180 90
sleep 4
import -window root "$out/02-hovered.png"
pointer 470 550
sleep 3
import -window root "$out/03-left.png"
kill "$daemon"
SESSION

DISPLAY=:$n GDK_BACKEND=x11 WAYLAND_DISPLAY= NO_AT_BRIDGE=1 \
  GSETTINGS_SCHEMA_DIR=$work/schemas GSETTINGS_BACKEND=memory XDG_STATE_HOME=$work/state \
  timeout 120 dbus-run-session -- bash "$work/session.sh" "$binary" "$out" 2>&1 \
  | grep -vE "^(dbus-daemon\[|fusermount|\(/usr/lib/xdg-desktop-portal)" >"$out/report.txt" || true
{ echo "daemon warnings:"; grep -i -E "warn|crit" "$out/daemon.log" || echo "none"; } >>"$out/report.txt"
rm -f "$out/daemon.log"
cat "$out/report.txt"
