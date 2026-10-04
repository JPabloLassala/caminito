#!/bin/bash
H=/home/jpablo/t70/check
T=$H/run-x11; rm -rf $T; mkdir -p $T/config $T/rt; chmod 700 $T/rt
export XDG_RUNTIME_DIR=$T/rt XDG_CONFIG_HOME=$T/config GSETTINGS_BACKEND=keyfile
unset WAYLAND_DISPLAY XDG_SESSION_TYPE
export DISPLAY=:77 GDK_BACKEND=x11
Xvfb :77 -screen 0 1280x720x24 >$T/xvfb.log 2>&1 &
sleep 1
marco >$T/marco.log 2>&1 &
python3 $H/fake-sm.py >$T/sm.log 2>&1 &
zenity --info --title=T70X11 --text=first >/dev/null 2>&1 &
sleep 2
mate-panel >$T/panel.log 2>&1 &
sleep 8
echo "--- windows"
for id in $(xprop -root _NET_CLIENT_LIST | sed 's/.*# //;s/,//g'); do
  xprop -id $id WM_NAME _NET_WM_WINDOW_TYPE _NET_WM_ICON_GEOMETRY | tr '\n' ' '; echo
done
echo "--- applets loaded in process"; grep -c "libwnck-applet\|libclock-applet\|notification-area" /proc/$(pgrep -n -x mate-panel)/maps
echo "--- session manager calls"; grep -v Deprec $T/sm.log | grep -v register_object
echo "--- panel log"; grep -i "critical\|warning\|error" $T/panel.log | head
kill $(jobs -p) 2>/dev/null
