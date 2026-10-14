#!/bin/bash
# The scroll wheel over the patched workspace switcher, after a switch to a lower workspace
# (240), in a nested Caminito on Xvfb. A click on the third button and then on the first
# leaves the first workspace active, and one notch down must then go to the second, and not
# to the fourth. The buttons are 227's check's (227-checks/nested-pager.sh), which this
# is cut from: run it the same way, with CAMINITO and PKG.
#   CAMINITO=$HOME/target-226/debug/caminito PKG=$HOME/t240/pkg .scratch/issues/240-checks/nested-scroll.sh
# The wheel is xdotool's, on the outer display. The switcher is taken to be in the bottom
# panel, where MATE's default layout has it. SHOT names a file for a picture of the screen.
# The sleeps wait for Xvfb and the panel, which say nothing when ready.
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
pager=$root/.scratch/issues/227-checks/pager.py
work=$(mktemp -d "$HOME/t240.XXXXXX")
mkdir -p "$work/config" "$work/rt" "$work/schemas"; chmod 700 "$work/rt"
cp "$root/data/org.mate.Caminito.gschema.xml" "$work/schemas/"
[ "$PKG" ] && cp "$PKG"/usr/share/glib-2.0/schemas/*.xml "$work/schemas/"
glib-compile-schemas "$work/schemas"
export XDG_RUNTIME_DIR=$work/rt XDG_CONFIG_HOME=$work/config GSETTINGS_SCHEMA_DIR=$work/schemas
export LANG=C LANGUAGE=C
# DISPLAY too: the accessibility bus takes its address from it.
unset DISPLAY WAYLAND_DISPLAY GTK_MODULES NO_AT_BRIDGE
Xvfb :240 -screen 0 1920x1200x24 >"$work/xvfb.log" 2>&1 &
sleep 1
inner() {
    look() {
        echo "buttons: $(python3 "$pager" buttons 2>/dev/null)"
        busctl --user call org.mate.Caminito.Debug /org/mate/Caminito/Debug org.mate.Caminito.Debug DumpState --json=short |
            python3 -c "import json,sys; print('Caminito\'s active workspace:', json.loads(json.loads(sys.stdin.read())['data'][0])['active_workspace'] + 1)"
    }
    DISPLAY=:240 "${CAMINITO:-caminito}" 2>"$work/caminito.log" &
    for _ in $(seq 100); do grep -q "listening on WAYLAND_DISPLAY=" "$work/caminito.log" && break; sleep 0.1; done
    line=$(grep -o "listening on WAYLAND_DISPLAY=[^ ]*" "$work/caminito.log" | head -1)
    export WAYLAND_DISPLAY=${line##*=} GDK_BACKEND=wayland
    for _ in $(seq 50); do busctl --user list 2>/dev/null | grep -q "^org.mate.Caminito " && break; sleep 0.1; done
    # The private bus has no systemd to start the accessibility registry with.
    /usr/lib/at-spi2-registryd >/dev/null 2>&1 &
    sleep 1
    mate-panel >"$work/panel.log" 2>&1 &
    sleep 8
    python3 "$pager" click 3 2>/dev/null; sleep 1; look
    python3 "$pager" click 1 2>/dev/null; sleep 1; look
    # Caminito's window on the outer display, and the button in the bottom panel in it.
    window=$(DISPLAY=:240 xdotool search --onlyvisible --name . | head -1)
    eval "$(DISPLAY=:240 xdotool getwindowgeometry --shell "$window")"
    read -r bx by _ panel_height < <(python3 "$pager" where 1 2>/dev/null)
    x=$((X + bx)); y=$((Y + HEIGHT - panel_height + by))
    echo "the wheel goes down one notch at $x,$y, in a window of ${WIDTH}x$HEIGHT at $X,$Y"
    DISPLAY=:240 xdotool mousemove "$x" "$y"; sleep 0.5
    [ "$SHOT" ] && DISPLAY=:240 import -window root "$SHOT"
    DISPLAY=:240 xdotool click 5; sleep 1; look
    kill $(jobs -p) 2>/dev/null
}
export -f inner; export work pager
dbus-run-session -- bash -c inner 2>/dev/null
kill $(jobs -p) 2>/dev/null
# gvfs and the document portal mount under the runtime directory.
fusermount3 -u "$work/rt/gvfs" 2>/dev/null; fusermount3 -u "$work/rt/doc" 2>/dev/null
[ "$KEEP" ] && echo "kept $work" || rm -rf "$work"
