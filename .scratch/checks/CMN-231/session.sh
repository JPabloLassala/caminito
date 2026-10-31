# The session of CMN-231's check: see run.sh.
set -euo pipefail
gsettings set org.mate.Marco.general theme EsmerilSeeThrough
gsettings set org.mate.Marco.general center-new-windows true
gsettings set org.mate.interface enable-animations false

log=$WORK/caminito.log
RUST_LOG=${RUST_LOG:-warn,caminito=info} "$TARGET/debug/caminito" >"$log" 2>&1 &
caminito=$!
trap 'kill $caminito 2>/dev/null || true' EXIT
for _ in $(seq 300); do grep -q 'Xwayland is ready on DISPLAY=' "$log" && break; sleep 0.1; done
nested=$(sed -n 's/.*Xwayland is ready on DISPLAY=\(:[0-9]*\).*/\1/p' "$log" | head -1)
[ -n "$nested" ] || { cat "$log"; echo "no Xwayland"; exit 1; }

xdo() { DISPLAY=$OUTER xdotool "$@"; }
shot() { sleep "${2:-1.5}"; DISPLAY=$OUTER import -window root "$OUT/$1.png"; }
# Drags from (x1,y1) to (x2,y2) on the screen, in steps.
drag() {
  xdo mousemove "$1" "$2"; sleep 0.3; xdo mousedown 1; sleep 0.2
  for i in 1 2 3 4 5 6 7 8; do
    xdo mousemove $(($1 + ($3 - $1) * i / 8)) $(($2 + ($4 - $2) * i / 8)); sleep 0.1
  done
  sleep 0.2; xdo mouseup 1; sleep 0.3
}

DISPLAY=$nested xterm -T beneath -bg '#e02020' -fg '#e02020' -geometry 70x24 -e sleep 600 &
sleep 2
DISPLAY=$nested xterm -T above -bg '#2040e0' -fg '#2040e0' -geometry 50x10 -e sleep 600 &
sleep 2
# The two are centred, on an output wider than the screen. The titlebar goes across the left
# edge of the window beneath, at x 428.
drag 640 333 428 333
beneath=$(DISPLAY=$nested xdotool search --name '^beneath$' | tail -1)

row=342
report() {
  shot "$1"
  {
    echo "== $1: see-through row y=$row, x=300..556, every 16 pixels (red channel)"
    magick "$OUT/$1.png" -crop "257x1+300+$row" +repage -sample 257x1 txt:- |
      awk -F'[(),]' 'NR>1 && (NR-2)%16==0 {printf "%d ", $3} END {print ""}'
  } >>"$OUT/report.txt"
}

report 01-default
# The window beneath moves itself, 60 pixels left, and stays beneath.
DISPLAY=$nested xdotool getwindowgeometry "$beneath" >>"$OUT/report.txt"
pos=$(DISPLAY=$nested xdotool getwindowgeometry --shell "$beneath")
eval "$pos"
DISPLAY=$nested xdotool windowmove "$beneath" $((X - 60)) "$Y"
report 02-moved
gsettings set org.mate.Caminito esmeril-strength 1
report 03-strength-1
gsettings set org.mate.Caminito esmeril false
report 04-off
gsettings set org.mate.Caminito esmeril true
report 05-on-again
gsettings set org.mate.Marco.general reduced-resources true
report 06-reduced
gsettings set org.mate.Marco.general reduced-resources false

echo "== caminito's warnings" >>"$OUT/report.txt"
grep -E -i ' (WARN|ERROR) ' "$log" | sed 's/\x1b\[[0-9;]*m//g' | sort -u -k3 | head -20 >>"$OUT/report.txt" || true
kill -0 $caminito && echo "caminito still runs" >>"$OUT/report.txt"
