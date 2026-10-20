#!/usr/bin/env bash
# One computer's session, which run.sh starts in bed.py's fake tree on a session bus of its
# own: a panel with the Thermal applet alone on it, then each step of that computer.
set -u
computer=$1; work=$2; out=$3
hwmon=$UMOCKDEV_DIR/sys/class/hwmon

# set <chip> <file> <value>: one of the fake chip's files, as the kernel would change it
set_value() {
  for h in "$hwmon"/*; do
    if [ "$(cat "$h/name")" = "$1" ]; then echo "$3" >"$h/$2"; fi
  done
}
# remove <chip>: the chip goes, as when its module is unloaded
remove() {
  for h in "$hwmon"/*; do
    if [ "$(cat "$h/name")" = "$1" ]; then
      rm -r "$(realpath "$h")" "$h"
    fi
  done
}
panel_size() { dconf write /org/mate/panel/toplevels/panel/size "$1"; size=$1; }
panel_orientation() { dconf write /org/mate/panel/toplevels/panel/orientation "'$1'"; }
alive() {
  if kill -0 "$panel" 2>/dev/null; then echo "$computer, $1: the panel lives"
  else echo "$computer, $1: THE PANEL IS GONE"; fi
}
# shot <name> [<w>x<h>]: the panel's corner, after two reads at least
shot() {
  sleep 4.5
  import -window root -crop "${2:-200x$size}+0+0" +repage -filter point -resize 200% \
    "$out/$1.png"
}

echo "== $computer: sensors reads"
sensors 2>&1

dconf write /org/mate/panel/general/toplevel-id-list "['panel']"
panel_orientation top
panel_size 32
dconf write /org/mate/panel/general/object-id-list "['thermal']"
dconf write /org/mate/panel/objects/thermal/object-type "'applet'"
dconf write /org/mate/panel/objects/thermal/applet-iid \
  "'CaminitoThermalAppletFactory::CaminitoThermalApplet'"
dconf write /org/mate/panel/objects/thermal/toplevel-id "'panel'"
dconf write /org/mate/panel/objects/thermal/position 0

mate-panel >"$work/panel-$computer.log" 2>&1 &
panel=$!
sleep 4

case $computer in
  thinkpad)
    shot t1-32px; alive "at 32 px"
    panel_size 24
    shot t2-24px; alive "at 24 px"
    panel_orientation left
    panel_size 40
    shot t3-vertical 40x120; alive "on a vertical panel"
    ;;
  desktop)
    shot d1-32px; alive "at 32 px"
    panel_size 24
    shot d2-24px; alive "at 24 px"
    panel_size 32
    set_value k10temp temp1_input 80000
    set_value amdgpu temp2_input 105000
    shot d3-warm-hot; alive "warm and hot"
    set_value nct6798 fan1_input 0
    set_value nct6798 fan2_input 0
    shot d4-fans-off; alive "every fan at 0"
    remove amdgpu
    shot d5-gpu-gone; alive "amdgpu's chip removed"
    ;;
esac
grep -iE 'critical|warning' "$work/panel-$computer.log" | sed "s/^/$computer, panel log: /"
kill "$panel" 2>/dev/null; wait "$panel" 2>/dev/null
