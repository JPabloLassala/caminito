#!/usr/bin/env bash
# Ticket 93's measurements: how long a nested Caminito's frames take, on whatever desktop
# this is run in. It starts `tools/dev-nested` with its side of the host's protocol
# logged, runs foot, then xterm, then both in it, each printing a character ten times a
# second for 8 s, and prints for each:
#
#   client  how long foot waited for the nested Caminito's frame callback after a commit
#   host    how long the nested Caminito waited for the host's frame callback after its own
#
# A slow `host` with a quick loop says the host holds the frames back; a slow `client`
# with a quick `host` says the nested Caminito does. Then it leaves the three windows open
# and asks what is slow by hand.
#
#   93-measure.sh [--bin]      # --bin: the checkout's debug build, as tools/dev-nested
#
# The logs stay in ~/t93/<host>-<time>, with the summary in `summary` there.
set -uo pipefail

here=$(dirname "$(realpath "$0")")
root=$here/../..
host=${XDG_CURRENT_DESKTOP:-unknown}
W=$HOME/t93/$host-$(date +%H%M%S)
mkdir -p "$W"
for tool in foot xterm gawk; do
  command -v $tool >/dev/null || { echo "93-measure: $tool isn't installed" >&2; exit 1; }
done

# The median, 90th percentile and longest wait in ms between a surface's commit with a
# frame request and that callback's done, in the WAYLAND_DEBUG lines on stdin.
latency() {
  gawk '
    function ms(  t) {
      match($0, /\[[0-9:. ]+\]/)
      split(substr($0, RSTART + 1, RLENGTH - 2), t, ":")
      return (t[1] * 3600 + t[2] * 60 + t[3]) * 1000
    }
    match($0, /-> wl_surface#[0-9]+\.frame\(new id wl_callback#[0-9]+/) {
      id = $0; sub(/.*wl_callback#/, "", id); sub(/[^0-9].*/, "", id)
      surface = $0; sub(/.*wl_surface#/, "", surface); sub(/\..*/, "", surface)
      requested[surface] = id
    }
    match($0, /-> wl_surface#[0-9]+\.commit\(/) {
      surface = $0; sub(/.*wl_surface#/, "", surface); sub(/\..*/, "", surface)
      if (surface in requested) { committed[requested[surface]] = ms(); delete requested[surface] }
    }
    /[^>] wl_callback#[0-9]+\.done\(/ {
      id = $0; sub(/.*wl_callback#/, "", id); sub(/\..*/, "", id)
      if (id in committed) { waits[++n] = ms() - committed[id]; delete committed[id] }
    }
    END {
      for (id in committed) unanswered++
      if (!n) { printf "no frames (%d unanswered)\n", unanswered; exit }
      asort(waits)
      printf "%3d frames, median %6.1f ms, p90 %6.1f ms, longest %7.1f ms, %d unanswered\n",
        n, waits[int((n + 1) / 2)], waits[int(n * 0.9) ? int(n * 0.9) : 1], waits[n], unanswered
    }'
}

# The nested Caminito's log has its own lines, Xwayland's and the helpers': only the ones
# libwayland wrote for Caminito itself start with the timestamp. The EGL library has a queue
# of its own, with a frame callback at each swap that it reads only at the next one, so
# its callbacks are left out of the wait and counted apart.
all_host_lines() { sed -n "$1,\$p" "$W/nested.log" | grep '^\[[0-9: .]*\] '; }
host_lines() { all_host_lines "$1" | grep -v -e '{EGLSurface.*\.frame(' -e '{EGLSurface.*wl_callback#'; }

summary=$W/summary
say() { printf '%s\n' "$*" | tee -a "$summary"; }

# In a session of its own, so that the trap ends Caminito and what it started with it.
WAYLAND_DEBUG=client setsid "$root/tools/dev-nested" "$@" >"$W/nested.out" 2>"$W/nested.log" &
nested=$!
trap 'kill -- -$nested 2>/dev/null; wait $nested 2>/dev/null' EXIT
for _ in $(seq 600); do
  grep -q 'dev-nested: WAYLAND_DISPLAY=' "$W/nested.log" && break
  kill -0 $nested 2>/dev/null || { tail -5 "$W/nested.log" >&2; exit 1; }
  sleep 0.2
done
line=$(grep -a -m1 'dev-nested: WAYLAND_DISPLAY=' "$W/nested.log") || { echo "93-measure: the nested Caminito didn't start; see $W/nested.log" >&2; exit 1; }
wayland=${line##*WAYLAND_DISPLAY=}; wayland=${wayland%% *}
display=${line##*DISPLAY=}
sleep 1

say "host $host, $(date -Is), $(pacman -Q caminito-git 2>/dev/null || echo 'caminito-git not installed')${1:+, with $*}"
say "renderer: $(sed 's/\x1b\[[0-9;]*m//g' "$W/nested.log" | grep -a -m1 -o 'GL Renderer: .*')"
tick='for i in $(seq 80); do printf x; sleep 0.1; done'

# phase <name> <foot?> <xterm?>
phase() {
  local name=$1 from pids=()
  from=$(($(wc -l <"$W/nested.log") + 1))
  if [[ $2 == yes ]]; then
    WAYLAND_DISPLAY=$wayland WAYLAND_DEBUG=1 foot -e sh -c "$tick" 2>"$W/foot-$name.log" &
    pids+=($!)
  fi
  if [[ $3 == yes ]]; then
    DISPLAY=$display xterm -e sh -c "$tick" 2>/dev/null &
    pids+=($!)
  fi
  if ((${#pids[@]})); then
    sleep 5
    say "$name: CPU $(top -b -n1 -d1 | gawk '$NF == "caminito" { printf "%s%% (pid %s) ", $9, $1 }')"
    wait "${pids[@]}"
  else
    sleep 3
  fi
  [[ $2 == yes ]] && say "$name: client $(latency <"$W/foot-$name.log")"
  say "$name: host   $(host_lines $from | latency)"
  say "$name: host   $(all_host_lines $from | grep -c -e '-> wl_surface#[0-9]*\.commit(') commits, $(all_host_lines $from | grep -c 'set_acquire_point') with an acquire point, $(all_host_lines $from | grep -c '{EGLSurface.*-> wl_surface#[0-9]*\.frame(') with the EGL library's own frame callback"
}

phase idle no no
phase foot yes no
phase xterm no yes
phase both yes yes

if [[ -t 0 ]]; then
  WAYLAND_DISPLAY=$wayland foot 2>/dev/null &
  DISPLAY=$display xterm 2>/dev/null &
  echo
  echo "foot and xterm are open in the nested Caminito. Type in each, move the pointer over"
  echo "them, and drag and resize one by its frame."
  read -r -p "What is slow, and in which? (one line; empty if nothing) " answer
  say "by hand: ${answer:-nothing is slow}"
  kill %% 2>/dev/null
fi
echo "93-measure: the summary is in $summary"
