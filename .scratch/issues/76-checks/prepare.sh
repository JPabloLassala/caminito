#!/bin/bash
# What the developer does before 76's login on a tty, from this checkout on x86_64: builds
# and installs the three Patched packages and caminito-git with makepkg -si, in the Packaging
# job's order, and copies the checks to caminitotest's home, since caminitotest can't read the
# developer's.
#
#   prepare.sh
#
# pacman asks before it takes out the stock package a Patched one conflicts with: answer yes.
# caminito-git is built from the checkout's committed HEAD, with its tests. Then log in as
# caminitotest on a spare tty, start the session, and run ~/76-checks/tty-login.sh.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
arch_dir=$(cd "$here/../../../packaging/arch" && pwd)
user=caminitotest

id $user >/dev/null 2>&1 || { echo "prepare: no $user; make it as the README's The MATE session says" >&2; exit 1; }

for package in mate-session-manager-caminito mate-panel-caminito mate-settings-daemon-caminito caminito-git; do
  echo "== $package"
  cd "$arch_dir/$package"
  # --force: a package that was built before is built again.
  makepkg --syncdeps --install --force
done

echo "== installed"
pacman -Q caminito-git mate-session-manager-caminito mate-panel-caminito mate-settings-daemon-caminito
pacman -Qo /usr/share/wayland-sessions/mate-caminito.desktop

echo "== the checks, in /home/$user/76-checks"
sudo install -d -o $user -g $user /home/$user/76-checks
sudo install -o $user -g $user -m 755 "$here/tty-login.sh" "$here/gtk-window.py" "$here/layer-demo.py" /home/$user/76-checks/
