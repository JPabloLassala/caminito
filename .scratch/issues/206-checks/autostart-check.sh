#!/bin/bash
# The autostart lists of mate-session-manager-caminito's 0003 and 0004 (78, 206), in a real
# mate-session: the pinned commit with the carried series applied and built, then started twice
# on Xvfb under a private session bus, with MATE_WM_NAME=Caminito and without it.
#
# Each id of the two lists is an autostart entry here, beside a control entry no list names.
# plank.desktop is the dock required component instead, as mate-tweak's dock layouts set it.
# Every entry records the GDK_BACKEND it was started with. The run starts with GDK_BACKEND=x11,
# so under Caminito the control shows 0001's unset, an entry of 0004 shows x11 again, and an
# entry of 0003 doesn't start. Without the variable all of them start with x11.
#
#   autostart-check.sh [work directory]
#
# It needs what the PKGBUILD's makedepends and depends name, plus xorg-server-xvfb and
# mate-desktop's schemas. The work directory keeps the clone and the build; it defaults to
# a new one under /tmp.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
patches=$repo/packaging/patches/mate-session-manager
commit=$(sed -n 's/^_commit=//p' "$repo/packaging/arch/mate-session-manager-caminito/PKGBUILD")
work=${1:-$(mktemp -d /tmp/206-autostart.XXXXXX)}
mkdir -p "$work"
work=$(cd "$work" && pwd)

skipped=(mate-screensaver.desktop mate-maximus-autostart.desktop mate-hud.desktop
  dock-replace.desktop picom.desktop magnus-autostart.desktop plank.desktop)
on_x11=(mate-power-manager.desktop tilda.desktop)

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
    >"$work/build.log" 2>&1 || { tail -30 "$work/build.log"; echo "autostart-check: the build failed" >&2; exit 1; }
fi

# The session's directories, every one of them below the work directory.
root=$work/root
rm -rf "$root"
mkdir -p "$root"/{xdg/autostart,data/applications,config,schemas}
cat >"$root/record" <<'RECORD'
#!/bin/sh
echo "$1 ${GDK_BACKEND-unset}" >>"$CHECK_STARTED"
RECORD
chmod +x "$root/record"
entry() {
  printf '[Desktop Entry]\nType=Application\nName=%s\nExec=%s %s\n' "$1" "$root/record" "$1" >"$2/$1"
}
for id in control.desktop "${skipped[@]}" "${on_x11[@]}"; do
  [ "$id" = plank.desktop ] && entry "$id" "$root/data/applications" || entry "$id" "$root/xdg/autostart"
done

# The system's schemas, the built tree's org.mate.session, and plank as the only required
# component.
cp /usr/share/glib-2.0/schemas/*.gschema.xml "$root/schemas/"
cp "$src/data/org.mate.session.gschema.xml" "$root/schemas/"
cat >"$root/schemas/99-206.gschema.override" <<'OVERRIDE'
[org.mate.session]
required-components-list=['dock']

[org.mate.session.required-components]
dock='plank'
OVERRIDE
glib-compile-schemas "$root/schemas"

cat >"$root/session.sh" <<SESSION
#!/bin/sh
"$src/mate-session/mate-session" --debug >"\$CHECK_STARTED.log" 2>&1 &
pid=\$!
for _ in \$(seq 100); do grep -q '^control.desktop ' "\$CHECK_STARTED" 2>/dev/null && break; sleep 0.2; done
sleep 3
kill \$pid
wait \$pid 2>/dev/null
exit 0
SESSION
chmod +x "$root/session.sh"

# One session: $1 names it, and the rest is its environment.
session() {
  local name=$1
  shift
  rm -f "$root/started-$name"
  env -i PATH="$PATH" HOME="$root" XDG_CONFIG_HOME="$root/config" XDG_CONFIG_DIRS="$root/xdg" \
    XDG_DATA_DIRS="$root/data" GSETTINGS_SCHEMA_DIR="$root/schemas" GSETTINGS_BACKEND=memory \
    XDG_CURRENT_DESKTOP=MATE GDK_BACKEND=x11 CHECK_STARTED="$root/started-$name" "$@" \
    xvfb-run -a dbus-run-session -- "$root/session.sh" >"$root/started-$name.bus.log" 2>&1
  touch "$root/started-$name"
}

failed=0
expect() {
  local name=$1 id=$2 want=$3 got
  got=$(awk -v id="$id" '$1 == id { print $2 }' "$root/started-$name")
  [ -n "$got" ] || got="not started"
  if [ "$got" = "$want" ]; then
    printf '%-32s %s\n' "$id" "$got"
  else
    printf '%-32s %s, expected %s\n' "$id" "$got" "$want"
    failed=1
  fi
}

echo "== under Caminito"
session caminito MATE_WM_NAME=Caminito
expect caminito control.desktop unset
for id in "${skipped[@]}"; do expect caminito "$id" "not started"; done
for id in "${on_x11[@]}"; do expect caminito "$id" x11; done

echo "== without MATE_WM_NAME"
session stock
for id in control.desktop "${skipped[@]}" "${on_x11[@]}"; do expect stock "$id" x11; done

if [ $failed -ne 0 ]; then
  echo "autostart-check: the session's logs are $root/started-*.log" >&2
  exit 1
fi
echo "autostart-check: passed"
