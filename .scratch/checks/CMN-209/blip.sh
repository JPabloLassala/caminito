#!/usr/bin/env bash
# CMN-209, question 4: how long a pinned application plays on the default
# before a libpulse client watching sink-input events moves it.
#
#   .scratch/checks/CMN-209/blip.sh [RUNS [DELAY-MS [PLAYER]]]
#
# Runs pin-probe against a private PipeWire with node.stream.restore-target
# off, starts the application RUNS times, and reads from WirePlumber's log
# when its link to the default (speakers) and its link to the pinned sink
# (headset) were activated. The difference is the blip. DELAY-MS is passed
# to pin-probe. PLAYER is paplay (a libpulse client, the default) or pw-cat (a
# native PipeWire client).

here=$(cd "$(dirname "$0")" && pwd)
export CMN209_ROOT=${CMN209_ROOT:-$(mktemp -d)}
export here RUNS=${1:-20} DELAY=${2:-0} PLAYER=${3:-paplay}

cc -O2 -o "$CMN209_ROOT/pin-probe" "$here/pin-probe.c" \
    $(pkg-config --cflags --libs libpulse-mainloop-glib glib-2.0) || exit 1

exec dbus-run-session -- bash -c '
. "$here/private-pipewire.sh"
WIREPLUMBER_DEBUG=s-linking:5 pw_start; make_sinks
wpctl settings node.stream.restore-target false > /dev/null
"$CMN209_ROOT/pin-probe" Rhythmbox headset "$DELAY" > "$CMN209_ROOT/log/probe" & probe=$!
sleep 0.5
for _ in $(seq "$RUNS"); do
    if [ "$PLAYER" = pw-cat ]; then
        timeout 0.6 pw-cat -p --raw --rate 48000 --channels 2 --format s16 \
            -P "{ application.name = Rhythmbox }" /dev/zero & APP_PID=$!
    else
        play_app Rhythmbox 0.6
    fi
    wait $APP_PID; sleep 0.4
done
kill $probe
pw_cleanup
python3 "$here/blip-analyse.py" "$CMN209_ROOT/log/wireplumber" "$CMN209_ROOT/log/probe"
'
