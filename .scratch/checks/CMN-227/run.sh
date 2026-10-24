#!/usr/bin/env bash
# CMN-227's check of the Dock item's badge and progress bar: render.c draws the item's own draw
# handler for each state on each of the four edges, under Xvfb, into PNGs in the output
# directory (one per state and edge, named badge-5-bottom.png and so on), and sheet.png has
# them all in a grid, one row for each state and a column for each edge: bottom, top, left,
# right. First the pure modules' tests: the Launcher entry state and the feedback's second source
# of attention, built here without libxfce4windowing, which only the full project needs.
#
#   run.sh [<output directory>]
#
# It needs GTK 3, Meson, a C compiler and Xvfb.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
applet=$root/helpers/caminito-dock-applet
out=$(realpath -m "${1:-$here/out}")
work=$(mktemp -d -t caminito-cmn-227.XXXXXX)
trap 'rm -rf "${work:?}"' EXIT
mkdir -p "$out" "$work/stub/libxfce4windowing"
rm -f "${out:?}"/*.png

echo "== the Launcher entry state and the feedback, in unit tests"
cat >"$work/meson.build" <<MESON
project('cmn-227', 'c', default_options: ['warning_level=2', 'c_std=gnu11'])
glib = dependency('glib-2.0')
gio_unix = dependency('gio-unix-2.0')
m = meson.get_compiler('c').find_library('m')
src = '$applet/'
resolve = static_library('r', src + 'dk-resolve.c', dependencies: [glib, gio_unix])
model = static_library('m', src + 'dk-model.c', link_with: resolve, dependencies: glib)
feedback = static_library('f', src + 'dk-feedback.c', link_with: model, dependencies: glib)
launcher = static_library('l', src + 'dk-launcher.c', link_with: [model, resolve],
                          dependencies: [glib, m])
test('launcher', executable('tl', src + 'test-launcher.c', link_with: [launcher, model, resolve],
                            dependencies: [glib, m]))
test('feedback', executable('tf', src + 'test-feedback.c', link_with: [feedback, model, resolve],
                            dependencies: glib))
MESON
meson setup "$work/build" "$work" >/dev/null
meson test -C "$work/build" 2>&1 | grep -E '^ *[0-9]+/[0-9]+|^(Ok|Fail):'

echo "== the item's drawing, on each edge"
# The item makes one call into libxfce4windowing, which render.c stands in for.
printf '#include <gtk/gtk.h>\nvoid xfw_window_set_button_geometry (gpointer, GdkWindow *, const GdkRectangle *, gpointer);\n' \
  >"$work/stub/libxfce4windowing/libxfce4windowing.h"
# shellcheck disable=SC2046
gcc -std=gnu11 -Wall -Wextra -I"$work/stub" -I"$applet" $(pkg-config --cflags gtk+-3.0) \
  -o "$work/render" "$here/render.c" "$applet/dk-item.c" "$applet/dk-launcher.c" \
  "$applet/dk-model.c" "$applet/dk-resolve.c" "$applet/dk-feedback.c" \
  $(pkg-config --libs gtk+-3.0 gio-unix-2.0) -lm
xvfb-run -a "$work/render" "$out"
ls "$out"
