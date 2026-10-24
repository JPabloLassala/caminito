#!/usr/bin/env bash
# CMN-224's check of Intellihide end to end, for what the headless tests of caminito and
# caminito-policy can't reach: the mate-panel patch (packaging/patches/mate-panel/0015) builds
# against upstream's pinned commit with the series before it, and its own wayland-intellihide.c,
# run by a probe on GDK's Wayland backend with gtk-layer-shell, is understood by Caminito
# (caminito/src/tests/mate_panel_intellihide.rs).
#
#   run.sh [<mate-panel checkout at the pinned commit>]
#
# It needs git, meson, ninja, a C compiler, the development files of GTK 3, gtk-layer-shell,
# wayland-client, mate-desktop, libwnck and the rest of mate-panel's, and dbus-run-session.
# Without a checkout it clones upstream into a scratch directory. It applies the series to a
# copy and builds that, leaving the checkout alone. Where libxfce4windowing 4.20.2 isn't
# installed, which only the window list needs, a stand-in .pc file satisfies meson's check for
# the build here; the panel itself doesn't link it.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
work=$(mktemp -d -t caminito-cmn-224.XXXXXX)
trap 'rm -rf "$work"' EXIT
commit=$(sed -n 's/^_commit=//p' "$root/packaging/arch/mate-panel-caminito/PKGBUILD")
src=${1:-}
if [ -z "$src" ]; then
  src=$work/upstream
  git clone -q https://github.com/mate-desktop/mate-panel.git "$src"
fi
git clone -q "$src" "$work/panel"
git -C "$work/panel" checkout -q "$commit"
git -C "$work/panel" submodule update --init -q 2>/dev/null || true
git -C "$work/panel" -c user.name=check -c user.email=check@example.com am -q "$root"/packaging/patches/mate-panel/*.patch

if ! pkg-config --exists 'libxfce4windowing-0 >= 4.20.2'; then
  mkdir -p "$work/pc"
  printf 'Name: libxfce4windowing-0\nDescription: stand-in\nVersion: 4.20.2\n' >"$work/pc/libxfce4windowing-0.pc"
  export PKG_CONFIG_PATH="$work/pc${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
fi
# The applets need gdbus-codegen and the like, which the panel does not.
sed -i "s/^subdir('applets')/#subdir('applets')/" "$work/panel/meson.build"
meson setup "$work/build" "$work/panel" -Dintrospection=disabled -Denable-eds=disabled \
  -Dgtk-doc=disabled >/dev/null
ninja -C "$work/build" mate-panel/mate-panel
grep -q 'wayland-intellihide.c' "$work/build/build.ninja" ||
  { echo "the panel was built without Wayland" >&2; exit 1; }
echo "mate-panel with the patch builds"

cc -o "$work/intellihide-probe" "$here/intellihide-probe.c" \
  "$work/panel/mate-panel/wayland-intellihide.c" \
  -DHAVE_CONFIG_H -I"$work/build" -I"$work/panel/mate-panel" \
  $(pkg-config --cflags --libs gtk+-3.0 gtk-layer-shell-0 wayland-client)
CAMINITO_INTELLIHIDE_PROBE="$work/intellihide-probe" dbus-run-session -- \
  cargo test --manifest-path "$root/Cargo.toml" -p caminito \
  tests::mate_panel_intellihide -- --ignored
