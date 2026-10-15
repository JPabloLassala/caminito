#!/bin/bash
# The Night Light tab's check (CMN-115, ticket 142): caminito-displays on a nested Caminito on
# Xvfb, on a private session bus and dconf, driven by drive.py through AT-SPI. Run it in the
# OrbStack machine with the two built binaries:
#   T=$HOME/target-cmn-115/debug CAMINITO=$T/caminito DISPLAYS=$T/caminito-displays \
#     .scratch/checks/CMN-115/nested.sh
# It needs Xvfb, dbus, dconf, at-spi2-core and python-gobject. SHOT names a file for a
# picture of the screen at the end. The sleeps wait for Xvfb and the registry, which say
# nothing when ready.
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d "$HOME/cmn-115.XXXXXX")
mkdir -p "$work/config" "$work/rt" "$work/schemas"; chmod 700 "$work/rt"
cp "$root/data/org.mate.Caminito.gschema.xml" "$work/schemas/"
glib-compile-schemas "$work/schemas"
export XDG_RUNTIME_DIR=$work/rt XDG_CONFIG_HOME=$work/config GSETTINGS_SCHEMA_DIR=$work/schemas
export LANG=C LANGUAGE=C
# DISPLAY too: the accessibility bus takes its address from it.
unset DISPLAY WAYLAND_DISPLAY GTK_MODULES NO_AT_BRIDGE GTK_A11Y
Xvfb :142 -screen 0 1280x900x24 >"$work/xvfb.log" 2>&1 &
sleep 1
inner() {
    cd "$work" || exit 1
    DISPLAY=:142 "$CAMINITO" 2>"$work/caminito.log" &
    for _ in $(seq 100); do grep -q "listening on WAYLAND_DISPLAY=" "$work/caminito.log" && break; sleep 0.1; done
    line=$(grep -o "listening on WAYLAND_DISPLAY=[^ ]*" "$work/caminito.log" | head -1)
    export WAYLAND_DISPLAY=${line##*=} GDK_BACKEND=wayland
    for _ in $(seq 50); do busctl --user list 2>/dev/null | grep -q "^org.mate.Caminito " && break; sleep 0.1; done
    # The private bus has no systemd to start the accessibility registry with.
    /usr/lib/at-spi2-registryd >/dev/null 2>&1 &
    sleep 1
    python3 "$here/drive.py" "$DISPLAYS" 2>"$work/drive.log"
    status=$?
    [ $status = 0 ] || cat "$work/drive.log" >&3
    kill $(jobs -p) 2>/dev/null
    return $status
}
export -f inner; export work here
dbus-run-session -- bash -c inner 3>&2 2>/dev/null
status=$?
kill $(jobs -p) 2>/dev/null
# The document portal mounts under the runtime directory.
fusermount3 -u "$work/rt/doc" 2>/dev/null
[ "$KEEP" ] && echo "kept $work" || rm -rf "$work"
exit $status
