#!/bin/bash
# System Info's Windowing System row in each reading (181). Builds row-probe.c, a GTK module
# that prints the label after "Windowing System" and quits, and runs the mate-system-info
# given as $1 under a headless Weston and Xvfb, on a private bus, with GSettings in memory.
#
#   windowing-row.sh path/to/mate-system-info
#
# Needs weston, Xvfb, dbus-run-session and gtk3's headers. Each line says ok or FAIL, and
# the script exits 1 after a FAIL.
set -u
bin=$(realpath "$1")
here=$(dirname "$(realpath "$0")")
work=$(mktemp -d)
gcc -shared -fPIC -o "$work/librow-probe.so" "$here/row-probe.c" $(pkg-config --cflags --libs gtk+-3.0) || exit 1

export XDG_RUNTIME_DIR=$work/run
mkdir -m 700 "$XDG_RUNTIME_DIR"
Xvfb :99 >/dev/null 2>&1 & xpid=$!
weston --backend=headless --socket=wl-row --idle-time=0 >/dev/null 2>&1 & wpid=$!
trap 'kill $xpid $wpid; rm -rf "$work"' EXIT
sleep 2

failed=0
check () {
  local name=$1 want=$2 got; shift 2
  got=$(env -u MATE_WM_NAME -u GDK_BACKEND DISPLAY=:99 WAYLAND_DISPLAY=wl-row \
    GTK_MODULES="$work/librow-probe.so" GSETTINGS_BACKEND=memory "$@" \
    dbus-run-session -- timeout 20 "$bin" 2>/dev/null | sed -n 's/^row=//p')
  if [ "$got" = "$want" ]; then echo "ok   $name: $got"
  else echo "FAIL $name: '$got', not '$want'"; failed=1; fi
}

check "Caminito, Wayland"           "Caminito (Wayland)" MATE_WM_NAME=Caminito
check "Caminito, GDK_BACKEND=x11"   "Caminito (X11)"     MATE_WM_NAME=Caminito GDK_BACKEND=x11
check "MATE_WM_NAME unset, Wayland" "Wayland"
check "MATE_WM_NAME unset, x11"     "X11"                GDK_BACKEND=x11
check "Metacity, Wayland"           "Wayland"            MATE_WM_NAME=Metacity
exit $failed
