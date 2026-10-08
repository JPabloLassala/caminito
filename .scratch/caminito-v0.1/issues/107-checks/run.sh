#!/bin/bash
# Builds button-test against a mate-power-manager checkout with the carried patch applied,
# and runs it in both modes on Xvfb, on a private session bus. The test makes that bus the
# system's too: UpClient, which GpmButton makes, wants one, and needs no UPower on it.
#
#   run.sh /path/to/mate-power-manager
#
# The checkout is left as it is: the patch goes on a copy of gpm-button.c in a scratch
# directory. gpm-button.c calls UpClient API that upower-glib deprecates, which isn't the
# patch's, so those warnings aren't errors. Needs gtk3's, upower-glib's and gio's headers, dbus-daemon and xvfb-run.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
source=$(cd "${1:?run.sh /path/to/mate-power-manager}" && pwd)
patch=$here/../../../../packaging/arch/mate-power-manager-caminito/0001-button-Grab-the-keys-through-Caminito.patch
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

mkdir "$work/src"
cp "$source/src/gpm-button.c" "$source/src/gpm-button.h" "$source/src/gpm-common.h" "$work/src/"
(cd "$work" && patch -sp1 <"$patch")
touch "$work/config.h"

cc -Wall -Wextra -Wno-unused-parameter -Wno-deprecated-declarations -Werror -o "$work/button-test" \
  -I"$work" -I"$work/src" "$here/button-test.c" "$work/src/gpm-button.c" \
  $(pkg-config --cflags --libs gtk+-3.0 gio-2.0 upower-glib x11)

for mode in caminito x11; do
  xvfb-run -a dbus-run-session -- "$work/button-test" "$mode"
done
