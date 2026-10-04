ct() { sudo -u caminotest env XDG_RUNTIME_DIR=/run/user/1001 WAYLAND_DISPLAY=wayland-1 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1001/bus "$@"; }
settle() { local a b; a=-1; while :; do b=$(stat -c %s /home/caminotest/wev.log); [ "$a" = "$b" ] && break; a=$b; sleep 0.7; done; }
inp() { local n; n=$(wc -l < /root/t56-inputd.log); echo "$*" > /tmp/t56.fifo; while [ "$(wc -l < /root/t56-inputd.log)" = "$n" ]; do sleep 0.1; done; sleep 0.4; settle; }
dump() { ct busctl --user call org.mate.Camino.Debug /org/mate/Camino/Debug org.mate.Camino.Debug DumpState; }
mark() { L=$(wc -l < /home/caminotest/wev.log); }
since() { tail -n +$((L+1)) /home/caminotest/wev.log | tr -d "\0" | grep -v -E "frame|^\s*$"; }
clog() { sed "s/\x1b\[[0-9;]*m//g" /home/caminotest/camino.log | grep -a -v -E "x11_sockets|Xwayland:|gtk-palette|xwm|close\{fd|Opening device|Udev event|Device changed"; }
cmark() { CL=$(clog | wc -l); }
csince() { clog | tail -n +$((CL+1)) | cut -c1-200; }
active() { [ "$(fgconsole)" = 4 ] || { chvt 4; sleep 2; }; }
