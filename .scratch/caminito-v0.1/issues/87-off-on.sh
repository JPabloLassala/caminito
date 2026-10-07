#!/bin/sh
# Ticket 87: turns each monitor off and on again under a compositor with no desktop in it,
# so that what the other monitor does is the commit's doing and not the panels' or the
# wallpaper's. Run it from a text login on a spare tty, as the developer:
#
#   .scratch/caminito-v0.1/issues/87-off-on.sh caminito
#   .scratch/caminito-v0.1/issues/87-off-on.sh wayfire
#
# Under caminito the saved layout gives the monitors their modes; under wayfire, which is
# wlroots and not Smithay, the script sets the same ones. A terminal says what's about to
# happen. Caminito's background is grey, so a monitor that goes black has lost its picture.
# The compositor's log is ~/87-<compositor>.log.

set -u
compositor=${1:?caminito or wayfire}
lg=HDMI-A-1
aoc=DP-1
log=$HOME/87-$compositor.log

if [ "${2:-}" = steps ]; then
    say() { printf '\n%s\n' "$*"; }
    off_on() {
        say "$1 goes off in 5 s. Watch the other monitor."
        sleep 5
        wlr-randr --output "$1" --off
        say "$1 is off. It comes back in 5 s. Watch the other monitor."
        sleep 5
        wlr-randr --output "$1" --on --mode "$2"
        say "$1 is on."
        sleep 3
    }
    if [ "$compositor" != caminito ]; then
        wlr-randr --output "$aoc" --mode 1920x1080@143.994995Hz
        wlr-randr --output "$lg" --mode 3840x2160@59.939999Hz
        sleep 3
    fi
    wlr-randr
    off_on "$lg" 3840x2160@59.939999Hz
    off_on "$aoc" 1920x1080@143.994995Hz
    say "Done. This closes in 5 s."
    sleep 5
    exit 0
fi

sockets() { ls "$XDG_RUNTIME_DIR" | grep -E '^wayland-[0-9]+$'; }
before=$(sockets)
RUST_LOG=caminito=debug,smithay=debug "$compositor" >"$log" 2>&1 &
pid=$!
socket=
for _ in $(seq 50); do
    socket=$(sockets | grep -vxF "$before" | head -n 1)
    [ -n "$socket" ] && break
    sleep 0.2
done
if [ -z "$socket" ]; then
    echo "$compositor didn't start: see $log"
    kill "$pid" 2>/dev/null
    exit 1
fi
sleep 2
WAYLAND_DISPLAY=$socket foot "$0" "$compositor" steps
kill "$pid"
wait "$pid" 2>/dev/null
echo "The log is $log"
