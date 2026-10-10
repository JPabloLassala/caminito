#!/bin/bash
# Builds the applet's source with the four carried patches, then draws a menu served by
# serve.py through the patched translator and menu-chords.vala, under Xvfb and a private
# session bus (145).
#
#   check.sh <vala-panel-appmenu clone, which is reset to its tag 25.04> <work directory>
#
# Needs meson, valac, GTK 3, libmate-panel-applet, libwnck 3, json-glib, Xvfb, dbus and Python
# with PyGObject ($PYTHON, python3 if unset). Prints look's lines, leaves <work directory>/menu.png
# and exits as look does. $G_IR_SCANNER, if set, is the g-ir-scanner meson uses.
set -euo pipefail

src=$(realpath "$1")
work=$(realpath -m "$2")
here=$(dirname "$(realpath "$0")")
patches=$here/../../../packaging/arch/vala-panel-appmenu-mate-caminito
mkdir -p "$work"

git -C "$src" checkout -q --detach 25.04
git -C "$src" clean -dfq
git -C "$src" checkout -q -- .
for patch in "$patches"/*.patch; do
    patch -d "$src" -Np1 -s -i "$patch"
done

# GObject introspection's scanner, for the translator's .gir, may need a Python of its own.
native=()
if [[ -n ${G_IR_SCANNER:-} ]]; then
    printf "[binaries]\ng-ir-scanner = '%s'\n" "$G_IR_SCANNER" >"$work/native.ini"
    native=(--native-file "$work/native.ini")
fi
rm -rf "$work/build"
meson setup "$src" "$work/build" "${native[@]}" \
    --prefix=/usr --libdir=lib --libexecdir=lib --buildtype=plain --wrap-mode=nodownload \
    -Dauto_features=disabled -Dmate=enabled -Dwm_backend=wnck \
    --force-fallback-for=appmenu-glib-translator \
    -Dappmenu-glib-translator:default_library=static >"$work/setup.log"
meson compile -C "$work/build" >"$work/compile.log"

translator=$work/build/subprojects/appmenu-glib-translator
valac -q -d "$work/look" -o look \
    --pkg gtk+-3.0 --pkg json-glib-1.0 --vapidir "$translator" --pkg appmenu-glib-translator \
    -X -I"$translator" -X -I"$src/subprojects/appmenu-glib-translator" -X -w \
    "$here/look.vala" "$src/lib/menu-chords.vala" \
    -X "$translator/libappmenu-glib-translator.a" --pkg gio-unix-2.0

dbus-run-session -- bash -c '
    "${PYTHON:-python3}" "$1" "$2" & server=$!
    trap "kill $server" EXIT
    until gdbus introspect --session --dest org.caminito.Check145 --object-path /MenuBar >/dev/null 2>&1; do
        sleep 0.1
    done
    xvfb-run -a "$3" "$4"
' check "$here/serve.py" "$src/subprojects/appmenu-glib-translator/com.canonical.dbusmenu.xml" \
    "$work/look/look" "$work/menu.png"
