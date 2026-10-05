#!/usr/bin/env bash
# Ticket 61's nested checks, on Xvfb, for a machine with no desktop to nest in. It runs the
# debug build in $CAMINO (default ~/target-61/debug/camino) with this script's arguments on a
# private bus, for at most 60 s, and prints Camino's exit status, the stderr of what it
# spawned, and what the session command saw.
#
#   61-nested-checks.sh "--session=foot $HOME/t61/foot-inner.sh"   # foot shows, exits with 5
#   61-nested-checks.sh --session                                  # mate-session starts
#
# Camino's own log is in the journal, since its stderr here isn't a terminal (14):
#   journalctl -t camino --since "2 minutes ago"
# foot-inner.sh, which this writes, runs inside foot: it takes Camino's state dump, which
# lists foot's window, and foot's environment, then exits with status 5.
W=$HOME/t61; mkdir -p $W; cd $W
cat > $W/foot-inner.sh <<EOF
#!/bin/sh
sleep 2
gdbus call --session --dest org.mate.Camino.Debug --object-path /org/mate/Camino/Debug --method org.mate.Camino.Debug.DumpState >$W/dump.json 2>&1
env | grep -E "^(WAYLAND_DISPLAY|DISPLAY|XDG_SESSION_TYPE|XDG_CURRENT_DESKTOP|MATE_WM|XCURSOR|GDK_BACKEND)" | sort >$W/foot-env
sleep 1
exit 5
EOF
chmod +x $W/foot-inner.sh; rm -f $W/dump.json $W/foot-env
export XDG_RUNTIME_DIR=$(mktemp -d -p $W); chmod 700 $XDG_RUNTIME_DIR; unset WAYLAND_DISPLAY DBUS_SESSION_BUS_ADDRESS
Xvfb :61 -screen 0 1280x800x24 >/dev/null 2>&1 & xvfb=$!
for i in $(seq 50); do xdpyinfo -display :61 >/dev/null 2>&1 && break; sleep 0.2; done
DISPLAY=:61 XDG_CONFIG_HOME=$(mktemp -d -p $W) timeout 60 dbus-run-session -- ${CAMINO:-$HOME/target-61/debug/camino} "$@" 2>$W/camino.log; echo "camino status=$?"
kill $xvfb; grep -v "^Xwayland: \|^camino-gtk" $W/camino.log | tail -40; echo ---; cat $W/foot-env 2>/dev/null; echo ---; head -c 1200 $W/dump.json 2>/dev/null; echo
