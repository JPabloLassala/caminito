#!/bin/bash
# Builds and installs the packages named on the command line, in the order given, from this
# checkout's packaging/arch/. With no names: the three Patched packages and camino-git, in
# the Packaging job's order.
#
# For a machine that isn't x86_64 (59): -A ignores the PKGBUILDs' arch, and --nocheck skips
# camino-git's check(), whose goldens fail on aarch64. On x86_64, use makepkg -si as the
# README says.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
arch_dir=$(cd "$here/../../../../packaging/arch" && pwd)

if [ $# -eq 0 ]; then
  set -- mate-session-manager-camino mate-panel-camino mate-settings-daemon-camino camino-git
fi

for package in "$@"; do
  echo "== $package"
  cd "$arch_dir/$package"
  rm -f ./*.pkg.tar.*
  # --force: a package that was built before is built again.
  makepkg --ignorearch --nocheck --syncdeps --noconfirm --force
  # Not makepkg --install: with --noconfirm, pacman answers no to taking out the stock
  # package that a Patched one replaces.
  sudo pacman -U $(ls ./*.pkg.tar.* | grep -v -- '-debug-') < <(yes)
done
