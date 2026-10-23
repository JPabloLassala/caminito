#!/usr/bin/env bash
# The session that run.sh starts on a bus of its own: a panel with the Sound applet alone on
# it, started before the sound server, then each state set through pactl.
set -u
here=$1; work=$2; out=$3; real_runtime_dir=${4:-}
unset PULSE_SERVER PIPEWIRE_REMOTE

alive() {
  if kill -0 "$panel" 2>/dev/null; then echo "$1: the panel lives"
  else echo "$1: THE PANEL IS GONE"; fi
}
xt() { python3 "$here/xtest.py" "$@"; }
start_server() {
  pipewire >"$work/pipewire.log" 2>&1 & pipewire=$!
  for _ in $(seq 50); do [ -e "$XDG_RUNTIME_DIR/pipewire-0" ] && break; sleep 0.1; done
  wireplumber >"$work/wireplumber.log" 2>&1 & wireplumber=$!
  pipewire-pulse >"$work/pipewire-pulse.log" 2>&1 & pulse=$!
  for _ in $(seq 50); do pactl info >/dev/null 2>&1 && break; sleep 0.1; done
}
stop_server() {
  kill "$pulse" "$wireplumber" "$pipewire" 2>/dev/null
  wait "$pulse" "$wireplumber" "$pipewire" 2>/dev/null
}
sink() { pactl load-module module-null-sink sink_name="$1" "sink_properties=$2" >/dev/null; }
# shot <name> [<height>]: the panel's corner, with the tooltip under it
shot() {
  sleep 1
  xt move 200 200; xt move 12 12
  sleep 1.5
  import -window root -crop "320x${2:-90}+0+0" +repage -filter point -resize 200% "$out/$1.png"
}
volume() { pactl get-sink-volume @DEFAULT_SINK@ | grep -oE '[0-9]+%' | head -1; }
mute() { pactl get-sink-mute @DEFAULT_SINK@; }

dconf write /org/mate/panel/general/toplevel-id-list "['panel']"
dconf write /org/mate/panel/toplevels/panel/orientation "'top'"
dconf write /org/mate/panel/toplevels/panel/size 24
dconf write /org/mate/panel/general/object-id-list "['sound']"
dconf write /org/mate/panel/objects/sound/object-type "'applet'"
dconf write /org/mate/panel/objects/sound/applet-iid \
  "'CaminitoSoundAppletFactory::CaminitoSoundApplet'"
dconf write /org/mate/panel/objects/sound/toplevel-id "'panel'"
dconf write /org/mate/panel/objects/sound/position 0

mate-panel >"$work/panel.log" 2>&1 &
panel=$!
sleep 4
shot 01-no-sound-server; alive "no sound server"
xt click 12 12 1; shot 02-popup-no-server 200; xt click 400 300 1
alive "the popup with no sound server"

start_server
sink speaker "device.description=Speakers"
sink headset "device.description=WH-1000XM4 device.form_factor=headset"
# pipewire-pulse's module arguments end a value at a space, quoted or not.
sink hdmi "device.description=HDMI device.form_factor=tv"
sink mic "media.class=Audio/Source/Virtual device.description=Microphone"
pactl set-default-sink speaker; pactl set-default-source mic
n=3
for level in 100 50 20; do
  pactl set-sink-volume speaker "$level%"
  shot "$(printf %02d $n)-speaker-$level"; alive "speaker at $level%"
  n=$((n + 1))
done
pactl set-sink-volume speaker 45%; pactl set-sink-mute speaker 1
shot 06-speaker-muted; alive "speaker muted"
pactl set-sink-mute speaker 0; pactl set-sink-volume speaker 0%
shot 07-speaker-0; alive "speaker at 0"
pactl set-default-sink headset; pactl set-sink-volume headset 45%
shot 08-headphones; alive "headphones"
pactl set-sink-mute headset 1
shot 09-headphones-muted; alive "headphones muted"
pactl set-default-sink hdmi; pactl set-sink-volume hdmi 70%
shot 10-monitor-output; alive "a monitor's output"
pw-record -P '{ application.name = "OBS Studio" }' "$work/recording.wav" 2>/dev/null &
recorder=$!
shot 11-recording; alive "recording"
kill "$recorder"; wait "$recorder" 2>/dev/null

# The wheel and middle-click, on the speakers.
pactl set-default-sink speaker; pactl set-sink-volume speaker 45%; sleep 0.5
xt click 12 12 4; sleep 0.3; xt click 12 12 4; sleep 0.5
echo "two steps up from 45%: $(volume)"
xt click 12 12 5; sleep 0.5
echo "one step down: $(volume)"
pactl set-sink-volume speaker 98%; sleep 0.5
xt click 12 12 4; sleep 0.3; xt click 12 12 4; sleep 0.5
echo "two steps up from 98%, with allow-volume-above-100-percent off: $(volume)"
gsettings set org.gnome.desktop.sound allow-volume-above-100-percent true; sleep 0.5
xt click 12 12 4; sleep 0.5
echo "a step up from 100%, with it on: $(volume)"
pactl set-sink-volume speaker 148%; sleep 0.5
xt click 12 12 4; sleep 0.3; xt click 12 12 4; sleep 0.5
echo "two steps up from 148%, with it on: $(volume)"
gsettings set org.mate.SettingsDaemon.plugins.media-keys volume-step 10
pactl set-sink-volume speaker 45%; sleep 0.5
xt click 12 12 4; sleep 0.5
echo "a step up from 45% with volume-step 10: $(volume)"
xt click 12 12 2; sleep 0.5
echo "middle-click: $(mute)"
xt click 12 12 2; sleep 0.5
echo "middle-click again: $(mute)"
alive "the wheel and middle-click"

pactl set-sink-mute speaker 0
xt click 12 12 1; shot 12-popup 200; xt click 400 300 1; alive "the popup"
sleep 0.5
xt click 12 12 3; sleep 1
import -window root -crop 320x200+0+0 +repage -filter point -resize 200% "$out/13-menu.png"
xt click 400 300 1; sleep 0.5; alive "the menu"

stop_server
shot 14-server-stopped; alive "the server stopped"
start_server
sink speaker "device.description=Speakers"
pactl set-default-sink speaker
shot 15-server-back; alive "the server back"
stop_server

grep -iE 'critical|warning' "$work/panel.log" | sed "s/^/panel log: /"
kill "$panel" 2>/dev/null; wait "$panel" 2>/dev/null

# The menu against the user's own sound server, whose cards have profiles, which null
# devices haven't. It only opens the menu: nothing is clicked in it.
if [ -n "$real_runtime_dir" ]; then
  XDG_RUNTIME_DIR=$real_runtime_dir mate-panel >"$work/panel-real.log" 2>&1 &
  panel=$!
  sleep 4
  xt click 12 12 3; sleep 1
  import -window root -crop 480x320+0+0 +repage "$out/16-menu-real-cards.png"
  xt click 470 310 1; sleep 0.5; alive "the menu on the user's own server"
  grep -iE 'critical|warning' "$work/panel-real.log" | sed "s/^/panel log: /"
  kill "$panel" 2>/dev/null; wait "$panel" 2>/dev/null
fi
