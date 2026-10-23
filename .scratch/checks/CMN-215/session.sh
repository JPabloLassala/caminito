#!/usr/bin/env bash
# The session that run.sh starts on a bus of its own, once on a tall screen ("full") and once
# on a short one ("short"): a panel with the Sound applet alone on it, started before the
# sound server, then each state set through pactl, with the popup opened by a click on the
# applet and driven by the keyboard.
set -u
mode=$1; here=$2; work=$3; out=$4
unset PULSE_SERVER PIPEWIRE_REMOTE
# The output's slider, in the popup under the 24px panel: past the round button, left of the
# pill.
slider_x=140; slider_y=48

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
# node <name> <description> <media class> [<properties>]: a null device, made with pw-cli,
# since pipewire-pulse's module arguments end a value at a space, quoted or not.
node() {
  pw-cli create-node adapter "{ factory.name=support.null-audio-sink node.name=$1
    node.description=\"$2\" media.class=$3 object.linger=true audio.position=[FL FR] ${4:-} }" \
    >/dev/null
}
sink() { node "$1" "$2" Audio/Sink "${3:-}"; }
# PipeWire makes a node a moment after it's asked for.
default() {
  for _ in $(seq 50); do pactl "set-default-$1" "$2" 2>/dev/null && return; sleep 0.1; done
  echo "can't make $2 the default $1"
}
open_popup() { xt move 300 300; sleep 0.5; xt click 12 12 1; sleep 1.5; }
close_popup() { xt key Escape; sleep 0.5; }
# shot <name>: the screen's left part, with the popup under the panel
shot() {
  sleep 1
  # Off the popup, which Xwayland draws only once something more happens.
  xt move 470 300; sleep 0.5
  import -window root -crop "400x$(xdpyinfo | grep -oP 'dimensions:\s+\d+x\K\d+')+0+0" +repage \
    "$out/$1.png"
}
volume() { pactl get-sink-volume @DEFAULT_SINK@ | grep -oE '[0-9]+%' | head -1; }

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

if [ "$mode" = full ]; then
  open_popup; shot 01-no-sound-server; close_popup; alive "no sound server"
fi

start_server
sink speaker "Speakers"
sink headset "WH-1000XM4" "device.form_factor=headset"
sink hdmi "DELL U2720Q" "device.form_factor=tv"
sink easyeffects "Easy Effects Sink"
for i in 1 2 3 4 5 6; do sink "extra$i" "Output $i"; done
default sink speaker
# A microphone that hears what the speakers play: a source that isn't a monitor.
pactl load-module module-remap-source source_name=mic master=speaker.monitor \
  source_properties=device.description=Microphone >/dev/null
node yeti "Yeti Stereo Microphone" Audio/Source/Virtual
default sink speaker; default source mic
pactl set-sink-volume speaker 60%

if [ "$mode" = full ]; then
  open_popup; shot 02-speaker; close_popup; alive "the speaker"
  pactl set-sink-mute speaker 1
  open_popup; shot 03-speaker-muted; close_popup
  pactl set-sink-mute speaker 0
  default sink headset; pactl set-sink-volume headset 45%
  open_popup; shot 04-headphones; close_popup
  pactl set-sink-mute headset 1
  open_popup; shot 05-headphones-muted; close_popup
  pactl set-sink-mute headset 0
  default sink hdmi; pactl set-sink-volume hdmi 70%
  open_popup; shot 06-monitor; close_popup
  pactl set-sink-mute hdmi 1
  open_popup; shot 07-monitor-muted; close_popup; alive "each output's form"
  pactl set-sink-mute hdmi 0
  default sink speaker

  # The keyboard alone: Tab to the output's pill, Space opens the list, Tab to WH-1000XM4,
  # the list's second, Space makes it the default.
  open_popup
  xt key Tab 3; xt key space; shot 08-outputs-open
  xt key Tab 2; xt key space; sleep 1
  echo "the output the keyboard chose: $(pactl get-default-sink)"
  shot 09-output-chosen
  close_popup; alive "the output's list"
  # Space on the round button mutes it; the arrows move the slider by the step.
  pactl set-sink-volume headset 45%
  open_popup
  xt key Tab; xt key space; sleep 0.5
  echo "Space on the output's mute button: $(pactl get-sink-mute headset)"
  xt key space; sleep 0.5
  xt key Tab; xt key Right; sleep 0.5
  echo "Right on the output's slider from 45%: $(volume)"
  close_popup; alive "the output's mute button and slider"

  # The output's list stays open from the opening before: Space on its pill closes it, and
  # Tab goes on to the input's pill, past the input's mute button and slider.
  open_popup
  xt key Tab 3; xt key space; xt key Tab 3; xt key space; shot 10-inputs-open
  xt key Tab 2; xt key space; sleep 1
  echo "the input the keyboard chose: $(pactl get-default-source)"
  default source mic
  close_popup; alive "the input's list"

  # A tone the microphone hears through the speakers' monitor.
  python3 - "$work/tone.wav" <<'PY'
import math, struct, sys, wave
with wave.open(sys.argv[1], "wb") as w:
    w.setnchannels(1); w.setsampwidth(2); w.setframerate(48000)
    w.writeframes(b"".join(struct.pack("<h", int(20000 * math.sin(2 * math.pi * 440 * i / 48000)
                                               * (0.5 + 0.5 * math.sin(2 * math.pi * i / 24000))))
                           for i in range(48000 * 20)))
PY
  pw-play --target speaker "$work/tone.wav" & player=$!
  # The speakers' monitor hears them after their volume.
  default sink speaker; pactl set-sink-volume speaker 100%
  open_popup; shot 11-meter-a; sleep 0.3; shot 11-meter-b; close_popup
  kill "$player" 2>/dev/null; wait "$player" 2>/dev/null; alive "the meter"

  # The wheel over the output's slider.
  pactl set-sink-volume speaker 60%; open_popup
  xt click "$slider_x" "$slider_y" 4 shift; sleep 0.5
  echo "Shift+wheel up over the slider from 60%: $(volume)"
  xt click "$slider_x" "$slider_y" 5 shift; xt click "$slider_x" "$slider_y" 5 shift; sleep 0.5
  echo "Shift+wheel down twice: $(volume)"
  xt click "$slider_x" "$slider_y" 5; xt click "$slider_x" "$slider_y" 4; sleep 0.5
  echo "the plain wheel down and up over the slider: $(volume)"
  close_popup; alive "the wheel"

  gsettings set org.gnome.desktop.sound allow-volume-above-100-percent true
  pactl set-sink-volume speaker 148%; sleep 0.5; open_popup
  xt click "$slider_x" "$slider_y" 4 shift; xt click "$slider_x" "$slider_y" 4 shift; sleep 0.5
  echo "Shift+wheel up twice from 148%, with allow-volume-above-100-percent on: $(volume)"
  close_popup
  gsettings set org.gnome.desktop.sound allow-volume-above-100-percent false
  pactl set-sink-volume speaker 98%; sleep 0.5; open_popup
  xt click "$slider_x" "$slider_y" 4 shift; xt click "$slider_x" "$slider_y" 4 shift; sleep 0.5
  echo "Shift+wheel up twice from 98%, with it off: $(volume)"
  close_popup; alive "the cap"

  "$work/shoot-ports" "$out/14-port-dropdown.png" || echo "the port dropdown couldn't be drawn"
else
  open_popup
  xt key Tab 3; xt key space; shot 12-short-screen
  before=$(volume)
  for _ in 1 2 3 4 5 6; do xt click "$slider_x" "$slider_y" 5; done
  shot 13-short-scrolled
  echo "the plain wheel down over the slider on the short screen: $before, then $(volume)"
  close_popup; alive "the short screen"
fi

stop_server
grep -iE 'critical|warning' "$work/panel.log" | grep -v EggSMClient | sed "s/^/panel log: /"
kill "$panel" 2>/dev/null; wait "$panel" 2>/dev/null
