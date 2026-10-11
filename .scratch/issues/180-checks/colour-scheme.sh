#!/bin/bash
# The colour scheme msd derives from the GTK theme (180), under a headless labwc with
# MATE_WM_NAME=Caminito: org.gnome.desktop.interface color-scheme, and what the portal's
# ReadOne org.freedesktop.appearance color-scheme gives, for BlackMATE and Menta, at start-up
# and live, and after the key is set to prefer-light by hand. With "stock" as the first
# argument, without the variable: the key keeps a value set before msd starts. Each line
# says ok or FAIL, and the script exits 1 after a FAIL.
# Needs the patched msd installed, labwc, xdg-desktop-portal, xdg-desktop-portal-gtk and
# mate-themes. As in 71's wayland.sh, settings go to a dconf database in a scratch
# XDG_CONFIG_HOME, on a bus of the script's own. The portal is started by D-Bus with
# XDG_CURRENT_DESKTOP=MATE, which picks the GTK backend (mate-portals.conf).
work=${TMPDIR:-/tmp}/caminito-180-checks
run=$work/run-${1:-caminito}
if [ -z "$CAMINITO_180_BUS" ]; then
  rm -rf $run; mkdir -p $run/config $run/rt; chmod 700 $run/rt
  export XDG_RUNTIME_DIR=$run/rt XDG_CONFIG_HOME=$run/config CAMINITO_180_BUS=1
  unset DISPLAY WAYLAND_DISPLAY XDG_SESSION_TYPE MATE_WM_NAME GSETTINGS_BACKEND GTK_THEME
  exec dbus-run-session -- bash "$0" "$@"
fi
msd=$(ls /usr/lib/mate-settings-daemon/mate-settings-daemon \
         /usr/libexec/mate-settings-daemon 2>/dev/null | head -1)
# The sleep waits for the compositor, which says nothing when ready.
WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 labwc >$run/labwc.log 2>&1 &
sleep 2
export WAYLAND_DISPLAY=wayland-0 GDK_BACKEND=wayland XDG_CURRENT_DESKTOP=MATE
dbus-update-activation-environment WAYLAND_DISPLAY GDK_BACKEND XDG_CURRENT_DESKTOP \
  XDG_RUNTIME_DIR XDG_CONFIG_HOME
[ "$1" = stock ] || export MATE_WM_NAME=Caminito

status=0
check() {
  if [ "$2" = "$3" ]; then echo "ok    $1: $2"; else echo "FAIL  $1: $2, not $3"; status=1; fi
}
key() { gsettings get org.gnome.desktop.interface color-scheme; }
portal() {
  gdbus call --session --dest org.freedesktop.portal.Desktop \
    --object-path /org/freedesktop/portal/desktop \
    --method org.freedesktop.portal.Settings.ReadOne org.freedesktop.appearance color-scheme
}
# Sets the MATE theme and waits up to a second for the key to be $2, which the portal gives
# as $3.
theme() {
  local start=$(date +%s%N)
  gsettings set org.mate.interface gtk-theme $1
  for _ in $(seq 20); do [ "$(key)" = "'$2'" ] && break; sleep 0.05; done
  local ms=$(( ($(date +%s%N) - start) / 1000000 ))
  check "$1, after $ms ms" "$(key)" "'$2'"
  [ $ms -le 1000 ] || check "$1 within a second" "$ms ms" "1000 ms or less"
  check "$1, the portal" "$(portal)" "(<uint32 $3>,)"
}

gsettings set org.mate.interface gtk-theme BlackMATE
[ "$1" = stock ] && gsettings set org.gnome.desktop.interface color-scheme prefer-light
# msd prints its debug lines with this key and G_MESSAGES_DEBUG both.
gsettings set org.mate.debug mate-settings-daemon true
G_MESSAGES_DEBUG=all $msd --debug --replace >$run/msd.log 2>&1 &
# msd says nothing when the mirror has run.
sleep 4
if [ "$1" = stock ]; then
  echo "--- without MATE_WM_NAME: the key stays prefer-light, at start-up and after a change"
  check "BlackMATE" "$(key)" "'prefer-light'"
  # A second is what the mirror has under Caminito.
  gsettings set org.mate.interface gtk-theme Menta; sleep 1
  check "Menta" "$(key)" "'prefer-light'"
else
  echo "--- at start-up, with BlackMATE"
  check "BlackMATE" "$(key)" "'prefer-dark'"
  check "BlackMATE, the portal" "$(portal)" "(<uint32 1>,)"
  echo "--- live, each within a second"
  theme Menta default 0
  theme BlackMATE prefer-dark 1
  theme Menta default 0
  echo "--- a value set by hand goes at the next theme change"
  gsettings set org.gnome.desktop.interface color-scheme prefer-light
  # Time for the mirror to answer, which it mustn't: no MATE key changed.
  sleep 0.5
  check "set by hand" "$(key)" "'prefer-light'"
  theme BlackMATE prefer-dark 1
fi
echo "--- the msd log"
grep -a "mirroring\|gnome-mirror\|CRITICAL" $run/msd.log | sed 's/.*: [0-9:.]*: //'
kill $(jobs -p) 2>/dev/null; sleep 1
exit $status
