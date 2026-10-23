#!/usr/bin/env bash
# A private PipeWire, WirePlumber and pipewire-pulse for CMN-209's experiments.
#
# Source it inside `dbus-run-session -- bash`, so nothing reaches the session's
# bus. Everything lives under $CMN209_ROOT: XDG_RUNTIME_DIR, XDG_CONFIG_HOME and
# XDG_STATE_HOME, so the user's ~/.config/wireplumber and
# ~/.local/state/wireplumber are never read or written. WirePlumber's hardware
# monitors are off, so the real sound card is left alone; the sinks are null
# sinks.
#
#   CMN209_ROOT=$(mktemp -d) dbus-run-session -- bash -c '
#     . .scratch/checks/CMN-209/private-pipewire.sh
#     pw_start; make_sinks; …; pw_cleanup'

set -u

: "${CMN209_ROOT:?set CMN209_ROOT to a throwaway directory}"
# A socket path must fit in 108 bytes, so the runtime directory is a short one.
export XDG_RUNTIME_DIR="${CMN209_RUNTIME:-$(mktemp -d /tmp/cmn209.XXXXXX)}"
export XDG_CONFIG_HOME="$CMN209_ROOT/config"
export XDG_STATE_HOME="$CMN209_ROOT/state"
export PULSE_SERVER="unix:$XDG_RUNTIME_DIR/pulse/native"
unset PIPEWIRE_REMOTE PULSE_RUNTIME_PATH
mkdir -p "$XDG_RUNTIME_DIR" "$XDG_CONFIG_HOME/wireplumber/wireplumber.conf.d" \
    "$XDG_STATE_HOME" "$CMN209_ROOT/log"
chmod 700 "$XDG_RUNTIME_DIR"

cat > "$XDG_CONFIG_HOME/wireplumber/wireplumber.conf.d/00-no-hardware.conf" <<'EOF'
wireplumber.profiles = {
  main = {
    monitor.alsa = disabled
    monitor.alsa-midi = disabled
    monitor.bluez = disabled
    monitor.bluez-midi = disabled
    monitor.v4l2 = disabled
    monitor.libcamera = disabled
    support.reserve-device = disabled
    support.logind = disabled
  }
}
EOF

PW_PID= WP_PID= PP_PID=

pw_start() {
    pipewire > "$CMN209_ROOT/log/pipewire" 2>&1 & PW_PID=$!
    sleep 0.5
    wp_start
    pipewire-pulse > "$CMN209_ROOT/log/pipewire-pulse" 2>&1 & PP_PID=$!
    for _ in $(seq 50); do pactl info > /dev/null 2>&1 && return 0; sleep 0.1; done
    echo "pipewire-pulse did not come up" >&2; return 1
}

wp_start() {
    WIREPLUMBER_DEBUG="${WIREPLUMBER_DEBUG:-s-node:4,s-linking:4}" \
        wireplumber >> "$CMN209_ROOT/log/wireplumber" 2>&1 & WP_PID=$!
    for _ in $(seq 50); do wpctl settings node.stream.restore-target > /dev/null 2>&1 && break; sleep 0.1; done
    sleep 0.5
}

# WirePlumber writes its state files about a second after a change, and a
# WirePlumber stopped before then loses the change; so wait first.
wp_restart() {
    sleep 1.5
    kill "$WP_PID"; wait "$WP_PID" 2> /dev/null
    wp_start
}

pw_stop() {
    sleep 1.5
    kill $PP_PID $WP_PID 2> /dev/null; wait $PP_PID $WP_PID 2> /dev/null
    kill $PW_PID 2> /dev/null; wait $PW_PID 2> /dev/null
}

# pw_cleanup: stops the stack and removes the runtime directory.
pw_cleanup() {
    pw_stop
    rm -rf "$XDG_RUNTIME_DIR"
}

# Two null sinks standing for speakers and a headset; speakers is the default.
make_sinks() {
    pactl load-module module-null-sink sink_name=speakers \
        sink_properties=device.description=Speakers > /dev/null
    pactl load-module module-null-sink sink_name=headset \
        sink_properties=device.description=Headset > /dev/null
    sleep 0.5
    pactl set-default-sink speakers
    sleep 0.3
}

# play_app NAME SECONDS: a libpulse client playing silence, as the application
# NAME, in the background; its PID is in APP_PID.
APP_PID=
play_app() {
    timeout "$2" paplay --raw --rate=48000 --channels=2 --format=s16le \
        --property=application.name="$1" /dev/zero & APP_PID=$!
}

# sink_index_of SINK: the index of the sink named SINK.
sink_index_of() {
    pactl -f json list sinks | python3 -c '
import json, sys
print(next(s["index"] for s in json.load(sys.stdin) if s["name"] == sys.argv[1]))
' "$1"
}

# sink_of NAME: the sink the application NAME's sink input plays to.
sink_of() {
    local idx
    idx=$(sink_input_of "$1") || return 1
    pactl -f json list sink-inputs | python3 -c '
import json, sys
idx = int(sys.argv[1])
sinks = {s["index"]: s["name"] for s in json.load(open(sys.argv[2]))}
for si in json.load(sys.stdin):
    if si["index"] == idx:
        print(sinks[si["sink"]])
' "$idx" <(pactl -f json list sinks)
}

sink_input_of() {
    pactl -f json list sink-inputs | python3 -c '
import json, sys
for si in json.load(sys.stdin):
    if si["properties"].get("application.name") == sys.argv[1]:
        print(si["index"]); break
else:
    sys.exit(1)
' "$1"
}

state_file() { cat "$XDG_STATE_HOME/wireplumber/stream-properties" 2> /dev/null; }
