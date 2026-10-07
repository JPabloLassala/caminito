#!/bin/bash
# Starts Caminito for caminitotest in a logind session on tty4, the virtual devices and wev.
systemctl stop caminito-tty56 2>/dev/null; sleep 1
mv -f /home/caminitotest/caminito.log /home/caminitotest/caminito.log.old 2>/dev/null
systemd-run --unit=caminito-tty56 --collect -p PAMName=login -p User=caminitotest -p TTYPath=/dev/tty4 -p StandardInput=tty -p StandardOutput=file:/home/caminitotest/caminito.log -p StandardError=file:/home/caminitotest/caminito.log -p UtmpIdentifier=tty4 -p UtmpMode=user -p TTYReset=yes -p TTYVHangup=yes -p WorkingDirectory=/home/caminitotest -E XDG_SESSION_TYPE=wayland -E XDG_SESSION_CLASS=user -E XDG_SEAT=seat0 -E XDG_VTNR=4 -E RUST_LOG=debug /home/caminitotest/caminito
chvt 4; sleep 5
modprobe uinput
[ -p /tmp/t56.fifo ] || { setsid nohup python3 /root/t56-inputd.py > /root/t56-inputd.log 2>&1 < /dev/null & sleep 1; }
rm -f /home/caminitotest/wev.log
setsid nohup sudo -u caminitotest env XDG_RUNTIME_DIR=/run/user/1001 WAYLAND_DISPLAY=wayland-1 stdbuf -oL wev > /home/caminitotest/wev.log 2>&1 < /dev/null &
sleep 2
. /root/t56-env
sleep 6; for d in "mouse m1" "kbd k1" "touchpad tp1" "padbuttons bp1"; do inp rm ${d#* } 2>/dev/null; inp mk $d; done
W=1 inp rel m1 -30 -30 80; W=1 inp rel m1 4 3 100
