#!/usr/bin/env bash
# CMN-127's check of the built mate-utils .debs, in a container of ubuntu:26.04, after
# `tools/debian-build mate-panel mate-utils` has left them in packaging/debian/out/:
#   .scratch/checks/CMN-127/replace-and-pin.sh
# 1. Ubuntu's four binary packages are installed, then ours over them with apt, with the
#    patched panel's applet library, which upgrades the four and removes nothing.
# 2. apt-cache policy gives 1001 to the caminito version and 99 to Ubuntu's, and an upgrade
#    has nothing to do.
# 3. The dictionary applet is a module of the panel's process, linked to the panel's .so.2.
# 4. The files of ours that Ubuntu's doesn't have, and the other way round, except the
#    translations and the help.
set -euo pipefail

if [ "${1-}" = --in-container ]; then
  export DEBIAN_FRONTEND=noninteractive
  packages=(mate-utils mate-utils-common libmatedict6 libmatedict-dev)
  apt-get update -qq
  apt-get install -y -qq --no-install-recommends "${packages[@]}" >/dev/null
  dpkg-query -W "${packages[@]}"
  files() {
    for p in "${packages[@]}"; do dpkg -L "$p"; done \
      | grep -v -e ^/usr/share/locale/ -e ^/usr/share/help/ | sort -u
  }
  files >/tmp/ubuntu.files
  echo "== 1. Ours over Ubuntu's"
  apt-get install -y --no-install-recommends /out/{mate-utils,libmatedict}*_1.29.1-0caminito*.deb \
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
  cat /etc/apt/preferences.d/caminito-mate-utils
  apt-cache policy "${packages[@]}"
  apt-get -s upgrade | grep -E '^[0-9]+ upgraded'
  if apt-get -s upgrade | grep -qE '^Inst (mate-utils|libmatedict)'; then echo "FAIL: an upgrade replaces ours"; exit 1; fi
  echo "== 3. The dictionary applet"
  applet=/usr/share/mate-panel/applets/org.mate.DictionaryApplet.mate-panel-applet
  grep -E '^(InProcess|Location)=' "$applet"
  module=$(sed -n 's/^Location=//p' "$applet")
  grep -qx 'InProcess=true' "$applet" || { echo "FAIL: the applet isn't in-process"; exit 1; }
  ldd "$module" | grep libmate-panel-applet
  ldd "$module" | grep -q 'libmate-panel-applet-4\.so\.2 ' \
    || { echo "FAIL: the applet doesn't link .so.2"; exit 1; }
  echo "== 4. Files except the translations and the help: < only Ubuntu's, > only ours"
  files >/tmp/ours.files
  diff /tmp/ubuntu.files /tmp/ours.files || true
  echo "ok"
  exit
fi

root=$(cd "$(dirname "$0")/../../.." && pwd)
podman run --rm -v "$root/packaging/debian/out:/out:ro" -v "$(realpath "$0"):/check.sh:ro" \
  ubuntu:26.04 bash /check.sh --in-container
