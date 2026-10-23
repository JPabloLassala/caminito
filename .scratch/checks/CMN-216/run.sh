#!/usr/bin/env bash
# CMN-216's check of the Sound popup's Applications: mate-panel on X11 with the applet alone
# on a top panel, against a private PipeWire, WirePlumber and pipewire-pulse in a throwaway
# XDG_RUNTIME_DIR, with null sinks and sources for sound devices and pw-cat streams for Sound
# applications (session.sh). The popup is opened by a click on the applet and driven by the
# keyboard alone. Each shot is of the screen's left part, the popup under the panel:
#
#   01-applications        System sounds first, then mpv's two copies in one row at their
#                          loudest stream's 80%, and Firefox; Audacious's paused stream isn't
#                          listed. Each playing application has its green activity bar
#   02-expanded            mpv's row opened from the keyboard: "Plays to" with "Follow the
#                          default (Speakers)" checked and the devices, and a slider for each
#                          of its two streams
#   03-moved               mpv moved to WH-1000XM4 from the keyboard: checked in its list, and
#                          its device as a badge in the theme's selection colour
#   04-recording           OBS Studio under the Recording heading, "Records from" open
#
# and writes report.txt: the server's streams before and after each step (mpv's two at 80%
# and 40%, then both moved by the row's slider in proportion; moved to the headset; back to
# following the default, and following its change to HDMI), System sounds' slider and the
# volume a canberra-gtk-play event sound plays at, whether the panel lived through each
# state, and its log's warnings.
#
#   run.sh [<meson build directory with the applet built>] [<output directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs
# mate-panel, PipeWire, WirePlumber, pipewire-pulse, pactl, pw-cat, canberra-gtk-play,
# python-xlib, ImageMagick, dconf and a C compiler, and Xvfb or, without it, a Wayland session
# for rootful Xwaylands.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-216.XXXXXX)
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
mkdir -p "$work/applets" "$out"
rm -f "${out:?}"/*.png "${out:?}/report.txt"
# The stock applets too: the variable replaces the panel's own directory.
cp /usr/share/mate-panel/applets/* "$work/applets/"
sed "s#@APPLET_LOCATION@#$module#" \
  "$root/helpers/caminito-sound-applet/org.mate.panel.CaminitoSoundApplet.mate-panel-applet.in" \
  >"$work/applets/org.mate.panel.CaminitoSoundApplet.mate-panel-applet"
# shellcheck disable=SC2046
cc -o "$work/paused" "$here/paused.c" $(pkg-config --cflags --libs libpulse)

mkdir -p "$work/config/gtk-3.0" "$work/config/wireplumber/wireplumber.conf.d" "$work/run" \
  "$work/state"
chmod 700 "$work/run"
# No XSettings manager runs on the check's display, so this is GTK's: event sounds are on,
# which canberra-gtk-play asks.
printf '[Settings]\ngtk-theme-name=%s\ngtk-icon-theme-name=%s\ngtk-enable-event-sounds=1\n' \
  "${CAMINITO_GTK_THEME:-Adwaita}" "${CAMINITO_ICON_THEME:-mate}" \
  >"$work/config/gtk-3.0/settings.ini"
# WirePlumber on the null devices alone: none of the computer's own.
cat >"$work/config/wireplumber/wireplumber.conf.d/50-null-devices.conf" <<'CONF'
wireplumber.profiles = { main = { monitor.alsa = disabled, monitor.alsa-midi = disabled,
  monitor.bluez = disabled, monitor.bluez-midi = disabled, monitor.v4l2 = disabled,
  monitor.libcamera = disabled } }
CONF

# A display of its own, Xvfb or a rootful Xwayland's window in the Wayland session, and
# session.sh on it, on a bus of its own. X11 even when run from a Wayland session, which GTK
# would take first. A runtime directory, settings and state of its own, so that nothing of the
# user's sound server is touched.
for n in $(seq 90 99); do [ -e /tmp/.X11-unix/X$n ] || break; done
if command -v Xvfb >/dev/null; then
  Xvfb ":$n" -screen 0 480x900x24 >/dev/null 2>&1 &
else
  Xwayland ":$n" -geometry 480x900 >/dev/null 2>&1 &
fi
xserver=$!
for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done
DISPLAY=:$n XDG_CONFIG_HOME=$work/config XDG_RUNTIME_DIR=$work/run XDG_STATE_HOME=$work/state \
  MATE_PANEL_APPLETS_DIR=$work/applets NO_AT_BRIDGE=1 GDK_BACKEND=x11 WAYLAND_DISPLAY= \
  timeout 300 dbus-run-session -- bash "$here/session.sh" "$here" "$work" "$out" \
  2>&1 | grep -vE '^(dbus-daemon\[|fusermount)' >>"$out/report.txt" || true
kill "$xserver" 2>/dev/null; wait "$xserver" 2>/dev/null || true
cat "$out/report.txt"
echo "screenshots in $out"
