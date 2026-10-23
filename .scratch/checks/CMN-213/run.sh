#!/usr/bin/env bash
# CMN-213's check of the Sound applet's icon: mate-panel on X11 with the applet alone on it,
# against a private PipeWire, WirePlumber and pipewire-pulse in a throwaway XDG_RUNTIME_DIR,
# with null sinks and sources for sound devices (session.sh). Each shot is of the panel's
# corner, at twice its size, with the tooltip under the icon:
#
#   01-no-sound-server     the speaker struck through, "No sound server"
#   02-popup-no-server     the popup: "No sound server"
#   03-speaker-100 … 05-speaker-20  the speaker's three steps, "Speakers: 100%" …
#   06-speaker-muted       the muted speaker, "Speakers: 45% (muted)"
#   07-speaker-0           the muted speaker, "Speakers: 0%"
#   08-headphones          headphones, "WH-1000XM4: 45%"
#   09-headphones-muted    headphones crossed out, "WH-1000XM4: 45% (muted)"
#   10-monitor-output      the speaker for an HDMI output, "HDMI: 70%"
#   11-recording           the red dot, and "Recording: OBS Studio" on the second line
#   12-popup               the popup, empty until the popup's tickets fill it
#   13-menu                the right-click menu: null devices have no cards to list
#   14-server-stopped      the sound server stopped: struck through again
#   15-server-back         and back: the speaker
#   16-menu-real-cards     with --real-cards, the menu against the user's own sound server,
#                          with a section of profiles for each card. Only opened.
#
# and writes report.txt: what the wheel and middle-click set on the server, with the GNOME
# key off and on, whether the panel lived through each state, and its log's warnings.
#
#   run.sh [--real-cards] [<meson build directory with the applet built>] [<output directory>]
#
# Without a build directory it builds the top-level project in a scratch one. It needs
# mate-panel, PipeWire, WirePlumber, pipewire-pulse, pactl, pw-record, python-xlib,
# ImageMagick and dconf, and Xvfb or, without it, a Wayland session for a rootful Xwayland.
# CAMINITO_DISPLAY names an X display to use instead.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-213.XXXXXX)
trap 'rm -rf "${work:?}"' EXIT
real_runtime_dir=
if [ "${1:-}" = --real-cards ]; then
  real_runtime_dir=$XDG_RUNTIME_DIR
  shift
fi
build=${1:-}
out=$(realpath -m "${2:-$here/out}")
if [ -z "$build" ]; then
  build=$work/build
  meson setup "$build" "$root" >/dev/null
  ninja -C "$build" >/dev/null
fi
module=$(find "$(realpath "$build")" -name libcaminito-sound-applet.so | head -1)
[ -n "$module" ] || { echo "no libcaminito-sound-applet.so in $build" >&2; exit 1; }
mkdir -p "$work/applets" "$out"
rm -f "${out:?}"/*.png "${out:?}/report.txt"
# The stock applets too: the variable replaces the panel's own directory.
cp /usr/share/mate-panel/applets/* "$work/applets/"
sed "s#@APPLET_LOCATION@#$module#" \
  "$root/helpers/caminito-sound-applet/org.mate.panel.CaminitoSoundApplet.mate-panel-applet.in" \
  >"$work/applets/org.mate.panel.CaminitoSoundApplet.mate-panel-applet"

# A display of its own: Xvfb, or a rootful Xwayland's window in the Wayland session.
xserver=
if [ -z "${CAMINITO_DISPLAY:-}" ]; then
  for n in $(seq 90 99); do [ -e /tmp/.X11-unix/X$n ] || break; done
  if command -v Xvfb >/dev/null; then
    Xvfb ":$n" -screen 0 480x320x24 >/dev/null 2>&1 &
  else
    Xwayland ":$n" -geometry 480x320 >/dev/null 2>&1 &
  fi
  xserver=$!
  export DISPLAY=:$n
  for _ in $(seq 50); do [ -e "/tmp/.X11-unix/X$n" ] && break; sleep 0.1; done
else
  export DISPLAY=$CAMINITO_DISPLAY
fi

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
# X11 even when run from a Wayland session, which GTK would take first. A runtime directory,
# settings and state of its own, so that nothing of the user's sound server is touched.
XDG_CONFIG_HOME=$work/config XDG_RUNTIME_DIR=$work/run XDG_STATE_HOME=$work/state \
  MATE_PANEL_APPLETS_DIR=$work/applets NO_AT_BRIDGE=1 GDK_BACKEND=x11 WAYLAND_DISPLAY= \
  timeout 240 dbus-run-session -- bash "$here/session.sh" "$here" "$work" "$out" "$real_runtime_dir" \
  2>&1 | grep -vE '^(dbus-daemon\[|fusermount)' >>"$out/report.txt" || true
[ -n "$xserver" ] && kill "$xserver"
cat "$out/report.txt"
echo "screenshots in $out"
