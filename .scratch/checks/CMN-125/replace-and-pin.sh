#!/usr/bin/env bash
# CMN-125's check of the built vala-panel-appmenu and appmenu-gtk-module .debs, in a container
# of ubuntu:26.04, after `tools/debian-build mate-panel appmenu-gtk-module vala-panel-appmenu`
# has left them in packaging/debian/out/:
#   .scratch/checks/CMN-125/replace-and-pin.sh
# 1. Ubuntu's binary packages are installed, then ours over them with apt, with mate-panel's
#    libmate-panel-applet-4-2 from the same build, which upgrades them and removes nothing.
# 2. apt-cache policy gives 1001 to the caminito versions and 99 to Ubuntu's, and an upgrade
#    has nothing to do. Ubuntu's versions sort below 1:25.04-0caminito1, the version in
#    caminito's Recommends.
# 3. The files of ours that Ubuntu's doesn't have, and the other way round, except the
#    translations.
# 4. The applet's .mate-panel-applet file names a library that exists, which links
#    libmate-panel-applet-4.so.2.
# 5. The file the session finds the patched module by is there.
set -euo pipefail

if [ "${1-}" = --in-container ]; then
  export DEBIAN_FRONTEND=noninteractive
  packages=(mate-applet-appmenu vala-panel-appmenu-common appmenu-gtk3-module
    appmenu-gtk-module-common libappmenu-gtk3-parser0)
  apt-get update -qq
  apt-get install -y -qq --no-install-recommends "${packages[@]}" >/dev/null
  dpkg-query -W "${packages[@]}"
  ubuntu_applet=$(dpkg-query -W -f '${Version}' mate-applet-appmenu)
  ubuntu_module=$(dpkg-query -W -f '${Version}' appmenu-gtk3-module)
  for p in "${packages[@]}"; do dpkg -L "$p"; done | grep -v ^/usr/share/locale/ | sort -u >/tmp/ubuntu.files
  echo "== 1. Ours over Ubuntu's"
  debs=()
  for p in "${packages[@]}"; do debs+=(/out/"${p}"_25.04-0caminito*.deb); done
  apt-get install -y --no-install-recommends "${debs[@]}" /out/libmate-panel-applet-4-2_*.deb \
    | tee /tmp/install.log | grep -E '^[0-9]+ upgraded|^Upgrading|^Removing|REMOVED' || true
  if grep -q '^REMOVED\|^Removing' /tmp/install.log; then echo "FAIL: the install removed a package"; exit 1; fi
  dpkg-query -W "${packages[@]}"
  for p in "${packages[@]}"; do
    case $(dpkg-query -W -f '${Version}' "$p") in
      *caminito*) ;;
      *) echo "FAIL: $p isn't the caminito version"; exit 1 ;;
    esac
  done
  echo "== 2. The pins"
  cat /etc/apt/preferences.d/caminito-vala-panel-appmenu /etc/apt/preferences.d/caminito-appmenu-gtk-module
  apt-cache policy "${packages[@]}"
  apt-get -s upgrade | grep -E '^Inst |^[0-9]+ upgraded'
  if apt-get -s upgrade | grep -qE '^Inst (mate-applet-appmenu|vala-panel-appmenu|appmenu-gtk|libappmenu)'; then
    echo "FAIL: an upgrade replaces ours"; exit 1
  fi
  for v in "$ubuntu_applet" "$ubuntu_module"; do
    if dpkg --compare-versions "$v" ge 1:25.04-0caminito1; then
      echo "FAIL: Ubuntu's $v satisfies >= 1:25.04-0caminito1"; exit 1
    fi
  done
  echo "Ubuntu's $ubuntu_applet and $ubuntu_module are below 1:25.04-0caminito1"
  echo "== 3. Files except the translations: < only Ubuntu's, > only ours"
  for p in "${packages[@]}"; do dpkg -L "$p"; done | grep -v ^/usr/share/locale/ | sort -u >/tmp/ours.files
  diff /tmp/ubuntu.files /tmp/ours.files || true
  echo "== 4. The applet's library"
  apt-get install -y -qq --no-install-recommends binutils >/dev/null
  applet_file=$(dpkg -L mate-applet-appmenu | grep '\.mate-panel-applet$')
  location=$(sed -n 's/^Location=//p' "$applet_file")
  echo "$applet_file: Location=$location"
  [ -f "$location" ] || { echo "FAIL: $location doesn't exist"; exit 1; }
  needed=$(readelf -d "$location" | grep -o 'libmate-panel-applet[^]]*')
  echo "$location: $needed"
  [ "$needed" = libmate-panel-applet-4.so.2 ] || { echo "FAIL: the applet links $needed"; exit 1; }
  echo "== 5. The session's marker file"
  dpkg -S /usr/share/caminito/appmenu-gtk-module
  echo "ok"
  exit
fi

root=$(cd "$(dirname "$0")/../../.." && pwd)
podman run --rm -v "$root/packaging/debian/out:/out:ro" -v "$(realpath "$0"):/check.sh:ro" \
  ubuntu:26.04 bash /check.sh --in-container
