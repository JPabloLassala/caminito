#!/usr/bin/env bash
# CMN-217's check of the Sound popup's Pinned applications: mate-panel on X11 with the applet
# alone on a top panel, against a private PipeWire, WirePlumber (its linking logged) and
# pipewire-pulse in a throwaway XDG_RUNTIME_DIR, with null sinks for sound devices and pw-cat
# streams for Sound applications (session.sh). The popup is opened by a click on the applet
# and driven by the keyboard alone. Each shot is of the screen's left part, the popup under
# the panel:
#
#   01-pinned          Pinned applications above Applications: mpv playing on WH-1000XM4,
#                      pinned from its row's pin button; Rhythmbox "‖ Paused" there; Spotify
#                      "■ Not running", pinned to DELL U2720Q, with no slider. Firefox under
#                      Applications, with its pin button
#   02-device-gone     WH-1000XM4 turned off: mpv's and Rhythmbox's badges dimmed,
#                      "WH-1000XM4 · not connected"
#   03-row-strip       mpv's × opened its strip: "Unpin mpv? It goes back to following the
#                      default output.", Cancel, Unpin
#   04-heading-strip   The heading's ×, focused, with "Unpin all" beside it, opened its strip:
#                      "Unpin all 3 applications? …", Cancel, "Unpin all"
#   05-undo            mpv unpinned: "mpv unpinned." with Undo in its row's place
#   06-undo-all        Unpin all: "3 applications unpinned." with Undo in the list's place
#
# and writes report.txt: the pins as dconf has them at each step, the server's streams
# (where each plays, its volumes and mute) before and after each, how long a pinned
# application's new stream played on the default before the applet moved it (from
# WirePlumber's links, as CMN-209's blip.sh read them), a pin after mate-panel restarts, a move
# without a pin after the application restarts, and, from a new session after this one
# ended (login.sh), the pins, WirePlumber's target restore, and the pinned Spotify playing.
#
#   run.sh [<meson build directory with the applet built>] [<output directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs
# mate-panel, PipeWire, WirePlumber, pipewire-pulse, pactl, pw-cat, wpctl, python-xlib,
# ImageMagick, dconf, glib-compile-schemas and a C compiler, and Xvfb or, without it, a
# Wayland session for rootful Xwaylands.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-217.XXXXXX)
trap 'rm -rf "${work:?}"' EXIT
build=${1:-}
out=$(realpath -m "${2:-$here/out}")
if [ -z "$build" ]; then
  build=$work/build
  meson setup "$build" "$root" >/dev/null
  ninja -C "$build" >/dev/null
fi
build=$(realpath "$build")
module=$(find "$build" -name libcaminito-sound-applet.so | head -1)
[ -n "$module" ] || { echo "no libcaminito-sound-applet.so in $build" >&2; exit 1; }
mkdir -p "$work/applets" "$work/schemas" "$out"
rm -f "${out:?}"/*.png "${out:?}/report.txt"
# The stock applets too: the variable replaces the panel's own directory.
cp /usr/share/mate-panel/applets/* "$work/applets/"
sed "s#@APPLET_LOCATION@#$module#" \
  "$root/helpers/caminito-sound-applet/org.mate.panel.CaminitoSoundApplet.mate-panel-applet.in" \
  >"$work/applets/org.mate.panel.CaminitoSoundApplet.mate-panel-applet"
# The applet's schema, which the system's directory hasn't, beside it.
cp "$root/helpers/caminito-sound-applet/org.mate.panel.applet.caminito-sound.gschema.xml" \
  "$work/schemas/"
glib-compile-schemas "$work/schemas"
# shellcheck disable=SC2046
cc -o "$work/paused" "$here/paused.c" $(pkg-config --cflags --libs libpulse)

mkdir -p "$work/config/gtk-3.0" "$work/config/wireplumber/wireplumber.conf.d" "$work/run" \
  "$work/state"
chmod 700 "$work/run"
printf '[Settings]\ngtk-theme-name=%s\ngtk-icon-theme-name=%s\n' \
  "${CAMINITO_GTK_THEME:-Adwaita}" "${CAMINITO_ICON_THEME:-mate}" \
  >"$work/config/gtk-3.0/settings.ini"
# WirePlumber on the null devices alone: none of the computer's own.
cat >"$work/config/wireplumber/wireplumber.conf.d/50-null-devices.conf" <<'CONF'
wireplumber.profiles = { main = { monitor.alsa = disabled, monitor.alsa-midi = disabled,
  monitor.bluez = disabled, monitor.bluez-midi = disabled, monitor.v4l2 = disabled,
  monitor.libcamera = disabled } }
CONF

for n in $(seq 90 99); do [ -e /tmp/.X11-unix/X$n ] || break; done
if command -v Xvfb >/dev/null; then
  Xvfb ":$n" -screen 0 480x900x24 >/dev/null 2>&1 &
else
  Xwayland ":$n" -geometry 480x900 >/dev/null 2>&1 &
fi
xserver=$!
for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done
export XDG_CONFIG_HOME=$work/config XDG_RUNTIME_DIR=$work/run XDG_STATE_HOME=$work/state \
  GSETTINGS_SCHEMA_DIR=$work/schemas
DISPLAY=:$n MATE_PANEL_APPLETS_DIR=$work/applets NO_AT_BRIDGE=1 GDK_BACKEND=x11 \
  WAYLAND_DISPLAY= timeout 400 dbus-run-session -- bash "$here/session.sh" "$here" "$work" "$out" \
  2>&1 | grep -vE '^(dbus-daemon\[|fusermount)|RTKit' >>"$out/report.txt" || true
# A login after this one: a bus of its own, the same settings and state, the panel and the
# sound server started again.
DISPLAY=:$n MATE_PANEL_APPLETS_DIR=$work/applets NO_AT_BRIDGE=1 GDK_BACKEND=x11 \
  WAYLAND_DISPLAY= timeout 60 dbus-run-session -- bash "$here/login.sh" "$here" "$work" \
  2>&1 | grep -vE '^(dbus-daemon\[|fusermount)|RTKit' >>"$out/report.txt" || true
kill "$xserver" 2>/dev/null; wait "$xserver" 2>/dev/null || true
cat "$out/report.txt"
echo "screenshots in $out"
