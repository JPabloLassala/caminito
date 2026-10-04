#!/bin/bash
# The patched panel on X11, under Xvfb with Marco: the applets load in the panel's process, the
# window list sets _NET_WM_ICON_GEOMETRY, and the panel asks the session manager about Shut
# Down. Run it inside dbus-run-session.
here=$(cd "$(dirname "$0")" && pwd)
work=${TMPDIR:-/tmp}/camino-70-checks
run=$work/run-x11; rm -rf $run; mkdir -p $run/config $run/rt; chmod 700 $run/rt
export XDG_RUNTIME_DIR=$run/rt XDG_CONFIG_HOME=$run/config GSETTINGS_BACKEND=keyfile
unset WAYLAND_DISPLAY XDG_SESSION_TYPE
export DISPLAY=:77 GDK_BACKEND=x11
Xvfb :77 -screen 0 1280x720x24 >$run/xvfb.log 2>&1 &
sleep 1
marco >$run/marco.log 2>&1 &
python3 $here/fake-sm.py >$run/sm.log 2>&1 &
zenity --info --title=T70X11 --text=first >/dev/null 2>&1 &
sleep 2
mate-panel >$run/panel.log 2>&1 &
sleep 8
echo "--- windows"
for id in $(xprop -root _NET_CLIENT_LIST | sed 's/.*# //;s/,//g'); do
  xprop -id $id WM_NAME _NET_WM_WINDOW_TYPE _NET_WM_ICON_GEOMETRY | tr '\n' ' '; echo
done
echo "--- applets loaded in process"; grep -c "libwnck-applet\|libclock-applet\|notification-area" /proc/$(pgrep -n -x mate-panel)/maps
echo "--- session manager calls"; grep -v Deprec $run/sm.log | grep -v register_object
echo "--- panel log"; grep -i "critical\|warning\|error" $run/panel.log | head
kill $(jobs -p) 2>/dev/null
