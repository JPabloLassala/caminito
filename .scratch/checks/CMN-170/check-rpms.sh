#!/usr/bin/env bash
# CMN-170's checks of the built RPMs, in the Fedora 45 machine, after packaging/fedora/build
# has built the eight specs:
#   orb -m camino-fedora bash .scratch/checks/CMN-170/check-rpms.sh
# 1. Each spec lists every patch of packaging/patches/<component>/ and of its fedora/, in order.
# 2. Each binary package provides its stock name at the baseline version, with the caminito
#    Release, and what it requires of another of the eight is at that baseline too.
# 3. mate-panel-libs has both applet libraries, and no development link.
# 4. Each build applied the number of patches its spec lists.
set -uo pipefail
root=$(cd "$(dirname "$0")/../../.." && pwd)
rpms=$(rpm --eval '%{_rpmdir}')
fail=0
bad() { echo "FAIL: $*"; fail=1; }

declare -A baseline=(
  [mate-session-manager]=1.28.0^20260721git57a9178 [mate-panel]=1.29.0~20260925git58ac6c1
  [mate-settings-daemon]=1.29.1 [mate-control-center]=1.29.1 [caja]=1.28.0
  [mate-utils]=1.29.1 [mate-power-manager]=1.28.1 [marco]=1.29.1
)

echo "== 1. The series in each spec"
for c in "${!baseline[@]}"; do
  spec=$root/packaging/fedora/$c/$c.spec
  want=$( { [ -d "$root/packaging/patches/$c" ] && ls "$root/packaging/patches/$c" | grep '\.patch$'
            [ -d "$root/packaging/patches/$c/fedora" ] && ls "$root/packaging/patches/$c/fedora" | grep '\.patch$'; } || true)
  have=$(grep -E '^Patch[12][0-9]{2}:' "$spec" | awk '{print $2}')
  if [ "$want" = "$have" ]; then echo "ok   $c: $(echo -n "$have" | grep -c .) carried"; else bad "$c's spec doesn't list its series"; fi
done

echo "== 2. Provides and requires"
for c in "${!baseline[@]}"; do
  srpm=$(ls "$(rpm --eval '%{_srcrpmdir}')/$c-${baseline[$c]}-0.caminito1.fc45.src.rpm" 2>/dev/null)
  [ -n "$srpm" ] || { bad "no source RPM of $c at ${baseline[$c]}"; continue; }
  for f in $(cd "$rpms" && ls aarch64/*.rpm noarch/*.rpm 2>/dev/null | grep -v -e debuginfo -e debugsource); do
    [ "$(rpm -qp --qf '%{SOURCERPM}' "$rpms/$f")" = "$(basename "$srpm")" ] || continue
    name=$(rpm -qp --qf '%{NAME}' "$rpms/$f")
    want="$name = ${baseline[$c]}-0.caminito1.fc45"
    if rpm -qp --provides "$rpms/$f" | grep -qxF "$want"; then echo "ok   provides $want"
    else bad "$f doesn't provide $want"; fi
    # A requirement on one of the eight packages' names is on this build, at its baseline.
    rpm -qp --requires "$rpms/$f" | grep -E '^(mate-session-manager|mate-panel|mate-settings-daemon|mate-control-center|caja|mate-utils|mate-power-manager|marco|mate-dictionary|mate-screenshot|mate-search-tool|mate-system-log|mate-disk-usage-analyzer)[a-z-]*(\(|$| )' |
      while read -r req; do
        case $req in
          *" = ${baseline[$c]}-0.caminito1.fc45") echo "ok     requires $req" ;;
          *" = "*) echo "FAIL:   $name requires $req" ;;
          *) echo "     $name requires $req (any version)" ;;
        esac
      done
  done
done

echo "== 3. mate-panel-libs"
libs=$(ls "$rpms"/aarch64/mate-panel-libs-1.29.0~*.rpm)
files=$(rpm -qlp "$libs")
for so in libmate-panel-applet-4.so.1 libmate-panel-applet-4.so.2; do
  echo "$files" | grep -qx "/usr/lib64/$so" && echo "ok   $so" || bad "mate-panel-libs has no $so"
done
echo "$files" | grep -qx /usr/lib64/libmate-panel-applet-4.so && bad "mate-panel-libs has the development link"
rpm -qp --provides "$libs" | grep 'libmate-panel-applet'
rpm -qlp "$rpms"/aarch64/mate-panel-devel-1.29.0~*.rpm | grep 'libmate-panel-applet-4.so$'
readlink -f /usr/lib64/libmate-panel-applet-4.so 2>/dev/null

echo "== 4. Patches applied"
for c in "${!baseline[@]}"; do
  log=$HOME/cmn-170-work/$c.log
  [ "$c" = mate-settings-daemon ] && log=$HOME/cmn-170-work/msd.log
  listed=$(grep -cE '^Patch[0-9]+:' "$root/packaging/fedora/$c/$c.spec")
  # msd's Patch0 is RHEL's only.
  [ "$c" = mate-settings-daemon ] && listed=$((listed - 1))
  applied=$(grep -c '^+ /usr/bin/patch ' "$log")
  [ "$listed" = "$applied" ] && echo "ok   $c: $applied applied" || bad "$c: $listed listed, $applied applied"
done

exit $fail
