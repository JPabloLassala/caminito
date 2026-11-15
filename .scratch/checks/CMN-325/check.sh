#!/usr/bin/env bash
# CMN-325: lays out what Meson installs under /usr/share/caminito in a temporary root, then
# resolves the Control Center's, the Preferences' and the Applications' menus with libmate-menu
# (menus.py), once as X11 MATE has them and once as a Session does. Needs meson, python-gobject
# and mate-menus. Run it from the repository's root.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "${tmp:?}"' EXIT
meson setup --prefix=/usr "$tmp/build" >/dev/null
meson introspect --installed "$tmp/build" | python3 -c '
import json, os, shutil, sys
root = sys.argv[1]
for src, dest in json.load(sys.stdin).items():
    if dest.startswith("/usr/share/caminito/xdg"):
        out = root + dest
        os.makedirs(os.path.dirname(out), exist_ok=True)
        shutil.copy(src, out)
' "$tmp/root"
data=$tmp/root/usr/share/caminito/xdg
config=$tmp/root/usr/share/caminito/xdg-config
echo "##### X11 MATE"
env -u XDG_CONFIG_DIRS XDG_DATA_DIRS=/usr/local/share:/usr/share XDG_CURRENT_DESKTOP=MATE \
    python3 "$here/menus.py"
echo "##### Session"
XDG_DATA_DIRS=$data:/usr/local/share:/usr/share XDG_CONFIG_DIRS=$config:/etc/xdg \
    XDG_CURRENT_DESKTOP=MATE python3 "$here/menus.py"
