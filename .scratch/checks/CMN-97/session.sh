#!/usr/bin/env bash
# One computer's session, which run.sh starts in CMN-99's fake tree on a session bus of its
# own: a panel with the Thermal applet alone on it, then each step of that computer.
set -u
computer=$1; work=$2; out=$3
hwmon=$UMOCKDEV_DIR/sys/class/hwmon

# set_value <chip> <file> <value>: one of the fake chip's files, as the kernel would change it
set_value() {
  for h in "$hwmon"/*; do
    if [ "$(cat "$h/name")" = "$1" ]; then echo "$3" >"$h/$2"; fi
  done
}
alive() {
  if kill -0 "$panel" 2>/dev/null; then echo "$computer, $1: the panel lives"
  else echo "$computer, $1: THE PANEL IS GONE"; fi
}
# The popup's window: the one 340 to 420 wide below the panel. Sets X Y WIDTH HEIGHT.
find_popup() {
  found=; for w in $(xdotool search --onlyvisible --name '' 2>/dev/null); do
    eval "$(xdotool getwindowgeometry --shell "$w")"
    if [ "$WIDTH" -ge 340 ] && [ "$WIDTH" -le 420 ] && [ "$Y" -ge 20 ]; then
      found="$X $Y $WIDTH $HEIGHT"
    fi
  done
  set -- ${found:-}
  X=${1:-}; Y=${2:-}; WIDTH=${3:-}; HEIGHT=${4:-}
}
open_popup() { xdotool mousemove 14 14 click 1; sleep 1.2; find_popup; }
close_popup() { xdotool mousemove 470 890 click 1; sleep 0.8; }
# Show all sensors is the popup's last line: its margin (12) and half the expander's line.
toggle_show_all() { xdotool mousemove $((X + 40)) $((Y + HEIGHT - 12 - 9)) click 1; sleep 1.2; find_popup; }
# shot <name>: the popup alone, with the pointer away from its rows
shot() {
  xdotool mousemove 470 20
  sleep 0.5
  if [ -z "$X" ]; then echo "$computer, $1: NO POPUP"; return; fi
  echo "$computer, $1: the popup is ${WIDTH}x$HEIGHT"
  import -window root -crop "${WIDTH}x$HEIGHT+$X+$Y" +repage "$out/$1.png"
}

dconf write /org/mate/panel/general/toplevel-id-list "['panel']"
dconf write /org/mate/panel/toplevels/panel/orientation "'top'"
dconf write /org/mate/panel/toplevels/panel/size 32
dconf write /org/mate/panel/general/object-id-list "['thermal']"
dconf write /org/mate/panel/objects/thermal/object-type "'applet'"
dconf write /org/mate/panel/objects/thermal/applet-iid \
  "'CaminitoThermalAppletFactory::CaminitoThermalApplet'"
dconf write /org/mate/panel/objects/thermal/toplevel-id "'panel'"
dconf write /org/mate/panel/objects/thermal/position 0

mate-panel >"$work/panel-$computer.log" 2>&1 &
panel=$!
# A read at the start and one 2 s later: three reads of the fan by the first expanded shot.
sleep 3

case $computer in
  thinkpad)
    open_popup
    shot t1-collapsed; alive "collapsed"
    toggle_show_all
    shot t2-expanded; alive "expanded"
    # Every second with the popup open: well past 10 reads.
    sleep 10
    shot t3-fan-bar; alive "after 10 reads"
    close_popup
    open_popup
    shot t4-reopened; alive "reopened"
    ;;
  desktop)
    open_popup
    shot d1-collapsed; alive "collapsed"
    toggle_show_all
    shot d2-expanded; alive "expanded"
    set_value k10temp temp1_input 80000
    set_value amdgpu temp2_input 105000
    sleep 2.5
    shot d3-warm-hot; alive "warm and hot"
    ;;
esac
grep -iE 'critical|warning' "$work/panel-$computer.log" | sed "s/^/$computer, panel log: /"
kill "$panel" 2>/dev/null; wait "$panel" 2>/dev/null
