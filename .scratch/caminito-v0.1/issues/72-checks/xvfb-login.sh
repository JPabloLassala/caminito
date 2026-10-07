#!/bin/bash
# The installed `caminito --session` as caminitotest, for a machine with no tty and no GPU: a
# winit window on Xvfb, but on caminitotest's own systemd user manager and bus, so the systemd
# export and the journal are the real ones (14). It is neither a Session nor a nested run
# (CONTEXT.md), and not the login on a tty of the ticket (72): there is one output, no input
# device, no logind session and no VT.
#
#   sudo xvfb-login.sh [CAMINITO]
#
# With CAMINITO, a caminito binary, that one runs in place of the installed one, with the
# installed helpers (74).
#
# It makes the user if there is none, starts mate-session under Caminito, waits for it, prints
# what the ticket's criteria ask about, logs out through mate-session, and prints what is
# left. The files it makes, the screenshots among them, are in /home/caminitotest/72-xvfb-login.
# It leaves the user, with lingering off again, and a home directory others can enter.
set -uo pipefail

[ "$(id -u)" -eq 0 ] || { echo "xvfb-login: run it as root" >&2; exit 1; }
here=$(cd "$(dirname "$0")" && pwd)
user=caminitotest
x_display=:72

id $user >/dev/null 2>&1 || useradd -m $user
uid=$(id -u $user)
work=/home/$user/72-xvfb-login
rm -rf $work
install -d -o $user -g $user $work
install -o $user -g $user -m 755 "$here/gtk-probe.py" "$here/xwd-to-png.py" $work/

caminito_bin=caminito
if [ $# -gt 0 ]; then
  install -o $user -g $user -m 755 "$1" $work/caminito
  caminito_bin=$work/caminito
fi

# Waits up to $1 seconds for the rest of its arguments to succeed.
wait_for() {
  local tries=$(($1 * 5)); shift
  for _ in $(seq $tries); do "$@" >/dev/null 2>&1 && return 0; sleep 0.2; done
  return 1
}

# The user's systemd manager and bus, as a login gives. A manager left from an earlier run
# would still have that run's variables (74), so it starts anew.
loginctl disable-linger $user 2>/dev/null
wait_for 10 test ! -e /run/user/$uid
loginctl enable-linger $user
wait_for 10 test -S /run/user/$uid/bus || { echo "xvfb-login: no user bus" >&2; exit 1; }

# Runs a command as the user with a login's environment and nothing of the caller's.
as_user() {
  runuser -u $user -- env -i HOME=/home/$user USER=$user LOGNAME=$user SHELL=/bin/bash \
    PATH=/usr/local/bin:/usr/bin LANG=C.UTF-8 XDG_RUNTIME_DIR=/run/user/$uid \
    DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$uid/bus "$@"
}
# The same, with the displays that Caminito exported to systemd.
in_session() {
  as_user env $(as_user systemctl --user show-environment | grep -E '^(WAYLAND_DISPLAY|DISPLAY)=') "$@"
}
session_environment() {
  as_user systemctl --user show-environment |
    grep -E '^(WAYLAND_DISPLAY|DISPLAY|MATE_WM_NAME|XDG_SESSION_TYPE|XDG_CURRENT_DESKTOP|XCURSOR_[A-Z]+)=' | sort
}
dump_state() {
  in_session gdbus call --session --dest org.mate.Caminito.Debug --object-path /org/mate/Caminito/Debug \
    --method org.mate.Caminito.Debug.DumpState
}
screenshot() {
  as_user xwd -root -silent -display $x_display -out $work/$1.xwd &&
    as_user $work/xwd-to-png.py $work/$1.xwd $work/$1.png && rm $work/$1.xwd
}

echo "== the systemd environment before"
session_environment

as_user Xvfb $x_display -screen 0 1280x800x24 >/dev/null 2>&1 &
xvfb=$!
session=
trap 'kill $session $xvfb 2>/dev/null; pkill -u $user -x caminito; loginctl disable-linger $user' EXIT
wait_for 10 as_user xdpyinfo -display $x_display || { echo "xvfb-login: no Xvfb" >&2; exit 1; }

since=$(date '+%Y-%m-%d %H:%M:%S')
# stderr is a file, as it is from a display manager, so the logs go to the journal.
as_user env DISPLAY=$x_display $caminito_bin --session </dev/null >$work/caminito.stderr 2>&1 &
session=$!

exported() { session_environment | grep -q '^WAYLAND_DISPLAY='; }
wait_for 30 exported || { echo "xvfb-login: Caminito exported nothing in 30 s" >&2; exit 1; }
# mate-session's phases, and the panel, caja and msd on screen. Nothing says when it's done.
sleep 25
caminito=$(pgrep -u $user -x caminito)

echo "== the systemd environment in the session"
session_environment

echo "== the session's processes"
pgrep -u $user -a | grep -vE 'systemd|\(sd-pam\)|Xvfb|dbus-(daemon|broker)|pgrep'
# The kernel keeps 15 characters of a name, and pgrep matches no pattern longer than that.
echo "mate-screensaver running: $(pgrep -u $user -x mate-screensave >/dev/null && echo yes || echo no)"

echo "== the screensaver names (caminito is pid $caminito)"
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
# What the window list shows on a workspace with no windows (66).
if command -v wmctrl >/dev/null; then
  in_session wmctrl -s 1
  sleep 2
  screenshot workspace-2
  in_session wmctrl -s 0
fi
sleep 6

echo "== the journal: lines for each identifier"
journalctl _UID=$uid --since "$since" -o json --no-pager |
  grep -o '"SYSLOG_IDENTIFIER":"[^"]*"' | sort | uniq -c
journalctl _UID=$uid --since "$since" --no-pager -o short-precise >$work/journal.txt
echo "== journalctl --user, as the user, as the README says"
for tag in caminito Xwayland caminito-gtk-palette; do
  echo "$tag: $(as_user journalctl --user -t $tag --since "$since" --no-pager -o cat 2>&1 | wc -l) lines"
done
echo "== the journal: caminito's warnings and errors"
journalctl _UID=$uid -t caminito --since "$since" --no-pager -p warning -o cat | sort | uniq -c | head -30

echo "== logout"
in_session mate-session-save --logout
gone() { ! kill -0 $session; }
if ! wait_for 30 gone; then
  echo "caminito is still running after 30 s"
  pkill -u $user -x caminito
fi
wait $session
echo "caminito status=$?"
sleep 1
echo "== the systemd environment after"
session_environment
echo "== what is left of the session"
pgrep -u $user -a | grep -vE 'systemd|\(sd-pam\)|Xvfb|dbus-(daemon|broker)|pgrep'
journalctl _UID=$uid --since "$since" --no-pager -o short-precise >$work/journal.txt
echo "== time-outs, in the journal and in what mate-session and its children wrote to stderr"
grep -iE 'timed out|time-?out' $work/journal.txt $work/caminito.stderr | head
echo "== the journal: caminito's last lines"
journalctl _UID=$uid -t caminito --since "$since" --no-pager -o cat | tail -15

# So the developer's user can read the results.
chmod a+x /home/$user
chmod -R a+rX $work
