#!/usr/bin/env bash
# CMN-285's check of the bubbles' stack: caminito-notifications on X11, where the bubbles are
# a plain window and their room is the output's height, on a session bus of its own and with
# its settings in memory. Each shot is of the whole screen:
#
#   01-full-stack   six notifications that stay: as many bubbles as fit, newest on top, under
#                   the header with "N more notifications" and the close button
#   02-critical     a critical one takes the oldest ordinary bubble's place, and the count
#                   grows by one
#   03-all-critical with only critical bubbles left, another critical one gets none, and the
#                   count is marked as holding a critical notification
#   04-clear-all    the pointer on the header's close button, which shows "Clear all"
#
# and writes report.txt: the history's length, and the daemon's warnings.
#
#   run.sh [<output directory>]
#
# It needs cargo, Xvfb, ImageMagick, python-xlib and glib-compile-schemas.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
out=$(realpath -m "${1:-$here/out}")
work=$(mktemp -d -t caminito-cmn-285.XXXXXX)
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
# Never expires, so that the stack stays for the shot.
notify() {
  gdbus call --session --dest org.freedesktop.Notifications \
    --object-path /org/freedesktop/Notifications --method org.freedesktop.Notifications.Notify \
    "$1" 0 "$2" "$3" "$4" '[]' "{'urgency': <byte $5>}" 0 >/dev/null
}
shot() { sleep 1; import -window root "$out/$1.png"; }
notify Mail mail-unread "Invoice 2026-10" "The invoice for October is attached." 1
notify Chat user-available "Ana" "Lunch at one?" 1
notify Calendar x-office-calendar "Standup in 5 min" "Room 3, or the usual call." 1
notify Builds emblem-default "Build passed" "master at 7865548f, in 4 min 12 s." 1
notify Chat user-available "Ana" "Or two, if you're busy." 1
notify Mail mail-unread "Re: Invoice 2026-10" "Thanks, paid." 1
shot 01-full-stack
notify Battery battery-caution "Battery low" "10% left: plug in the charger." 2
shot 02-critical
for i in 1 2 3 4 5; do notify Battery battery-caution "Battery low ($i)" "Still unplugged." 2; done
shot 03-all-critical
# The close button is at the header's right, at the window's top left.
python3 -c 'from Xlib import display, X; from Xlib.ext import xtest
d = display.Display(); xtest.fake_input(d, X.MotionNotify, x=340, y=12); d.sync()'
shot 04-clear-all
listed=$(gdbus call --session --dest org.mate.Caminito.Notifications \
  --object-path /org/mate/Caminito/Notifications --method org.mate.Caminito.Notifications.Counts)
echo "counts in the history: $listed"
kill "$daemon"
SESSION

DISPLAY=:$n GDK_BACKEND=x11 WAYLAND_DISPLAY= NO_AT_BRIDGE=1 \
  GSETTINGS_SCHEMA_DIR=$work/schemas GSETTINGS_BACKEND=memory XDG_STATE_HOME=$work/state \
  timeout 120 dbus-run-session -- bash "$work/session.sh" "$binary" "$out" 2>&1 \
  | grep -vE "^(dbus-daemon\[|fusermount|\(/usr/lib/xdg-desktop-portal)" >"$out/report.txt" || true
{ echo "daemon warnings:"; grep -i -E "warn|crit" "$out/daemon.log" || echo "none"; } >>"$out/report.txt"
rm -f "$out/daemon.log"
cat "$out/report.txt"
