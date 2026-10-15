#!/bin/bash
# The Night Light tab in --demo (142), on Xvfb with GDK's X11 backend: a picture of each tab.
#   BIN=$HOME/target-cmn-115/debug/caminito-displays .scratch/checks/CMN-115/demo.sh [<out dir>]
here=$(dirname "$(realpath "$0")")
out=$(realpath -m "${1:-$here/out}")
mkdir -p "$out"
work=$(mktemp -d "$HOME/cmn-115-demo.XXXXXX")
Xvfb :215 -screen 0 1000x800x24 >"$work/xvfb.log" 2>&1 &
xvfb=$!
sleep 1
export DISPLAY=:215 GDK_BACKEND=x11 GTK_A11Y=none NO_AT_BRIDGE=1 XDG_RUNTIME_DIR=$work
dbus-run-session -- bash -c "
  '$BIN' --demo --night-light 2>'$work/displays.log' &
  sleep 3; import -window root '$out/demo-night-light.png'
  kill %1
  '$BIN' --demo 2>>'$work/displays.log' &
  sleep 3; import -window root '$out/demo-displays.png'
  kill %1"
kill $xvfb
cat "$work/displays.log"
rm -rf "$work"
