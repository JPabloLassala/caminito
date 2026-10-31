#!/usr/bin/env bash
# CMN-231's Nested check of Esmeril: Caminito on winit under Xvfb, on llvmpipe, on a session bus
# of its own, with the test theme EsmerilSeeThrough. A wide red window lies under a blue one
# whose titlebar is across the red one's top edge. Each shot is of the whole screen:
#
#   01-default     esmeril on at strength 15: the edge beneath the titlebar is soft
#   02-moved       the red window moved left beneath it: what shows follows it
#   03-strength-1  strength 1: the edge is far sharper, at once
#   04-off         esmeril off: the edge is sharp
#   05-on-again    esmeril on again, still at strength 1
#   06-reduced     reduced-resources on: sharp again
#
# and writes report.txt: a row of pixels along the titlebar's see-through rows for each shot,
# and Caminito's warnings. The window beneath moves itself, as a drag would raise it. It needs a build in $CARGO_TARGET_DIR, Xvfb, xdotool, xterm,
# ImageMagick, dconf and glib-compile-schemas.
#
#   run.sh [<output directory>]
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
out=$(realpath -m "${1:-$here/out}")
target=${CARGO_TARGET_DIR:-$root/target}
work=$(mktemp -d "$HOME/caminito-cmn-231.XXXXXX")
mkdir -p "$out" "$work/schemas" "$work/data/themes" "$work/config" "$work/run"
chmod 700 "$work/run"
rm -f "${out:?}"/*.png "${out:?}/report.txt"
unset WAYLAND_DISPLAY

cp /usr/share/glib-2.0/schemas/*.xml "$work/schemas/"
cp "$root/data/org.mate.Caminito.gschema.xml" "$work/schemas/"
glib-compile-schemas "$work/schemas"
ln -s "$root/tests/themes/EsmerilSeeThrough" "$work/data/themes/"

for n in $(seq 90 120); do [ -e "/tmp/.X11-unix/X$n" ] || break; done
Xvfb ":$n" -screen 0 1000x700x24 >/dev/null 2>&1 &
xserver=$!
trap 'kill $xserver 2>/dev/null; sleep 1; rm -rf "${work:?}" 2>/dev/null || true' EXIT
for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done

export OUTER=":$n" OUT=$out TARGET=$target WORK=$work
DISPLAY=$OUTER XDG_DATA_HOME=$work/data XDG_CONFIG_HOME=$work/config \
  XDG_RUNTIME_DIR=$work/run GSETTINGS_SCHEMA_DIR=$work/schemas GSETTINGS_BACKEND=dconf \
  LIBGL_ALWAYS_SOFTWARE=1 dbus-run-session -- bash "$here/session.sh"
