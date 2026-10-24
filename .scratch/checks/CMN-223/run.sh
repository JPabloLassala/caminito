#!/usr/bin/env bash
# CMN-223's check of the floating mate-panel (carried patch 0015): the patched mate-panel
# under a nested Caminito on Xvfb, with a bottom panel and a left panel that aren't expanded.
# The Dock applet is CMN-219's and isn't built yet, so the bottom panel has the window list,
# the applet it replaces, and a Clock; the left panel has a Clock. Each case is a screenshot
# and the work area Caminito reports, with a maximised window open for it:
#
#   01-floating       both panels floating: they stand off their edges, with rounded
#                     corners, the bottom one centred
#   02-not-floating   the same panels with Floating off, set while they run
#   03-floating-expanded
#                     the bottom panel expanded, floating: it stands off the bottom edge and
#                     reaches both sides, with rounded corners
#   04-no-caminito    the panels with Floating on, run by a mate-panel that has no
#                     org.mate.Caminito to find on its D-Bus session: nothing changes
#
# report.txt has, for each case, the work area, each panel's geometry, exclusive zone and gaps
# to the output's edges, and the maximised window's geometry, as Caminito's DumpState has them.
# The work area ends at the panel's inner edge in 01, 8 pixels further in than in 02: the
# compositor counts a layer surface's margin in its exclusive zone, so the patch leaves the
# zone at the panel's size.
#
# 05-properties.png isn't made here: it's the bottom panel's Properties under Caminito, with
# Floating below Autohide, taken once with a throwaway call of panel_properties_dialog_present
# in the build, which isn't in the patch.
#
#   run.sh [<mate-panel's install prefix>] [<caminito>] [<output directory>]
#
# The prefix is a build of mate-panel with packaging/patches/mate-panel/ applied, with
# -Din-process-applets=all (a Wayland panel has no out-of-process applets), installed with
# `meson install` (default /tmp/mp/prefix); the compositor defaults to the debug build of this
# checkout. XFW_LIB names a directory of libxfce4windowing's libraries, where the system hasn't
# them. It needs Xvfb, python-gobject with GTK 3, dbus-run-session, busctl,
# gsettings, glib-compile-schemas and the libraries mate-panel is built against.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
pkg=$(realpath "${1:-/tmp/mp/prefix}")
caminito=$(realpath "${2:-${CARGO_TARGET_DIR:-$HOME/target}/debug/caminito}")
out=$(realpath -m "${3:-$here/out}")
work=$(mktemp -d "${TMPDIR:-/tmp}/cmn-223.XXXXXX")
trap 'kill $(jobs -p) 2>/dev/null; [ "${KEEP:-}" ] && echo "kept $work" || rm -rf "${work:?}"' EXIT
mkdir -p "$work/config" "$work/rt" "$work/schemas" "$out"
chmod 700 "$work/rt"
rm -f "${out:?}"/0[1-4]-*.png "${out:?}/report.txt"
# Marco's schemas and the rest that Caminito reads, vendored for its tests.
cp "$root"/tests/schemas/*.xml "$pkg"/share/glib-2.0/schemas/*.xml "$work/schemas/"
glib-compile-schemas "$work/schemas"

export XDG_RUNTIME_DIR=$work/rt XDG_CONFIG_HOME=$work/config GSETTINGS_SCHEMA_DIR=$work/schemas
export XDG_DATA_DIRS=$pkg/share:/usr/local/share:/usr/share
export LD_LIBRARY_PATH=$pkg/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}${XFW_LIB:+:$XFW_LIB}
export MATE_PANEL_APPLETS_DIR=$pkg/share/mate-panel/applets
export LANG=C LANGUAGE=C GDK_BACKEND=x11 NO_AT_BRIDGE=1
unset WAYLAND_DISPLAY GTK_MODULES

display=:223
Xvfb "$display" -screen 0 1280x800x24 >"$work/xvfb.log" 2>&1 &
sleep 1
export DISPLAY=$display

inner() {
    report=$out/report.txt
    gs() { gsettings set "$@"; }
    toplevel() { echo "org.mate.panel.toplevel:/org/mate/panel/toplevels/$1/"; }
    object() { echo "org.mate.panel.object:/org/mate/panel/objects/$1/"; }
    # A panel that isn't expanded, centred along its edge.
    panel() {
        local id=$1 orientation=$2
        gs "$(toplevel "$id")" orientation "$orientation"
        gs "$(toplevel "$id")" size 56
        gs "$(toplevel "$id")" expand false
        if [ "$orientation" = bottom ]; then
            gs "$(toplevel "$id")" x-centered true
            gs "$(toplevel "$id")" y-bottom 0
        else
            gs "$(toplevel "$id")" y-centered true
            gs "$(toplevel "$id")" x 0
        fi
        gs "$(toplevel "$id")" floating "$3"
    }
    applet() { # id toplevel iid index
        gs "$(object "$1")" object-type applet
        gs "$(object "$1")" applet-iid "$3"
        gs "$(object "$1")" toplevel-id "$2"
        gs "$(object "$1")" pack-index "$4"
    }
    configure() { # floating
        panel bottom bottom "$1"
        panel left left "$1"
        applet window-list bottom WnckletFactory::WindowListApplet 0
        applet bottom-clock bottom ClockAppletFactory::ClockApplet 1
        applet left-clock left ClockAppletFactory::ClockApplet 0
        gs org.mate.panel toplevel-id-list "['bottom', 'left']"
        gs org.mate.panel object-id-list "['window-list', 'bottom-clock', 'left-clock']"
    }
    state() { # label
        busctl --user call org.mate.Caminito.Debug /org/mate/Caminito/Debug \
            org.mate.Caminito.Debug DumpState --json=short |
            python3 -I -c '
import json, sys
dump = json.loads(json.loads(sys.stdin.read())["data"][0])
open(sys.argv[1], "w").write(json.dumps(dump, indent=1))
' "$work/dump-$1.json"
        python3 -I "$here/report.py" "$1" "$work/dump-$1.json" >>"$report"
    }

    "$caminito" 2>"$work/caminito.log" &
    for _ in $(seq 150); do grep -q "listening on WAYLAND_DISPLAY=" "$work/caminito.log" && break; sleep 0.1; done
    line=$(grep -o "listening on WAYLAND_DISPLAY=[^ ]*" "$work/caminito.log" | head -1)
    caminito_display=${line##*=}
    for _ in $(seq 50); do busctl --user list 2>/dev/null | grep -q "^org.mate.Caminito " && break; sleep 0.1; done
    configure true

    run_panel() { # the panel, with Caminito's bus or not
        WAYLAND_DISPLAY=$caminito_display GDK_BACKEND=wayland "$pkg/bin/mate-panel" >>"$work/panel.log" 2>&1 &
        panel_pid=$!
    }
    run_panel
    sleep 6
    WAYLAND_DISPLAY=$caminito_display GDK_BACKEND=wayland python3 -I "$here/client.py" maximized \
        >"$work/client.log" 2>&1 &
    sleep 3
    state "01-floating"
    python3 -I "$here/shot.py" "$out/01-floating.png"

    gs "$(toplevel bottom)" floating false
    gs "$(toplevel left)" floating false
    sleep 3
    state "02-not-floating"
    python3 -I "$here/shot.py" "$out/02-not-floating.png"

    # An expanded panel floats too, off its edge only: it stays as long as its edge.
    gs "$(toplevel bottom)" expand true
    gs "$(toplevel bottom)" floating true
    sleep 3
    state "03-floating-expanded"
    python3 -I "$here/shot.py" "$out/03-floating-expanded.png"
    gs "$(toplevel bottom)" expand false

    # The panel again, with Floating on, on a bus where nothing owns org.mate.Caminito: the
    # same Wayland compositor, which the panel then treats as any other.
    kill "$panel_pid"; sleep 1
    gs "$(toplevel bottom)" floating true
    gs "$(toplevel left)" floating true
    second=$(dbus-daemon --session --fork --print-address)
    DBUS_SESSION_BUS_ADDRESS=$second WAYLAND_DISPLAY=$caminito_display GDK_BACKEND=wayland \
        "$pkg/bin/mate-panel" >>"$work/panel-no-caminito.log" 2>&1 &
    sleep 6
    state "04-no-caminito"
    python3 -I "$here/shot.py" "$out/04-no-caminito.png"
    kill $(jobs -p) 2>/dev/null || true
}
export -f inner
export work out here root caminito pkg
dbus-run-session -- bash -c inner
echo "wrote $out"
