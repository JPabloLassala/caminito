#!/usr/bin/env bash
# The session that run.sh starts on a bus of its own: a panel with the Sound applet alone on
# it, the sound server with its devices and Sound applications, then each state, with the
# popup opened by a click on the applet and driven by the keyboard.
set -u
here=$1; work=$2; out=$3
unset PULSE_SERVER PIPEWIRE_REMOTE
key=/org/mate/panel/applet/caminito-sound/pins

alive() {
  if kill -0 "$panel" 2>/dev/null; then echo "$1: the panel lives"
  else echo "$1: THE PANEL IS GONE"; fi
}
xt() { python3 "$here/xtest.py" "$@"; }
streams() { python3 "$here/streams.py" "$@" | sed 's/^/  /'; }
pins() { echo "  $(dconf read $key)"; }
node() {
  pw-cli create-node adapter "{ factory.name=support.null-audio-sink node.name=$1
    node.description=\"$2\" media.class=$3 object.linger=true audio.position=[FL FR] ${4:-} }" \
    >/dev/null
}
# The headset as a module, so that it can be turned off and on.
headset_on() {
  headset=$(pactl load-module module-null-sink sink_name=headset \
    'sink_properties=device.description=WH-1000XM4 device.form_factor=headset')
}
default() {
  for _ in $(seq 50); do pactl "set-default-$1" "$2" 2>/dev/null && return; sleep 0.1; done
  echo "can't make $2 the default $1"
}
open_popup() { xt move 300 300; sleep 0.5; xt click 12 12 1; sleep 1.5; }
close_popup() { xt key Escape; sleep 0.5; }
shot() {
  sleep 1
  xt move 470 600; sleep 0.5
  import -window root -crop "420x900+0+0" +repage "$out/$1.png"
}
start_panel() { mate-panel >>"$work/panel.log" 2>&1 & panel=$!; sleep 4; }
# The tab stops before Pinned applications: the output's and the input's mute button, slider
# and pill.
to_pinned() { xt key Tab 6; }
play() { pw-cat --playback -P "{ application.name = \"$1\" ${2:-} }" "$work/tone.wav" & }

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
# Its links, with their times, for the blip below.
WIREPLUMBER_DEBUG=s-linking:5 wireplumber >"$work/wireplumber.log" 2>&1 & wireplumber=$!
pipewire-pulse >"$work/pipewire-pulse.log" 2>&1 & pulse=$!
for _ in $(seq 50); do pactl info >/dev/null 2>&1 && break; sleep 0.1; done
node speaker "Speakers" Audio/Sink
node hdmi "DELL U2720Q" Audio/Sink "device.form_factor=tv"
node mic "Yeti Stereo Microphone" Audio/Source/Virtual
headset_on
default sink speaker; default source mic

start_panel
echo "WirePlumber's target restore, once the applet connected: $(wpctl settings node.stream.restore-target)"

python3 - "$work/tone.wav" <<'PY'
import math, struct, sys, wave
with wave.open(sys.argv[1], "wb") as w:
    w.setnchannels(1); w.setsampwidth(2); w.setframerate(48000)
    w.writeframes(b"".join(struct.pack("<h", int(20000 * math.sin(2 * math.pi * 440 * i / 48000)
                                               * (0.5 + 0.5 * math.sin(2 * math.pi * i / 24000))))
                           for i in range(48000 * 120)))
PY
play mpv "media.name = \"song.flac\""; mpv=$!
sleep 1
play Firefox "media.name = \"A video\""; firefox=$!
"$work/paused" Rhythmbox & paused=$!
sleep 1.5

# mpv moved to the headset without a pin, then pinned from its row: to where it plays now.
pactl move-sink-input "$(python3 "$here/streams.py" mpv | awk '{print $1}')" headset
sleep 0.5
# System sounds' slider, then mpv's name, slider and pin.
open_popup; to_pinned; xt key Tab 4; xt key space; sleep 1; close_popup
echo "mpv pinned from its row:"; pins
# Rhythmbox, paused, and Spotify, not running, pinned too, as another applet would.
dconf write $key "[{'side': 'output', 'app': 'mpv', 'name': 'mpv', 'device': 'headset',
  'device-description': 'WH-1000XM4'}, {'side': 'output', 'app': 'Rhythmbox',
  'name': 'Rhythmbox', 'icon': 'rhythmbox', 'device': 'headset',
  'device-description': 'WH-1000XM4'}, {'side': 'output', 'app': 'Spotify', 'name': 'Spotify',
  'icon': 'spotify-client', 'device': 'hdmi', 'device-description': 'DELL U2720Q'}]"
sleep 1
echo "the pins:"; pins
echo "the streams, Rhythmbox's paused one moved to its pin:"; streams
open_popup; shot 01-pinned; close_popup; alive "the pins"

# The headset off, and on again.
pactl unload-module "$headset"; sleep 1
echo "WH-1000XM4 turned off:"; streams
open_popup; shot 02-device-gone; close_popup
headset_on; sleep 1.5
echo "WH-1000XM4 on again:"; streams
alive "the device gone and back"

# mpv's ×: past the heading's ×, mpv's name and slider.
open_popup; to_pinned; xt key Tab 4; xt key space; shot 03-row-strip
# The same × again closes it; back to the heading's ×, focused, which opens its own.
xt key space; xt key Tab 3 shift; xt key space; shot 04-heading-strip
xt key space; close_popup; alive "the strips"

# mpv unpinned: its × and its strip's Unpin, past Cancel.
pactl set-sink-input-volume "$(python3 "$here/streams.py" mpv | awk '{print $1}')" 60%
echo "before unpinning mpv:"; pins; streams
open_popup; to_pinned; xt key Tab 4; xt key space; xt key Tab 2; xt key space
shot 05-undo
echo "mpv unpinned:"; pins; streams mpv
# The focus went with the row: from the top, past the heading's ×, the Undo.
xt key Tab 8; xt key space; sleep 1.5
echo "undone:"; pins; streams mpv
close_popup; alive "the unpin"

# Unpin all, with mpv at 60% and Rhythmbox muted: the heading's ×, then its strip's
# "Unpin all", past Cancel.
pactl set-sink-input-mute "$(python3 "$here/streams.py" Rhythmbox | awk '{print $1}')" 1
echo "before unpinning all:"; pins
streams
open_popup; to_pinned; xt key Tab; xt key space; xt key Tab 2; xt key space
shot 06-undo-all
echo "all unpinned, volumes and mutes as they were:"; pins
streams
# No pinned row left: the Undo is the first stop after the input's pill.
xt key Tab 7; xt key space; sleep 1.5
echo "undone:"; pins; streams
close_popup; alive "unpin all"

# How long a pinned application's new stream plays on the default first: Spotify, pinned to
# HDMI, ten times, alone.
kill "$mpv" "$firefox" "$paused" 2>/dev/null; sleep 1
mark=$(wc -l <"$work/wireplumber.log")
for _ in $(seq 10); do
  timeout 0.6 pw-cat --playback -P '{ application.name = "Spotify" }' "$work/tone.wav"
  sleep 0.4
done
echo "Spotify started ten times, pinned to HDMI, from WirePlumber's links:"
tail -n +"$((mark + 1))" "$work/wireplumber.log" | python3 "$here/blip.py" speaker hdmi \
  | sed 's/^/  /'

# mate-panel restarted: the pin's still applied.
kill "$panel"; wait "$panel" 2>/dev/null
start_panel
play Spotify; spotify=$!; sleep 1.5
echo "Spotify after mate-panel restarted:"; streams Spotify
kill "$spotify" 2>/dev/null; alive "the restart"

# A move without a pin, forgotten when the application restarts.
play Firefox; firefox=$!; sleep 1.5
pactl move-sink-input "$(python3 "$here/streams.py" Firefox | awk '{print $1}')" headset; sleep 0.5
echo "Firefox moved to WH-1000XM4 without a pin:"; streams Firefox
kill "$firefox"; sleep 0.5
play Firefox; firefox=$!; sleep 1.5
echo "Firefox started again:"; streams Firefox
kill "$firefox" 2>/dev/null

kill "$pulse" "$wireplumber" "$pipewire" 2>/dev/null
wait "$pulse" "$wireplumber" "$pipewire" 2>/dev/null
grep -iE 'critical|warning' "$work/panel.log" | grep -v EggSMClient | sed "s/^/panel log: /"
kill "$panel" 2>/dev/null; wait "$panel" 2>/dev/null
