#!/bin/bash
# Builds buttons.c against the keybindings capplet's eggaccelerators.c with the carried
# series applied, and runs it on Xvfb (GTK needs a display to name a modifier).
#
#   run.sh [mate-control-center checkout at v1.29.1, with the series applied]
#
# With no argument it clones the tag into a temporary directory and applies the series.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
patches=$(cd "$here/../../../packaging/patches/mate-control-center" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

src=${1:-}
if [ -z "$src" ]; then
  src=$work/mate-control-center
  git clone -q --depth 1 -b v1.29.1 https://github.com/mate-desktop/mate-control-center.git "$src"
  for patch in "$patches"/*.patch; do
    patch -d "$src" -Np1 -s -i "$patch"
  done
fi

keybindings=$src/capplets/keybindings
gcc -Wall -Werror -o "$work/buttons" "$here/buttons.c" "$keybindings/eggaccelerators.c" \
  -I"$keybindings" $(pkg-config --cflags --libs gtk+-3.0 x11)
xvfb-run -a "$work/buttons"
