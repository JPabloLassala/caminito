#!/bin/bash
# The patched msd on X11, under Xvfb with no MATE_WM_NAME: it owns _XSETTINGS_S0, draws the
# background on the root window, starts the typing-break plugin and leaves the GNOME keys
# alone. It needs xwd and ImageMagick for the background's pixel.
# Settings go to a dconf database in a scratch XDG_CONFIG_HOME, and the script starts its own
# session bus after it sets that, as wayland.sh does.
# The sleeps wait for Xvfb, msd and the redraw, which say nothing when ready.
here=$(cd "$(dirname "$0")" && pwd)
work=${TMPDIR:-/tmp}/caminito-71-checks
run=$work/run-x11
if [ -z "$CAMINITO_71_BUS" ]; then
  rm -rf $run; mkdir -p $run/config $run/rt; chmod 700 $run/rt
  export XDG_RUNTIME_DIR=$run/rt XDG_CONFIG_HOME=$run/config CAMINITO_71_BUS=1
  unset DISPLAY WAYLAND_DISPLAY XDG_SESSION_TYPE MATE_WM_NAME GSETTINGS_BACKEND GDK_BACKEND
  exec dbus-run-session -- bash "$0" "$@"
fi
Xvfb :72 -screen 0 1280x800x24 >/dev/null 2>&1 &
sleep 1
export DISPLAY=:72
echo "_XSETTINGS_S0 owner before msd: $(python3 $here/probe.py owner _XSETTINGS_S0)"
gsettings set org.mate.debug mate-settings-daemon true
gsettings set org.mate.typing-break enabled true
gsettings set org.mate.background show-desktop-icons false
gsettings set org.mate.background picture-filename ''
gsettings set org.mate.background color-shading-type solid
gsettings set org.mate.background primary-color '#ff0000'
gsettings set org.mate.interface gtk-theme BlackMATE
G_MESSAGES_DEBUG=all /usr/lib/mate-settings-daemon/mate-settings-daemon --debug --replace >$run/msd.log 2>&1 &
sleep 6
gsettings set org.mate.interface font-name 'Serif 13'
sleep 2
echo "_XSETTINGS_S0 owner with msd: $(python3 $here/probe.py owner _XSETTINGS_S0)"
echo "root pixel (ff0000 set): $(xwd -root -silent | magick xwd:- -format '%[hex:u.p{300,300}]\n' info:)"
echo "GNOME gtk-theme: $(gsettings get org.gnome.desktop.interface gtk-theme)"
echo "GNOME font-name: $(gsettings get org.gnome.desktop.interface font-name)"
echo "GNOME keys in the database: $(dconf dump /org/gnome/ | grep -c =)"
echo "--- the msd log"
grep -a "): DEBUG\|WARNING" $run/msd.log | grep -ai "typing_break\|mirroring\|xsettings manager\|background manager" | sed 's/.*: [0-9:.]*: //'
kill $(jobs -p) 2>/dev/null; sleep 1
