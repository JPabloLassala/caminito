#!/usr/bin/env bash
# The session that run.sh starts on a bus of its own: a panel with the Sound applet alone on
# it, the sound server with its devices and Sound applications, then each state, with the
# popup opened by a click on the applet and driven by the keyboard.
set -u
here=$1; work=$2; out=$3
unset PULSE_SERVER PIPEWIRE_REMOTE

alive() {
  if kill -0 "$panel" 2>/dev/null; then echo "$1: the panel lives"
  else echo "$1: THE PANEL IS GONE"; fi
}
xt() { python3 "$here/xtest.py" "$@"; }
streams() { python3 "$here/streams.py" "$@" | sed 's/^/  /'; }
node() {
  pw-cli create-node adapter "{ factory.name=support.null-audio-sink node.name=$1
    node.description=\"$2\" media.class=$3 object.linger=true audio.position=[FL FR] ${4:-} }" \
    >/dev/null
}
default() {
  for _ in $(seq 50); do pactl "set-default-$1" "$2" 2>/dev/null && return; sleep 0.1; done
  echo "can't make $2 the default $1"
}
open_popup() { xt move 300 300; sleep 0.5; xt click 12 12 1; sleep 1.5; }
close_popup() { xt key Escape; sleep 0.5; }
shot() {
  sleep 1
  # Off the popup, which Xwayland draws only once something more happens.
  xt move 470 600; sleep 0.5
  import -window root -crop "420x900+0+0" +repage "$out/$1.png"
}
# The tab stops before the applications: the output's and the input's mute button, slider
# and pill, then System sounds' slider.
to_system() { xt key Tab 7; }

dconf write /org/mate/panel/general/toplevel-id-list "['panel']"
dconf write /org/mate/panel/toplevels/panel/orientation "'top'"
dconf write /org/mate/panel/toplevels/panel/size 24
dconf write /org/mate/panel/general/object-id-list "['sound']"
dconf write /org/mate/panel/objects/sound/object-type "'applet'"
dconf write /org/mate/panel/objects/sound/applet-iid \
  "'CaminitoSoundAppletFactory::CaminitoSoundApplet'"
dconf write /org/mate/panel/objects/sound/toplevel-id "'panel'"
dconf write /org/mate/panel/objects/sound/position 0

pipewire >"$work/pipewire.log" 2>&1 & pipewire=$!
for _ in $(seq 50); do [ -e "$XDG_RUNTIME_DIR/pipewire-0" ] && break; sleep 0.1; done
wireplumber >"$work/wireplumber.log" 2>&1 & wireplumber=$!
pipewire-pulse >"$work/pipewire-pulse.log" 2>&1 & pulse=$!
for _ in $(seq 50); do pactl info >/dev/null 2>&1 && break; sleep 0.1; done
node speaker "Speakers" Audio/Sink
node headset "WH-1000XM4" Audio/Sink "device.form_factor=headset"
node hdmi "DELL U2720Q" Audio/Sink "device.form_factor=tv"
node mic "Yeti Stereo Microphone" Audio/Source/Virtual
default sink speaker; default source mic
# WirePlumber's route-settings, where System sounds' volume goes, once it has made them.
for _ in $(seq 50); do pw-metadata -n route-settings 2>/dev/null | grep -q Found && break; sleep 0.1; done

mate-panel >"$work/panel.log" 2>&1 &
panel=$!
sleep 4

python3 - "$work/tone.wav" <<'PY'
import math, struct, sys, wave
with wave.open(sys.argv[1], "wb") as w:
    w.setnchannels(1); w.setsampwidth(2); w.setframerate(48000)
    w.writeframes(b"".join(struct.pack("<h", int(20000 * math.sin(2 * math.pi * 440 * i / 48000)
                                               * (0.5 + 0.5 * math.sin(2 * math.pi * i / 24000))))
                           for i in range(48000 * 120)))
PY
play() { pw-cat --playback -P "{ application.name = \"$1\" ${2:-} }" "$work/tone.wav" & }
play mpv "media.name = \"song.flac\""; mpv1=$!
play mpv "media.name = \"talk.webm\""; mpv2=$!
# After mpv's, so that it's listed after them, in the server's order.
sleep 1
play Firefox "media.name = \"A video\""; firefox=$!
"$work/paused" Audacious & paused=$!
sleep 1.5
# mpv's streams at 80% and 40%: the row shows the loudest.
read -r -a mpv_streams <<<"$(python3 "$here/streams.py" mpv | awk '{print $1}' | tr '\n' ' ')"
pactl set-sink-input-volume "${mpv_streams[0]}" 80%
pactl set-sink-input-volume "${mpv_streams[1]}" 40%
sleep 0.5
echo "the streams:"; streams

open_popup; shot 01-applications; alive "the applications"
# mpv's name, past System sounds' slider: Space opens its row.
to_system; xt key Tab; xt key space; shot 02-expanded

# Left on mpv's slider: 80% down a step to 75%, and the other stream in proportion.
xt key Tab; xt key Left; sleep 1
echo "mpv's slider down a step from 80%:"; streams mpv
# Past the slider: "Follow the default", Speakers, WH-1000XM4.
xt key Tab 3; xt key space; sleep 1
echo "mpv moved to WH-1000XM4:"; streams mpv
shot 03-moved
# Back to "Follow the default", then the default changed.
xt key Tab 2 shift; xt key space; sleep 1
echo "mpv following the default again:"; streams mpv
close_popup; alive "the move"
default sink hdmi; sleep 1
echo "mpv after the default changed to HDMI:"; streams mpv
default sink speaker

pw-record -P '{ application.name = "OBS Studio" }' "$work/recording.wav" >/dev/null & recorder=$!
sleep 1.5
# OBS Studio's name, back from the footer's "Sound settings…" past its slider: mpv's row is
# still open from the opening before.
open_popup; xt key Tab 3 shift; xt key space; shot 04-recording
close_popup; alive "the recording"

# System sounds' slider from 100% down 14 steps to 30%, then an event sound.
open_popup; to_system; xt key Left 14; sleep 1; close_popup
canberra-gtk-play -i bell -f "$work/tone.wav" & event=$!
echo "a canberra-gtk-play event sound after System sounds' slider went to 30%:"
# While it plays: libcanberra may play a short part of the file only.
for _ in $(seq 50); do
  heard=$(python3 "$here/streams.py" | grep -w event) && break
  sleep 0.1
done
echo "  ${heard:-none heard}"
kill "$event" 2>/dev/null; alive "System sounds"

kill "$mpv1" "$mpv2" "$firefox" "$paused" "$recorder" 2>/dev/null
kill "$pulse" "$wireplumber" "$pipewire" 2>/dev/null
wait "$pulse" "$wireplumber" "$pipewire" 2>/dev/null
grep -iE 'critical|warning' "$work/panel.log" | grep -v EggSMClient | sed "s/^/panel log: /"
kill "$panel" 2>/dev/null; wait "$panel" 2>/dev/null
