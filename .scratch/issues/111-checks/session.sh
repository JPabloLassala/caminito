#!/usr/bin/env bash
# The session run.sh starts: Caminito nested, the panel, and each of MATE's apps in turn,
# with and without the applet and the module. Then the applet is removed from under two
# apps that run.
set -u
here=$1; root=$2; work=$3; out=$work/out; rm -rf "$out"; mkdir -p "$out"
module=$work/module/usr/lib
prop() { busctl --user get-property org.mate.Caminito /org/mate/Caminito org.mate.Caminito GlobalMenu 2>&1; }
titles() {
  busctl --user call org.mate.Caminito.Debug /org/mate/Caminito/Debug org.mate.Caminito.Debug DumpState --json=short |
    python3 -c "import json,sys; print([w.get('title') for w in json.loads(json.loads(sys.stdin.read())['data'][0]).get('windows', [])])"
}
look() { python3 "$here/look.py" "$@" 2>/dev/null; }
# Starts an app in a process group of its own, with the module if $1 is "module", and waits
# for its window.
start() {
  local with=$1; shift
  if [ "$with" = module ]; then
    GTK_MODULES=appmenu-gtk-module GTK_PATH=$module/gtk-3.0 LD_LIBRARY_PATH=$module setsid "$@" >>"$out/apps.log" 2>&1 &
  else
    setsid "$@" >>"$out/apps.log" 2>&1 &
  fi
  app=$!
  sleep 4
}
apps=(pluma "caja --no-desktop $work" atril mate-terminal)
each() {
  local with=$1 about=$2 command name
  for command in "${apps[@]}"; do
    name=${command%% *}
    start "$with" $command
    echo "-- $name"
    echo "GlobalMenu: $(prop)"
    echo "its window's menubar: $(look bars "$name" | tr '\n' ' ')"
    echo "the panel's menubars: $(look bars mate-panel | tr '\n' ' ')"
    if [ "$about" = about ]; then
      echo "Help > About from the panel: $(look activate mate-panel Help About)"
      sleep 2
      echo "windows: $(titles)"
    fi
    # Its group and nothing by name: the desktop this runs in has a caja of its own.
    kill -- -$app; sleep 2
  done
}
"${CAMINITO:-$root/target/debug/caminito}" 2>"$out/caminito.log" &
caminito=$!
for _ in $(seq 100); do grep -q "listening on WAYLAND_DISPLAY=" "$out/caminito.log" && break; sleep 0.1; done
line=$(grep -o "listening on WAYLAND_DISPLAY=[^ ]*" "$out/caminito.log" | head -1)
export WAYLAND_DISPLAY=${line##*=} GDK_BACKEND=wayland LANG=C LANGUAGE=C
# NO_AT_BRIDGE keeps an app off the accessibility bus, and an editor's terminal may have it.
unset DISPLAY GTK_MODULES NO_AT_BRIDGE
for _ in $(seq 50); do busctl --user list 2>/dev/null | grep -q "^org.mate.Caminito " && break; sleep 0.1; done
# The private bus has no systemd to start the accessibility registry with.
/usr/lib/at-spi2-registryd >/dev/null 2>&1 & registry=$!
sleep 1
{
GSETTINGS_SCHEMA_DIR=$work/schemas mate-panel >"$out/panel.log" 2>&1 & panel=$!
sleep 4
ids=$(dconf read /org/mate/panel/general/object-id-list)
echo "== with the module, without the applet"
each module -
dconf write /org/mate/panel/objects/appmenu/object-type "'applet'"
dconf write /org/mate/panel/objects/appmenu/applet-iid "'AppmenuAppletFactory::AppmenuApplet'"
dconf write /org/mate/panel/objects/appmenu/toplevel-id "'top'"
dconf write /org/mate/panel/objects/appmenu/position 300
dconf write /org/mate/panel/general/object-id-list "${ids%]}, 'appmenu']"
sleep 4
echo "registrar's name: $(busctl --user list | grep -i "AppMenu.Registrar")"
echo "== with the applet, without the module"
each - -
echo "== with the applet and the module"
each module about
echo "== the applet removed from under pluma and mate-terminal, with the module"
start module pluma; pluma=$app
start module mate-terminal
echo "with the applet: pluma's menubar: $(look bars pluma | tr '\n' ' ') mate-terminal's: $(look bars mate-terminal | tr '\n' ' ')"
dconf write /org/mate/panel/general/object-id-list "$ids"
# The switch outlives the applet by a grace period (114).
sleep 14
echo "registrar's name: $(busctl --user list | grep -i "AppMenu.Registrar")"
echo "without it: pluma's menubar: $(look bars pluma | tr '\n' ' ') mate-terminal's: $(look bars mate-terminal | tr '\n' ' ')"
echo "windows: $(titles)"
kill -- -$pluma -$app
echo "== panel log"; grep -i "appmenu\|caminito" "$out/panel.log" | sort | uniq -c | head -10
echo "== apps log"; grep -i "critical\|appmenu\|assertion" "$out/apps.log" | sort | uniq -c | head -20
} >"$out/report" 2>&1
kill $panel $registry 2>/dev/null; sleep 0.5; kill $caminito; wait $caminito 2>/dev/null
