#!/usr/bin/env bash
# CMN-126's check of the built mate-power-manager .debs, in a container of ubuntu:26.04, after
# `tools/debian-build mate-power-manager` has left them in packaging/debian/out/:
#   .scratch/checks/CMN-126/replace-and-pin.sh
# 1. Ubuntu's two binary packages are installed, then ours over them with apt, with
#    mate-panel's libmate-panel-applet-4-2 from the same build, which upgrades the two and
#    removes nothing.
# 2. apt-cache policy gives 1001 to the caminito version and 99 to Ubuntu's, and an upgrade
#    has nothing to do.
# 3. The files of ours that Ubuntu's doesn't have, and the other way round, except the
#    translations.
# 4. The applets link libmate-panel-applet-4.so.2.
set -euo pipefail

if [ "${1-}" = --in-container ]; then
  export DEBIAN_FRONTEND=noninteractive
  packages=(mate-power-manager mate-power-manager-common)
  apt-get update -qq
  apt-get install -y -qq --no-install-recommends "${packages[@]}" >/dev/null
  dpkg-query -W "${packages[@]}"
  for p in "${packages[@]}"; do dpkg -L "$p"; done | grep -v ^/usr/share/locale/ | sort -u >/tmp/ubuntu.files
  echo "== 1. Ours over Ubuntu's"
  apt-get install -y --no-install-recommends /out/mate-power-manager*_1.28.1-0caminito*.deb \
    /out/libmate-panel-applet-4-2_*.deb \
    | tee /tmp/install.log | grep -E '^[0-9]+ upgraded|^Upgrading|^Removing|REMOVED' || true
  if grep -q '^REMOVED\|^Removing' /tmp/install.log; then echo "FAIL: the install removed a package"; exit 1; fi
  dpkg-query -W "${packages[@]}"
  for p in "${packages[@]}"; do
    case $(dpkg-query -W -f '${Version}' "$p") in
      *caminito*) ;;
      *) echo "FAIL: $p isn't the caminito version"; exit 1 ;;
    esac
  done
  echo "== 2. The pin"
  cat /etc/apt/preferences.d/caminito-mate-power-manager
  apt-cache policy "${packages[@]}"
  apt-get -s upgrade | grep -E '^[0-9]+ upgraded'
  if apt-get -s upgrade | grep -q '^Inst mate-power-manager'; then echo "FAIL: an upgrade replaces ours"; exit 1; fi
  echo "== 3. Files except the translations: < only Ubuntu's, > only ours"
  for p in "${packages[@]}"; do dpkg -L "$p"; done | grep -v ^/usr/share/locale/ | sort -u >/tmp/ours.files
  diff /tmp/ubuntu.files /tmp/ours.files || true
  echo "== 4. The applets' applet library"
  apt-get install -y -qq --no-install-recommends binutils >/dev/null
  for applet in /usr/libexec/mate-power-manager/mate-*-applet; do
    needed=$(readelf -d "$applet" | grep -o 'libmate-panel-applet[^]]*')
    echo "$applet: $needed"
    [ "$needed" = libmate-panel-applet-4.so.2 ] || { echo "FAIL: $applet links $needed"; exit 1; }
  done
  echo "ok"
  exit
fi

root=$(cd "$(dirname "$0")/../../.." && pwd)
podman run --rm -v "$root/packaging/debian/out:/out:ro" -v "$(realpath "$0"):/check.sh:ro" \
  ubuntu:26.04 bash /check.sh --in-container
