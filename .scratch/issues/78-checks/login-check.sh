#!/bin/bash
# The criteria of the X11-only autostarts (78) on a machine with no tty: the login on Xvfb
# (72) as caminitotest, then the programs that dumped core since it began, and who has the
# screensaver's name. It is not the login on a tty.
#
# In an OrbStack machine systemd-coredump gets nothing ("core_pattern handlers not supported
# in namespaces"), so coredumpctl is always empty there: the kernel's own "coredump:" lines
# are what shows a crash.
#
# With no seat, logind refuses caminitotest the inhibitor locks that mate-power-manager takes
# as it starts, and it aborts on that. So for the run a polkit rule allows them.
#
#   sudo login-check.sh
#
# It leaves xvfb-login.log, the whole of what the login printed, beside itself.
set -uo pipefail

[ "$(id -u)" -eq 0 ] || { echo "login-check: run it as root" >&2; exit 1; }
here=$(cd "$(dirname "$0")" && pwd)
user=caminitotest
# Where xvfb-login.sh leaves its files.
work=/home/$user/72-xvfb-login
three='mate-(screensaver|power-manager|maximus)'
since=$(date '+%Y-%m-%d %H:%M:%S')
pacman -Q mate-session-manager-caminito mate-screensaver mate-power-manager mate-netbook mate-utils

rule=/etc/polkit-1/rules.d/00-78-caminitotest-inhibit.rules
trap 'rm -f $rule' EXIT
cat >$rule <<RULE || { echo "login-check: no polkit rule" >&2; exit 1; }
polkit.addRule(function(action, subject) {
  if (action.id.indexOf("org.freedesktop.login1.inhibit-") == 0 && subject.user == "$user")
    return polkit.Result.YES;
});
RULE

# mate-power-manager's environment, read while the login runs. The first second is for
# xvfb-login.sh to end what an earlier run left.
power_manager_environment=$(mktemp)
(
  sleep 1
  for _ in $(seq 300); do
    pid=$(pgrep -u $user -o -x mate-power-mana) && sleep 1 &&
      { tr '\0' '\n' <"/proc/$pid/environ" | grep -E '^(GDK_BACKEND|DISPLAY|WAYLAND_DISPLAY)=' | sort; break; }
    sleep 0.2
  done >"$power_manager_environment" 2>&1
) &
"$here/../72-checks/xvfb-login.sh" >"$here/xvfb-login.log" 2>&1
login_status=$?
wait
[ $login_status -eq 0 ] && grep -q '^== the screensaver names' "$here/xvfb-login.log" ||
  { echo "login-check: the login failed (status $login_status), see $here/xvfb-login.log" >&2; exit 1; }

sed -n '/^== the screensaver names/,/^== the desktop/p' "$here/xvfb-login.log" | grep -v '^== the desktop'
echo "== the three, in the processes 25 s after the start"
sed -n '/^== the session.s processes/,/^== the screensaver names/p' "$here/xvfb-login.log" |
  grep -E "^[0-9]+ .*$three" || echo none
echo "== mate-power-manager's environment"
cat "$power_manager_environment"
rm -f "$power_manager_environment"
echo "== core dumps since $since, of any program"
coredumpctl list --since "$since" --no-pager 2>&1
# Two lines for each crash: the second, "format_corename failed, aborting core", is left out.
journalctl -k --since "$since" --no-pager -o cat | grep -E '^coredump: [0-9]+\(' | grep -v 'format_corename failed' ||
  echo "no coredump: line from the kernel"
echo "== the three's own errors"
grep -hE "^\(?$three|PowerManager" $work/caminito.stderr | head
