#!/usr/bin/env bash
# A login after session.sh's, which run.sh starts on a new bus with the same settings and
# state: the sound server and the panel start again, and Spotify, pinned to HDMI in the
# session before, plays.
set -u
here=$1; work=$2
unset PULSE_SERVER PIPEWIRE_REMOTE

streams() { python3 "$here/streams.py" "$@" | sed 's/^/  /'; }
node() {
  pw-cli create-node adapter "{ factory.name=support.null-audio-sink node.name=$1
    node.description=\"$2\" media.class=$3 object.linger=true audio.position=[FL FR] ${4:-} }" \
    >/dev/null
}

echo "after logging out, the new session's pins:"
echo "  $(dconf read /org/mate/panel/applet/caminito-sound/pins)"
pipewire >>"$work/pipewire.log" 2>&1 & pipewire=$!
for _ in $(seq 50); do [ -e "$XDG_RUNTIME_DIR/pipewire-0" ] && break; sleep 0.1; done
wireplumber >>"$work/wireplumber.log" 2>&1 & wireplumber=$!
pipewire-pulse >>"$work/pipewire-pulse.log" 2>&1 & pulse=$!
for _ in $(seq 50); do pactl info >/dev/null 2>&1 && break; sleep 0.1; done
echo "WirePlumber's target restore before the panel starts: $(wpctl settings node.stream.restore-target)"
node speaker "Speakers" Audio/Sink
node hdmi "DELL U2720Q" Audio/Sink "device.form_factor=tv"
for _ in $(seq 50); do pactl set-default-sink speaker 2>/dev/null && break; sleep 0.1; done
mate-panel >>"$work/panel.log" 2>&1 & panel=$!
sleep 4
pw-cat --playback -P '{ application.name = "Spotify" }' "$work/tone.wav" & spotify=$!
sleep 1.5
echo "Spotify in the new session:"; streams Spotify
kill "$spotify" "$panel" 2>/dev/null
kill "$pulse" "$wireplumber" "$pipewire" 2>/dev/null
wait "$pulse" "$wireplumber" "$pipewire" "$panel" 2>/dev/null
