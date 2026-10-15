#!/usr/bin/env bash
# The session run.sh starts, on a session bus of its own: mate-session's mock on it, a
# private bus for the system bus (UPower isn't there: the applet shows its plug), a panel
# with the applet alone on it, then each step.
set -u
root=$1; work=$2; out=$3
session=org.gnome.SessionManager
path=/org/gnome/SessionManager

DBUS_SYSTEM_BUS_ADDRESS=$(dbus-daemon --session --fork --print-address=1 --print-pid=3 3>"$work/system-bus.pid")
export DBUS_SYSTEM_BUS_ADDRESS

# The app IDs of mate-session's inhibitors, as the mock lists them.
listed() {
  for p in $(gdbus call --session --dest $session --object-path $path \
      --method org.gnome.SessionManager.GetInhibitors | grep -o '/org/gnome/SessionManager/Inhibitor[0-9]*'); do
    gdbus call --session --dest $session --object-path "$p" \
      --method org.gnome.SessionManager.Inhibitor.GetAppId | sed "s/^('//; s/',)\$//"
  done | tr '\n' ' '
}
mock() { gdbus call --session --dest $session --object-path $path --method "org.caminito.SessionMock.$1" "${@:2}"; }
log() { mock GetLog | sed -E "s/^\((@as )?\[//; s/\],\)\$//; s/', '/;  /g; s/'//g"; }

"$CAMINITO_PYTHON" -m dbusmock --template "$root/helpers/caminito-power-applet/session-mock.py" \
  >"$work/mock.log" 2>&1 &
mockpid=$!
for _ in $(seq 100); do
  gdbus introspect --session --dest $session --object-path $path >/dev/null 2>&1 && break
  sleep 0.1
done

dconf write /org/mate/panel/general/toplevel-id-list "['panel']"
dconf write /org/mate/panel/toplevels/panel/orientation "'top'"
dconf write /org/mate/panel/toplevels/panel/size 28
dconf write /org/mate/panel/general/object-id-list "['power']"
dconf write /org/mate/panel/objects/power/object-type "'applet'"
dconf write /org/mate/panel/objects/power/applet-iid \
  "'CaminitoPowerAppletFactory::CaminitoPowerApplet'"
dconf write /org/mate/panel/objects/power/toplevel-id "'panel'"
dconf write /org/mate/panel/objects/power/position 0

alive() { if kill -0 "$panel" 2>/dev/null; then echo "$1: panel running"; else echo "$1: PANEL GONE"; fi; }
away() { xdotool mousemove 390 390 click 1; sleep 0.8; }
start_panel() { mate-panel >>"$work/panel.log" 2>&1 & panel=$!; sleep 4; }
# The popup's window: the one 300 to 340 wide below the panel. Sets X Y WIDTH HEIGHT.
find_popup() {
  found=; for w in $(xdotool search --onlyvisible --name '' 2>/dev/null); do
    eval "$(xdotool getwindowgeometry --shell "$w")"
    if [ "$WIDTH" -ge 300 ] && [ "$WIDTH" -le 340 ] && [ "$Y" -ge 20 ] && [ "$HEIGHT" -le 380 ]; then
      found="$X $Y $WIDTH $HEIGHT"
    fi
  done
  set -- ${found:-}
  X=${1:-}; Y=${2:-}; WIDTH=${3:-}; HEIGHT=${4:-}
}
open_popup() { xdotool mousemove 14 14 click 1; sleep 1.2; find_popup; }
shot() { import -window root "$out/$1.png"; }
# Stay awake's switch is the first row's right end: the popup's border (8) and half the row.
press_switch() { xdotool mousemove $((X + WIDTH - 8 - 26)) $((Y + 8 + 20)) click 1; sleep 1.2; }

start_panel
alive "start"

open_popup
echo "popup (x y w h): ${X:+$X $Y $WIDTH $HEIGHT}"
shot 1-off
echo "1 off: the mock's log: [$(log)]"

press_switch
shot 2-on
echo "2 on: the mock's log: [$(log)]"

mock AddInhibitor firefox "Playing video" 8 >/dev/null
mock AddInhibitor org.example.NoDesktopFile "Backing up the photos to a disk that has a long name" 4 >/dev/null
mock AddInhibitor pluma "Unsaved changes" 1 >/dev/null
sleep 1.5
find_popup
shot 3-application
echo "3 applications: mate-session lists [$(listed)] (pluma has the logout flag alone, so the popup doesn't)"

for p in $(gdbus call --session --dest $session --object-path $path \
    --method org.gnome.SessionManager.GetInhibitors | grep -o '/org/gnome/SessionManager/Inhibitor[0-9]*'); do
  case "$(gdbus call --session --dest $session --object-path "$p" \
      --method org.gnome.SessionManager.Inhibitor.GetAppId)" in
    *firefox*|*NoDesktopFile*|*pluma*) mock RemoveInhibitor "$p" >/dev/null ;;
  esac
done
sleep 1.5
find_popup
shot 4-removed
echo "4 removed: mate-session lists [$(listed)]"

press_switch
shot 5-off-again
echo "5 off again: the mock's log: [$(log)]"
alive "popup"
away

# Stay awake on, and the panel killed: mate-session drops the inhibitor with its owner, and
# the next panel starts with the switch off.
open_popup
press_switch
echo "6 on again: the mock's log: [$(log)]"
away
kill "$panel"; wait "$panel" 2>/dev/null
sleep 1.5
echo "6 the panel gone: the mock's log: [$(log)]"
start_panel
open_popup
shot 6-restarted
alive "restarted"

kill "$panel" 2>/dev/null; wait "$panel" 2>/dev/null
kill "$mockpid" 2>/dev/null
kill "$(cat "$work/system-bus.pid")" 2>/dev/null
wait 2>/dev/null
