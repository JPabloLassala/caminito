#!/usr/bin/env bash
# One session of elsewhere.sh: a display of one kind, one module, and each app in turn.
set -u
here=$1; work=$2; where=$3; which=$4; module=$work/$which/usr/lib
log=$work/log; mkdir -p "$log"
export LANG=C LANGUAGE=C GSETTINGS_SCHEMA_DIR=$work/$which/schemas
# NO_AT_BRIDGE keeps an app off the accessibility bus, and an editor's terminal may have it.
unset DISPLAY WAYLAND_DISPLAY GTK_MODULES NO_AT_BRIDGE
if [ $where = x11 ]; then
  Xvfb :111 -screen 0 1280x800x24 >/dev/null 2>&1 & display=$!
  export DISPLAY=:111 GDK_BACKEND=x11
  sleep 1
else
  WLR_BACKENDS=headless wayfire -c "$work/wayfire.ini" >"$log/wayfire" 2>&1 & display=$!
  for _ in $(seq 50); do grep -q "Using socket name" "$log/wayfire" && break; sleep 0.1; done
  export WAYLAND_DISPLAY=$(grep -o "Using socket name .*" "$log/wayfire" | cut -d' ' -f4) GDK_BACKEND=wayland
fi
# The private bus has no systemd to start the accessibility registry with.
/usr/lib/at-spi2-registryd >/dev/null 2>&1 & registry=$!
sleep 1
props() {
  local id
  for id in $(xwininfo -root -tree | grep -o '0x[0-9a-f]* "' | cut -d' ' -f1); do
    xprop -id "$id" _GTK_MENUBAR_OBJECT_PATH _UNITY_OBJECT_PATH 2>/dev/null | grep '= "'
  done | sort -u | tr '\n' ' '
}
for owned in no yes; do
  [ $owned = yes ] && { python3 "$here/../110-checks/own.py" com.canonical.AppMenu.Registrar 60 & owner=$!; sleep 1; }
  for command in pluma "caja --no-desktop $work" atril mate-terminal mate-calc; do
    name=${command%% *}
    GTK_MODULES=appmenu-gtk-module GTK_PATH=$module/gtk-3.0 LD_LIBRARY_PATH=$module setsid $command >"$log/app" 2>&1 & app=$!
    sleep 4
    echo "registrar $owned, $name: menubar: $(timeout 5 python3 "$here/look.py" bars "$name" 2>/dev/null | tr '\n' ' ')"
    [ $where = x11 ] && echo "  properties: $(props)"
    grep -i "critical\|assertion" "$log/app" | sed 's/^.*\*\*: [0-9:.]*: /  /' | sort -u
    # Its group and nothing by name: the desktop this runs in has a caja of its own. A stock
    # module's pluma is in a loop and takes no less than a KILL.
    kill -KILL -- -$app; wait $app 2>/dev/null; sleep 1
  done
done >>"$work/$where-$which" 2>&1
kill ${owner:-} $registry $display 2>/dev/null
