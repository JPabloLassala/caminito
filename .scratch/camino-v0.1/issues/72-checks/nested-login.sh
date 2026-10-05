#!/bin/bash
# The installed `camino --session` as caminotest, for a machine with no tty and no GPU: nested
# on Xvfb, but on caminotest's own systemd user manager and bus, so the systemd export and the
# journal are the real ones (14). It is not the login on a tty that 72 asks for: there is one
# output, no input device, no logind session and no VT.
#
#   sudo nested-login.sh
#
# It makes the user if there is none, starts the session, waits for it, prints what 72's
# criteria ask about, logs out through mate-session, and prints what is left. The files it
# makes, a screenshot among them, are in /home/caminotest/72-nested-login.
set -uo pipefail

here=$(cd "$(dirname "$0")" && pwd)
user=caminotest
x_display=:72

id $user >/dev/null 2>&1 || useradd -m $user
uid=$(id -u $user)
work=/home/$user/72-nested-login
rm -rf $work
install -d -o $user -g $user $work
install -o $user -g $user -m 755 "$here/gtk-probe.py" "$here/xwd-to-png.py" $work/

# The user's systemd manager and bus, as a login gives.
loginctl enable-linger $user
for _ in $(seq 50); do [ -S /run/user/$uid/bus ] && break; sleep 0.2; done

# Runs a command as the user with a login's environment and nothing of the caller's.
as_user() {
  runuser -u $user -- env -i HOME=/home/$user USER=$user LOGNAME=$user SHELL=/bin/bash \
    PATH=/usr/local/bin:/usr/bin LANG=C.UTF-8 XDG_RUNTIME_DIR=/run/user/$uid \
    DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$uid/bus "$@"
}
# The same, with the displays that Camino exported to systemd.
in_session() {
  as_user env $(as_user systemctl --user show-environment | grep -E '^(WAYLAND_DISPLAY|DISPLAY)=') "$@"
}
session_environment() {
  as_user systemctl --user show-environment |
    grep -E '^(WAYLAND_DISPLAY|DISPLAY|MATE_WM_NAME|XDG_SESSION_TYPE|XDG_CURRENT_DESKTOP|XCURSOR_[A-Z]+)=' | sort
}
dump_state() {
  in_session gdbus call --session --dest org.mate.Camino.Debug --object-path /org/mate/Camino/Debug \
    --method org.mate.Camino.Debug.DumpState
}
screenshot() {
  as_user xwd -root -silent -display $x_display -out $work/$1.xwd &&
    as_user $work/xwd-to-png.py $work/$1.xwd $work/$1.png && rm $work/$1.xwd
}

echo "== the systemd environment before"
session_environment

as_user Xvfb $x_display -screen 0 1280x800x24 >/dev/null 2>&1 &
for _ in $(seq 50); do as_user xdpyinfo -display $x_display >/dev/null 2>&1 && break; sleep 0.2; done

since=$(date '+%Y-%m-%d %H:%M:%S')
# stderr is a file, as it is from a display manager, so the logs go to the journal.
as_user env DISPLAY=$x_display camino --session </dev/null >$work/camino.stderr 2>&1 &
runuser=$!

for _ in $(seq 150); do session_environment | grep -q '^WAYLAND_DISPLAY=' && break; sleep 0.2; done
# mate-session's phases, and the panel, caja and msd on screen. Nothing says when it's done.
sleep 25
camino=$(pgrep -u $user -x camino)

echo "== the systemd environment in the session"
session_environment

echo "== the session's processes"
pgrep -u $user -a | grep -vE 'systemd|\(sd-pam\)|Xvfb|dbus-(daemon|broker)|pgrep'
echo "mate-screensaver running: $(pgrep -u $user -x mate-screensaver >/dev/null && echo yes || echo no)"

echo "== the screensaver names (camino is pid $camino)"
for name in org.mate.ScreenSaver org.freedesktop.ScreenSaver; do
  echo "$name: pid $(as_user busctl --user status $name 2>&1 | sed -n 's/^PID=//p')"
done

echo "== the desktop"
dump_state >$work/dump-desktop.txt 2>&1
screenshot desktop

echo "== a GTK3 window, maximised, native and on XWayland"
for key in gtk-theme font-name; do echo "org.mate.interface $key: $(in_session gsettings get org.mate.interface $key)"; done
echo "org.mate.peripherals-mouse cursor-theme: $(in_session gsettings get org.mate.peripherals-mouse cursor-theme)"
in_session env GDK_BACKEND=wayland $work/gtk-probe.py probe-native 2>$work/probe-native.stderr &
in_session env GDK_BACKEND=x11 $work/gtk-probe.py probe-x11 2>$work/probe-x11.stderr &
sleep 5
dump_state >$work/dump-windows.txt 2>&1
screenshot windows
sleep 4

echo "== the journal: lines for each identifier"
journalctl _UID=$uid --since "$since" -o json --no-pager |
  grep -o '"SYSLOG_IDENTIFIER":"[^"]*"' | sort | uniq -c
journalctl _UID=$uid --since "$since" --no-pager -o short-precise >$work/journal.txt
echo "== the journal: camino's warnings and errors"
journalctl _UID=$uid -t camino --since "$since" --no-pager -p warning -o cat | sort | uniq -c | head -30
echo "== the journal: timeouts"
grep -iE 'timed out|timeout' $work/journal.txt | head

echo "== logout"
in_session mate-session-save --logout
for _ in $(seq 150); do kill -0 $runuser 2>/dev/null || break; sleep 0.2; done
if kill -0 $runuser 2>/dev/null; then
  echo "camino is still running after 30 s"
  pkill -u $user -x camino
fi
wait $runuser
echo "camino status=$?"
sleep 1
echo "== the systemd environment after"
session_environment
echo "== what is left of the session"
pgrep -u $user -a | grep -vE 'systemd|\(sd-pam\)|Xvfb|dbus-(daemon|broker)|pgrep'
journalctl _UID=$uid --since "$since" --no-pager -o short-precise >$work/journal.txt
echo "== the journal: camino's last lines"
journalctl _UID=$uid -t camino --since "$since" --no-pager -o cat | tail -15

pkill -u $user -x Xvfb
# So the developer's user can read the results.
chmod a+x /home/$user
chmod -R a+rX $work
