#!/usr/bin/env bash
# CMN-172's checks of tools/bootstrap-fedora's result, in a Fedora 45 machine, after it ran:
#   orb -m camino-fedora-clean bash .scratch/checks/CMN-172/check-bootstrap.sh
# 1. `dnf versionlock` holds each package that comes from packaging/fedora/'s specs, at its
#    installed version. Those are the packages with release `0.caminito…`, and caminito.
# 2. No package has two versionlock entries.
# 3. `dnf upgrade --assumeno` proposes none of them.
# 4. LightDM isn't enabled.
set -uo pipefail
fail=0
bad() { echo "FAIL: $*"; fail=1; }

mapfile -t ours < <(rpm -qa --qf '%{NAME} %{VERSION}-%{RELEASE}\n' \
  | awk '$2 ~ /-0\.caminito[0-9]+\./ || $1 == "caminito" {print $1}' | sort)
# The 26 binary packages of CMN-170's eight specs, and caminito.
expected_min=27
echo "installed from the specs: ${#ours[@]}"
[ "${#ours[@]}" -ge "$expected_min" ] || bad "fewer than $expected_min packages from the specs"

locks=$(dnf versionlock list)
for name in "${ours[@]}"; do
  grep -qx "Package name: $name" <<<"$locks" || bad "$name isn't held"
  evr=$(rpm -q --qf '%{VERSION}-%{RELEASE}' "$name")
  grep -A1 -x "Package name: $name" <<<"$locks" | grep -qxF "evr = $evr" \
    || bad "$name is held at another version than $evr"
done

dups=$(sed -n 's/^Package name: //p' <<<"$locks" | sort | uniq -d)
[ -z "$dups" ] || bad "more than one entry for: $dups"

upgrade=$(sudo dnf upgrade --assumeno 2>&1)
for name in "${ours[@]}"; do
  grep -qE "^ $name +[a-z0-9_]+ " <<<"$upgrade" && bad "dnf upgrade proposes $name"
done

state=$(systemctl is-enabled lightdm 2>/dev/null || true)
[ "$state" != enabled ] || bad "lightdm is enabled"

[ "$fail" -eq 0 ] && echo "OK"
exit "$fail"
