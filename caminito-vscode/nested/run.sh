#!/usr/bin/env bash
# The global menu extension by hand (116): a nested Caminito on a private bus, with a scratch
# XDG_CONFIG_HOME and a runtime directory of its own, mate-panel with the global menu applet
# from a built vala-panel-appmenu-mate-caminito package, and two windows of an editor on
# native Wayland with its custom title bar, which loads the extension as a built-in. It
# waits while the person looks, and prints the claims Caminito logged and the extension's
# own log when Enter is pressed.
#
#   run.sh <vala-panel-appmenu-mate-caminito-*.pkg.tar.zst>
#
# EDITOR_COMMAND is the editor to run, `code` by default, and EDITOR_BUILTINS its built-in
# extensions directory, /usr/share/code/resources/app/extensions by default (ADR 0022).
#
# It needs a Wayland desktop to nest in, a debug build of caminito in target/, npm and
# mate-panel-caminito.
#
# Without a runtime directory of its own, the accessibility bus that the private session
# bus starts takes the desktop's socket path in /run/user/<uid>/at-spi/ (10).
set -euo pipefail
here=$(dirname "$(realpath "$0")")
extension=$(dirname "$here")
root=$(git -C "$here" rev-parse --show-toplevel)
editor=${EDITOR_COMMAND:-code}
builtins=${EDITOR_BUILTINS:-/usr/share/code/resources/app/extensions}

(cd "$extension" && npm ci --silent && npm run --silent build)

work=$(mktemp -d -t caminito-vscode.XXXXXX)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work"/{pkg,applets,schemas,config,builtin,extensions,a,b} "$work/user/User"
mkdir -m 700 "$work/runtime"
tar -C "$work/pkg" -xf "$1" 2>/dev/null
# The stock applets too: the variable replaces the panel's own directory.
cp /usr/share/mate-panel/applets/* "$work/applets/"
sed "s#^Location=.*#Location=$work/pkg/usr/lib/mate-panel/libappmenu-mate.so#" \
  "$work/pkg/usr/share/mate-panel/applets/org.vala-panel.appmenu.mate-panel-applet" \
  >"$work/applets/org.vala-panel.appmenu.mate-panel-applet"
cp "$work/pkg/usr/share/glib-2.0/schemas/org.valapanel.appmenu.gschema.xml" "$work/schemas/"
glib-compile-schemas "$work/schemas"

# The editor's own built-ins and the extension, linked as the package will link it.
for dir in "$builtins"/*/; do ln -s "$dir" "$work/builtin/"; done
ln -s "$extension" "$work/builtin/caminito.global-menu"
cat >"$work/user/User/settings.json" <<'JSON'
{
  "window.titleBarStyle": "custom",
  "window.menuBarVisibility": "classic",
  "workbench.startupEditor": "none"
}
JSON

# The desktop's socket by its full path, since the runtime directory changes under it.
host=${WAYLAND_DISPLAY:?run.sh needs a Wayland desktop}
[[ $host == /* ]] || host=$XDG_RUNTIME_DIR/$host
WAYLAND_DISPLAY=$host XDG_RUNTIME_DIR=$work/runtime XDG_CONFIG_HOME=$work/config \
  dbus-run-session -- bash "$here/session.sh" "$root" "$work" "$editor"
