#!/bin/bash
# mate-session-manager-caminito's 0003 skips nm-applet under Caminito (CMN-195, ADR 0041), in a
# real mate-session: the pinned commit with the carried series applied and built, then started
# twice under a private session bus, with MATE_WM_NAME=Caminito and without it.
#
# nm-applet.desktop is an entry of the system's autostart directory, and a user's copy of it is
# in the user's, as when someone edited it in the Startup Applications. The user's copy records
# a name of its own, so the stock run shows it's the copy mate-session reads. Under Caminito
# neither starts and the log names the entry as disabled; without the variable the user's copy
# starts. A control entry starts in both.
#
#   nm-applet-check.sh [work directory]
#
# It needs what the PKGBUILD's makedepends and depends name, mate-desktop's schemas, and an X
# server: Xvfb if it's installed, else a rootful Xwayland on $WAYLAND_DISPLAY, a small window
# for the length of each run. The work directory keeps the clone and the build; it
# defaults to a new one under /tmp.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
patches=$repo/packaging/patches/mate-session-manager
commit=$(sed -n 's/^_commit=//p' "$repo/packaging/arch/mate-session-manager-caminito/PKGBUILD")
work=${1:-$(mktemp -d /tmp/cmn-195.XXXXXX)}
mkdir -p "$work"
work=$(cd "$work" && pwd)

echo "== the series on $commit"
src=$work/mate-session-manager
if [ ! -x "$src/mate-session/mate-session" ]; then
  rm -rf "$src"
  git clone -q https://github.com/mate-desktop/mate-session-manager.git "$src"
  git -C "$src" checkout -q "$commit"
  git -C "$src" submodule -q update --init
  for patch in "$patches"/*.patch; do
    echo "${patch##*/}"
    patch -d "$src" -Np1 -s -i "$patch"
  done
  (cd "$src" && autoreconf -fi && ./configure --prefix=/usr --sysconfdir=/etc && make) \
    >"$work/build.log" 2>&1 || { tail -30 "$work/build.log"; echo "nm-applet-check: the build failed" >&2; exit 1; }
fi

# The session's directories, every one of them below the work directory.
root=$work/root
rm -rf "$root"
mkdir -p "$root"/{xdg/autostart,data/applications,config/autostart,schemas}
cat >"$root/record" <<'RECORD'
#!/bin/sh
echo "$1" >>"$CHECK_STARTED"
RECORD
chmod +x "$root/record"
# entry <file> <directory> <the name it records>
entry() {
  printf '[Desktop Entry]\nType=Application\nName=%s\nExec=%s %s\n' "$3" "$root/record" "$3" >"$2/$1"
}
entry control.desktop "$root/xdg/autostart" control
entry nm-applet.desktop "$root/xdg/autostart" system-nm-applet
entry nm-applet.desktop "$root/config/autostart" user-nm-applet

# The system's schemas, the built tree's org.mate.session, and no required component.
cp /usr/share/glib-2.0/schemas/*.xml "$root/schemas/"
cp "$src/data/org.mate.session.gschema.xml" "$root/schemas/"
cat >"$root/schemas/99-cmn-195.gschema.override" <<'OVERRIDE'
[org.mate.session]
required-components-list=[]
OVERRIDE
glib-compile-schemas "$root/schemas"

cat >"$root/session.sh" <<SESSION
#!/bin/sh
"$src/mate-session/mate-session" --debug >"\$CHECK_STARTED.log" 2>&1 &
pid=\$!
for _ in \$(seq 100); do grep -qx control "\$CHECK_STARTED" 2>/dev/null && break; sleep 0.2; done
sleep 3
kill \$pid
wait \$pid 2>/dev/null
exit 0
SESSION
chmod +x "$root/session.sh"

# An X server of the session's own: Xvfb if it's installed, else a rootful Xwayland.
start_x() {
  local server=(Xvfb)
  command -v Xvfb >/dev/null || server=(Xwayland -geometry 320x200)
  rm -f "$root/display"
  "${server[@]}" -displayfd 3 3>"$root/display" 2>"$root/x.log" &
  x_pid=$!
  for _ in $(seq 50); do [ -s "$root/display" ] && break; sleep 0.1; done
  [ -s "$root/display" ] || { cat "$root/x.log"; echo "nm-applet-check: no X server" >&2; exit 1; }
  x_display=:$(cat "$root/display")
}
stop_x() {
  kill "$x_pid"
  wait "$x_pid" 2>/dev/null || true
}

# One session: $1 names it, and the rest is its environment.
session() {
  local name=$1
  shift
  : >"$root/started-$name"
  start_x
  env -i PATH="$PATH" HOME="$root" XDG_CONFIG_HOME="$root/config" XDG_CONFIG_DIRS="$root/xdg" \
    XDG_DATA_DIRS="$root/data" GSETTINGS_SCHEMA_DIR="$root/schemas" GSETTINGS_BACKEND=memory \
    XDG_CURRENT_DESKTOP=MATE G_MESSAGES_DEBUG=all DISPLAY="$x_display" CHECK_STARTED="$root/started-$name" "$@" \
    dbus-run-session -- "$root/session.sh" >"$root/started-$name.bus.log" 2>&1
  stop_x
}

failed=0
check() {
  if "${@:2}"; then
    printf '%-40s ok\n' "$1"
  else
    printf '%-40s FAILED\n' "$1"
    failed=1
  fi
}
started() { grep -qx "$2" "$root/started-$1"; }
not_started() { ! started "$@"; }

echo "== under Caminito"
session caminito MATE_WM_NAME=Caminito
check "control started" started caminito control
check "the system's nm-applet not started" not_started caminito system-nm-applet
check "the user's nm-applet not started" not_started caminito user-nm-applet
check "the log names nm-applet as disabled" \
  grep -q 'nm-applet.desktop is disabled under Caminito' "$root/started-caminito.log"

echo "== without MATE_WM_NAME"
session stock
check "control started" started stock control
check "the user's nm-applet started" started stock user-nm-applet
check "the system's nm-applet not started" not_started stock system-nm-applet

if [ $failed -ne 0 ]; then
  echo "nm-applet-check: the session's logs are $root/started-*.log" >&2
  exit 1
fi
echo "nm-applet-check: passed"
