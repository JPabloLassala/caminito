#!/bin/bash
# The patched msd on Wayland, under a headless labwc, with MATE_WM_NAME=Caminito or, with
# "stock" as the first argument, without it: which plugins start, the mirror into the GNOME
# keys, the background's layer surfaces while caja's show-desktop-icons is on, and the
# clipboard plugin keeping the clipboard.
# Settings go to a dconf database in a scratch XDG_CONFIG_HOME. The script starts its own
# session bus after it sets that, because dconf-service takes the bus's environment, and
# would write the user's database otherwise. (The keyfile backend isn't used: it loses
# changes when msd and gsettings both write.)
# The sleeps wait for the compositor and for msd, which say nothing when ready.
work=${TMPDIR:-/tmp}/caminito-71-checks
run=$work/run-wayland-${1:-caminito}
if [ -z "$CAMINITO_71_BUS" ]; then
  rm -rf $run; mkdir -p $run/config $run/rt; chmod 700 $run/rt
  export XDG_RUNTIME_DIR=$run/rt XDG_CONFIG_HOME=$run/config CAMINITO_71_BUS=1
  unset DISPLAY WAYLAND_DISPLAY XDG_SESSION_TYPE MATE_WM_NAME GSETTINGS_BACKEND
  exec dbus-run-session -- bash "$0" "$@"
fi
WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 labwc >$run/labwc.log 2>&1 &
sleep 2
export WAYLAND_DISPLAY=wayland-0 GDK_BACKEND=wayland
[ "$1" = stock ] || export MATE_WM_NAME=Caminito
# msd prints its debug lines with this key and G_MESSAGES_DEBUG both.
gsettings set org.mate.debug mate-settings-daemon true
gsettings set org.mate.interface gtk-theme BlackMATE
gsettings set org.mate.Marco.general action-double-click-titlebar toggle_maximize_horizontally
gsettings set org.mate.Marco.general action-middle-click-titlebar toggle_shade
gsettings set org.mate.font-rendering dpi 120
gsettings set org.mate.typing-break enabled true
echo "show-desktop-icons: $(gsettings get org.mate.background show-desktop-icons)"
G_MESSAGES_DEBUG=all WAYLAND_DEBUG=1 /usr/lib/mate-settings-daemon/mate-settings-daemon --debug --replace >$run/msd.log 2>&1 &
sleep 6
gnome() { gsettings get org.gnome.desktop.$1 $2; }
echo "--- at start-up"
echo "gtk-theme: $(gnome interface gtk-theme)"
echo "double-click action: $(gnome wm.preferences action-double-click-titlebar)"
echo "middle-click action: $(gnome wm.preferences action-middle-click-titlebar)"
echo "text-scaling-factor: $(gnome interface text-scaling-factor)"
echo "--- live"
gsettings set org.mate.interface gtk-theme Menta
gsettings set org.mate.interface font-name 'Serif 13'
gsettings set org.mate.peripherals-mouse cursor-theme mate-black
gsettings set org.mate.peripherals-mouse cursor-size 48
gsettings set org.mate.sound event-sounds false
gsettings set org.mate.font-rendering antialiasing grayscale
sleep 1
echo "gtk-theme: $(gnome interface gtk-theme)"
echo "font-name: $(gnome interface font-name)"
echo "cursor-theme: $(gnome interface cursor-theme)"
echo "cursor-size: $(gnome interface cursor-size)"
echo "event-sounds: $(gnome sound event-sounds)"
echo "font-antialiasing: $(gnome interface font-antialiasing)"
echo "--- one way: a GNOME-side change stays until a MATE key changes"
gsettings set org.gnome.desktop.interface gtk-theme Adwaita; sleep 1
echo "MATE gtk-theme: $(gsettings get org.mate.interface gtk-theme)"
gsettings set org.mate.interface cursor-blink-time 900; sleep 1
echo "GNOME gtk-theme after another MATE key changed: $(gnome interface gtk-theme)"
echo "--- the clipboard after its owner quits (needs wl-clipboard)"
# msd takes the selection over 2 s after its owner goes.
wl-copy "ticket 71" 2>/dev/null; sleep 2; pkill -x wl-copy; sleep 4
echo "wl-copy running: $(pgrep -c -x wl-copy), pasted: $(timeout 3 wl-paste 2>&1)"
sleep 2
echo "--- the msd log"
grep -c "get_layer_surface" $run/msd.log | sed 's/^/layer surfaces: /'
grep -o 'get_layer_surface([^)]*)' $run/msd.log | head -2
grep -a "): DEBUG\|WARNING" $run/msd.log | grep -ai "typing_break\|mirroring\|xsettings manager\|requires X11\|caja desktop" | sed 's/.*: [0-9:.]*: //'
grep -o "ext_data_control_manager_v1[^ ]*\|zwlr_output_manager_v1#[0-9]*\.[a-z_]*" $run/msd.log | sort | uniq -c | head
echo "typing monitor running: $(pgrep -c -x mate-typing-mon)"
kill $(jobs -p) 2>/dev/null; sleep 1
