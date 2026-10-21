#!/usr/bin/env bash
# CMN-128's check of the built mate-settings-daemon .debs, in a container of ubuntu:26.04, after
# `tools/debian-build mate-settings-daemon` has left them in packaging/debian/out/:
#   .scratch/checks/CMN-128/replace-and-pin.sh
# 1. Ubuntu's three binary packages are installed, then ours over them with apt, which upgrades
#    the three and removes nothing.
# 2. apt-cache policy gives 1001 to the caminito version and 99 to Ubuntu's, and an upgrade
#    has nothing to do.
# 3. The files of ours that Ubuntu's doesn't have, and the other way round, but translations.
set -euo pipefail

if [ "${1-}" = --in-container ]; then
  export DEBIAN_FRONTEND=noninteractive
  packages=(mate-settings-daemon mate-settings-daemon-common mate-settings-daemon-dev)
  apt-get update -qq
  apt-get install -y -qq --no-install-recommends "${packages[@]}" >/dev/null
  dpkg-query -W "${packages[@]}"
  for p in "${packages[@]}"; do dpkg -L "$p"; done | grep -v ^/usr/share/locale/ | sort -u >/tmp/ubuntu.files
  echo "== 1. Ours over Ubuntu's"
  apt-get install -y --no-install-recommends /out/mate-settings-daemon*_1.29.1-0caminito*.deb \
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
  cat /etc/apt/preferences.d/caminito-mate-settings-daemon
  apt-cache policy "${packages[@]}"
  apt-get -s upgrade | grep -E '^[0-9]+ upgraded'
  if apt-get -s upgrade | grep -q '^Inst mate-settings-daemon'; then echo "FAIL: an upgrade replaces ours"; exit 1; fi
  echo "== 3. Files but translations: < only Ubuntu's, > only ours"
  for p in "${packages[@]}"; do dpkg -L "$p"; done | grep -v ^/usr/share/locale/ | sort -u >/tmp/ours.files
  diff /tmp/ubuntu.files /tmp/ours.files || true
  echo "ok"
  exit
fi

root=$(cd "$(dirname "$0")/../../.." && pwd)
podman run --rm -v "$root/packaging/debian/out:/out:ro" -v "$(realpath "$0"):/check.sh:ro" \
  ubuntu:26.04 bash /check.sh --in-container
