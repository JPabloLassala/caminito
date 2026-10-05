#!/bin/bash
# The patched msd in a nested Camino on Xvfb, with MATE_WM_NAME=Camino: who owns
# _XSETTINGS_S0, a native GTK3 window following MATE's theme, font and cursor, the wallpaper,
# the typing-break plugin, and a saved monitors-wayland.xml at scale 1.5.
# It needs xwd and ImageMagick for the wallpaper's pixel. CAMINO names a camino binary (a debug build, as tools/dev-nested --bin makes).
# Settings go to a dconf database in a scratch XDG_CONFIG_HOME, and the script starts its own
# session bus after it sets that, as wayland.sh does.
# The sleeps wait for Xvfb, Camino, msd and the redraws, which say nothing when ready.
here=$(cd "$(dirname "$0")" && pwd)
work=${TMPDIR:-/tmp}/camino-71-checks
run=$work/run-camino
if [ -z "$CAMINO_71_BUS" ]; then
  rm -rf $run; mkdir -p $run/config $run/rt; chmod 700 $run/rt
  export XDG_RUNTIME_DIR=$run/rt XDG_CONFIG_HOME=$run/config CAMINO_71_BUS=1 NO_COLOR=1
  unset DISPLAY WAYLAND_DISPLAY XDG_SESSION_TYPE MATE_WM_NAME GSETTINGS_BACKEND
  exec dbus-run-session -- bash "$0" "$@"
fi
probe() { python3 $here/probe.py "$@"; }
# One pixel of the Xvfb screen, where Camino's window is at the top left.
pixel() { xwd -root -silent -display :71 | magick xwd:- -format '%[hex:u.p{300,300}]\n' info:; }
Xvfb :71 -screen 0 1600x1000x24 >/dev/null 2>&1 &
sleep 1
DISPLAY=:71 ${CAMINO:-camino} >$run/camino.log 2>&1 &
sleep 6
export WAYLAND_DISPLAY=$(sed -n 's/.*listening on WAYLAND_DISPLAY=//p' $run/camino.log | head -1)
export DISPLAY=$(sed -n 's/.*Xwayland is ready on DISPLAY=//p' $run/camino.log | head -1)
echo "nested: WAYLAND_DISPLAY=$WAYLAND_DISPLAY DISPLAY=$DISPLAY"
export MATE_WM_NAME=Camino GDK_BACKEND=wayland
echo "--- before msd"
wlr-randr | grep -i "scale"
probe layout $XDG_CONFIG_HOME/monitors-wayland.xml 1.5
before=$(probe owner _XSETTINGS_S0); echo "_XSETTINGS_S0 owner: $before"
gsettings set org.mate.debug mate-settings-daemon true
gsettings set org.mate.typing-break enabled true
gsettings set org.mate.background picture-filename ''
gsettings set org.mate.background color-shading-type solid
gsettings set org.mate.background primary-color '#ff0000'
gsettings set org.mate.interface gtk-theme BlackMATE
probe watch >$run/watch.log 2>&1 &
G_MESSAGES_DEBUG=all WAYLAND_DEBUG=1 /usr/lib/mate-settings-daemon/mate-settings-daemon --debug --replace >$run/msd.log 2>&1 &
sleep 8
echo "--- with msd"
echo "_XSETTINGS_S0 owner: $(probe owner _XSETTINGS_S0) (was $before)"
wlr-randr | grep -i "scale"
echo "wallpaper pixel (ff0000 set): $(pixel)"
probe png $run/blue.png 0000ff
gsettings set org.mate.background picture-filename $run/blue.png
gsettings set org.mate.background picture-options stretched
gsettings set org.mate.interface gtk-theme Menta
gsettings set org.mate.interface font-name 'Serif 13'
gsettings set org.mate.peripherals-mouse cursor-theme mate-black
sleep 4
echo "wallpaper pixel (a blue picture set): $(pixel)"
echo "GNOME gtk-theme: $(gsettings get org.gnome.desktop.interface gtk-theme)"
echo "--- the background's layer surface"
grep -a -o 'get_layer_surface([^)]*)\|zwlr_layer_surface_v1#[0-9]*\.configure([^)]*)' $run/msd.log
echo "--- the GTK3 window"; cat $run/watch.log
echo "--- the msd log"
grep -a "): DEBUG\|WARNING" $run/msd.log | grep -ai "typing_break\|mirroring\|xsettings manager\|wlrandr.*\(match\|appl\)\|clipboard manager" | sed 's/.*: [0-9:.]*: //'
echo "typing monitor running: $(pgrep -c -x mate-typing-mon)"
echo "--- the Camino log"; grep -i "warn\|error\|panic" $run/camino.log | grep -v "winit_x11\|egl::ffi\|gtk-palette" | cut -c1-200 | head
kill $(jobs -p) 2>/dev/null; sleep 1
