#!/bin/bash
# The patched workspace switcher in a nested Caminito on Xvfb (227). The nested backend has
# one output, so this shows only that the switcher finds its panel's output's group, lists
# that group's workspaces and switches them, with workspaces-per-output on and off. Two
# monitors need a Session (229). Run it in the OrbStack machine, with mate-panel-caminito
# installed and CAMINITO naming a caminito binary:
#   CAMINITO=$HOME/target-227/debug/caminito .scratch/issues/227-checks/nested-pager.sh
# Where the package is built and can't be installed, PKG names the directory its
# .pkg.tar.zst was unpacked into, and the run is in a mount namespace of its own with the
# package's panel over the installed one.
# gdb stops the panel where it lists a group's workspaces, which nothing but the patched
# switcher does, and prints who asked.
# The sleeps wait for Xvfb, the panel and gdb, which say nothing when ready.
if [ "$PKG" ] && [ -z "$IN_NAMESPACE" ]; then
    exec unshare -rm env IN_NAMESPACE=1 "$0" "$@"
fi
if [ "$PKG" ]; then
    for path in bin/mate-panel lib/mate-panel share/mate-panel lib/libmate-panel-applet-4.so.2.0.0; do
        mount --bind "$PKG/usr/$path" "/usr/$path" || exit 1
    done
fi
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d "$HOME/t227.XXXXXX")
mkdir -p "$work/config" "$work/rt" "$work/schemas"; chmod 700 "$work/rt"
cp "$root/data/org.mate.Caminito.gschema.xml" "$work/schemas/"
[ "$PKG" ] && cp "$PKG"/usr/share/glib-2.0/schemas/*.xml "$work/schemas/"
glib-compile-schemas "$work/schemas"
export XDG_RUNTIME_DIR=$work/rt XDG_CONFIG_HOME=$work/config GSETTINGS_SCHEMA_DIR=$work/schemas
export LANG=C LANGUAGE=C
# DISPLAY too: the accessibility bus takes its address from it.
unset DISPLAY WAYLAND_DISPLAY GTK_MODULES NO_AT_BRIDGE
Xvfb :227 -screen 0 1920x1200x24 >"$work/xvfb.log" 2>&1 &
sleep 1
inner() {
    active() {
        busctl --user call org.mate.Caminito.Debug /org/mate/Caminito/Debug org.mate.Caminito.Debug DumpState --json=short |
            python3 -c "import json,sys; print('Caminito\'s active workspace:', json.loads(json.loads(sys.stdin.read())['data'][0])['active_workspace'] + 1)"
    }
    look() {
        echo "buttons: $(python3 "$here/pager.py" buttons 2>/dev/null)"
        active
    }
    DISPLAY=:227 "${CAMINITO:-caminito}" 2>"$work/caminito.log" &
    for _ in $(seq 100); do grep -q "listening on WAYLAND_DISPLAY=" "$work/caminito.log" && break; sleep 0.1; done
    line=$(grep -o "listening on WAYLAND_DISPLAY=[^ ]*" "$work/caminito.log" | head -1)
    export WAYLAND_DISPLAY=${line##*=} GDK_BACKEND=wayland
    for _ in $(seq 50); do busctl --user list 2>/dev/null | grep -q "^org.mate.Caminito " && break; sleep 0.1; done
    # The private bus has no systemd to start the accessibility registry with.
    /usr/lib/at-spi2-registryd >/dev/null 2>&1 &
    sleep 1
    WAYLAND_DEBUG=1 mate-panel >"$work/panel.log" 2>&1 & panel=$!
    sleep 8
    for key in true false; do
        gsettings set org.mate.Caminito workspaces-per-output $key
        sleep 2
        echo "== workspaces-per-output $key"
        echo "groups the panel was told of: $(grep -c "ext_workspace_manager_v1#[0-9]*\.workspace_group(" "$work/panel.log")"
        look
        python3 "$here/pager.py" click 3 2>/dev/null; sleep 1; look
        timeout 12 gdb -p $panel -batch -ex "break xfw_workspace_group_list_workspaces" -ex continue -ex "bt 3" \
            >"$work/gdb-$key.log" 2>&1 &
        sleep 6
        # A fifth workspace makes the switcher build its buttons again.
        gsettings set org.mate.Marco.general num-workspaces 5; sleep 3
        echo "who lists the group's workspaces: $(grep -m2 "^#[01] " "$work/gdb-$key.log" | sed 's/^#[01] *0x[0-9a-f]* in //' | tr '\n' ' ')"
        wait %gdb 2>/dev/null
        look
        python3 "$here/pager.py" click 1 2>/dev/null; sleep 1; look
        gsettings set org.mate.Marco.general num-workspaces 4; sleep 2
    done
    echo "--- panel alive: $(kill -0 $panel 2>/dev/null && echo yes || echo no)"
    echo "--- the panel's warnings"; grep -i "warning\|critical" "$work/panel.log" | grep -v "^\[" | head
    kill $(jobs -p) 2>/dev/null
}
export -f inner; export work here
dbus-run-session -- bash -c inner
kill $(jobs -p) 2>/dev/null
# gvfs and the document portal mount under the runtime directory.
fusermount3 -u "$work/rt/gvfs" 2>/dev/null; fusermount3 -u "$work/rt/doc" 2>/dev/null
[ "$KEEP" ] && echo "kept $work" || rm -rf "$work"
