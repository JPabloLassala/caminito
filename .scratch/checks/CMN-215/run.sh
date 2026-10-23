#!/usr/bin/env bash
# CMN-215's check of the Sound popup's output and input rows: mate-panel on X11 with the
# applet alone on a top panel, against a private PipeWire, WirePlumber and pipewire-pulse in a
# throwaway XDG_RUNTIME_DIR, with null sinks and sources for sound devices (session.sh). The
# popup is opened by a click on the applet and driven by the keyboard alone, Tab and Space,
# which is the ticket's keyboard check too. Each shot is of the screen's left part, the
# popup under the panel:
#
#   01-no-sound-server     "No sound server" and the footer
#   02-speaker             the speaker at 60% on the round button, "Speakers" on the pill
#   03-speaker-muted       the speaker crossed out
#   04-headphones          headphones, "WH-1000XM4"
#   05-headphones-muted    headphones struck out by the popup
#   06-monitor             a monitor, "DELL U2720Q"
#   07-monitor-muted       the monitor struck out
#   08-outputs-open        the list of outputs, opened from the keyboard: each with its icon
#                          and "Virtual device" (null sinks have no card), the check on the
#                          default; Easy Effects Sink a virtual device among them
#   09-output-chosen       WH-1000XM4 chosen from the keyboard: checked, and on the pill
#   10-inputs-open         the list of inputs, the output's monitors left out
#   11-meter-a, 11-meter-b the input's meter moving with a tone the microphone hears, a moment
#                          apart
#   12-short-screen        on a 480x320 screen, the outputs' list open: the popup ends at the
#                          screen's bottom with the footer shown, and scrolls
#   13-short-scrolled      the same after the plain wheel over the slider: scrolled, the
#                          volume the same
#   14-port-dropdown       a port dropdown, open, which null devices can't have: the popup
#                          drawn from a status built by hand (shoot-ports.c), as the tests'
#
# and writes report.txt: the default the keyboard chose on the server, the volume after the
# plain wheel, Shift+wheel and the arrows over the slider, whether the panel lived through
# each state, and its log's warnings.
#
#   run.sh [<meson build directory with the applet built>] [<output directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs
# mate-panel, PipeWire, WirePlumber, pipewire-pulse, pactl, pw-play, python-xlib, ImageMagick,
# dconf and a C compiler, and Xvfb or, without it, a Wayland session for rootful Xwaylands.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-215.XXXXXX)
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

# The port dropdown, from the build's own libraries.
lib() { find "$build" -name "$1" | head -1; }
# shellcheck disable=SC2046
cc -o "$work/shoot-ports" "$here/shoot-ports.c" \
  -I"$root/helpers/caminito-sound-applet" -I"$root/helpers/caminito-applet-common" \
  "$(lib libsd-popup.a)" "$(lib libsd-state.a)" "$(lib libcaminito-applet-common.a)" \
  $(pkg-config --cflags --libs libmatepanelapplet-4.0 libpulse)

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

# session <mode> <width>x<height>: a display of its own, Xvfb or a rootful Xwayland's window
# in the Wayland session, and session.sh on it, on a bus of its own. X11 even when run from a
# Wayland session, which GTK would take first. A runtime directory, settings and state of its
# own, so that nothing of the user's sound server is touched.
session() {
  local n xserver
  for n in $(seq 90 99); do [ -e /tmp/.X11-unix/X$n ] || break; done
  if command -v Xvfb >/dev/null; then
    Xvfb ":$n" -screen 0 "$2x24" >/dev/null 2>&1 &
  else
    Xwayland ":$n" -geometry "$2" >/dev/null 2>&1 &
  fi
  xserver=$!
  for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done
  rm -rf "${work:?}/run/"* "${work:?}/state/"*
  DISPLAY=:$n XDG_CONFIG_HOME=$work/config XDG_RUNTIME_DIR=$work/run XDG_STATE_HOME=$work/state \
    MATE_PANEL_APPLETS_DIR=$work/applets NO_AT_BRIDGE=1 GDK_BACKEND=x11 WAYLAND_DISPLAY= \
    timeout 300 dbus-run-session -- bash "$here/session.sh" "$1" "$here" "$work" "$out" \
    2>&1 | grep -vE '^(dbus-daemon\[|fusermount)' >>"$out/report.txt" || true
  kill "$xserver" 2>/dev/null; wait "$xserver" 2>/dev/null || true
}
session full 480x720
session short 480x320
cat "$out/report.txt"
echo "screenshots in $out"
