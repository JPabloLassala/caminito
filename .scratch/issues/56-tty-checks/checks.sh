#!/bin/bash
. /root/t56-env
M=org.mate.peripherals-mouse; T=org.mate.peripherals-touchpad; K=org.mate.peripherals-keyboard; X=org.mate.peripherals-keyboard-xkb.kbd
gs() { ct gsettings set "$@"; sleep 1.5; }
xs() { since | grep -a motion | sed -E "s/.*x, y: ([-0-9.]+),.*/\1/"; }
dist() { xs | awk "NR==1{a=\$1} {b=\$1} END{if(NR)printf \"%.0f\", b-a; else printf \"none\"}"; }
mdist() { mark; inp rel m1 $1 0 $2; dist; inp rel m1 -$1 0 $2; }
tdist() { mark; inp swipe tp1 400 0; dist; inp swipe tp1 -400 0; }
btns() { since | grep -a "button:" | sed -E "s/.*button: ([0-9]+).*state: ([01]).*/\1:\2/" | tr "\n" " "; }
axis() { since | grep -a "axis: time" | awk -v ax="$1" "\$0 ~ \"axis: \" ax {v+=\$NF; n++} END{if(n)printf \"%.1f\", v; else printf \"none\"}"; }
say() { printf "%-58s %s\n" "$1" "$2"; }
home() { inp rel m1 -30 -30 80; inp rel m1 4 3 100; }
active; cmark; ct gsettings reset $X layouts; home
echo "== mouse"
mark; inp btn m1 BTN_LEFT; say "left-handed=false: left button" "$(btns)"
gs $M left-handed true; mark; inp btn m1 BTN_LEFT; say "left-handed=true: left button" "$(btns)"
inp mk mouse m2; sleep 1; mark; inp btn m2 BTN_LEFT; say "  mouse plugged in after: left button" "$(btns)"; inp rm m2
gs $M left-handed false; mark; inp btn m1 BTN_LEFT; say "left-handed=false again: left button" "$(btns)"
say "motion-acceleration=-1: 20x6 units move" "$(mdist 6 20)"
gs $M motion-acceleration 1.0; say "motion-acceleration=1 (slowest)" "$(mdist 6 20)"
gs $M motion-acceleration 10.0; say "motion-acceleration=10 (fastest)" "$(mdist 6 20)"
ct gsettings reset $M motion-acceleration; sleep 1.5
gs $M accel-profile flat; say "accel-profile=flat: 8x30 fast, 40x6 slow" "$(mdist 30 8) $(mdist 6 40)"
gs $M accel-profile adaptive; say "accel-profile=adaptive: 8x30 fast, 40x6 slow" "$(mdist 30 8) $(mdist 6 40)"
gs $M accel-profile default
mark; inp chord m1; say "middle-button-enabled=true: left+right" "$(btns)"
gs $M middle-button-enabled false; mark; inp chord m1; say "middle-button-enabled=false: left+right" "$(btns)"
gs $M middle-button-enabled true
mark; inp wheel m1 -1; say "wheel down one notch" "$(axis 0)"
home; echo "== touchpad"
mark; inp tap tp1; say "tap-to-click=false: tap" "$(btns)"
gs $T tap-to-click true; mark; inp tap tp1; say "tap-to-click=true: tap" "$(btns)"
inp mk touchpad tp2; sleep 1.5; mark; inp tap tp2; say "  touchpad plugged in after: tap" "$(btns)"; inp rm tp2
gs $T tap-to-click false
say "touchpad-enabled=true: swipe" "$(tdist)"
gs $T touchpad-enabled false; say "touchpad-enabled=false: swipe" "$(tdist)"
gs $T touchpad-enabled true; sleep 1
mark; inp scroll2 tp1 0 300; say "natural-scroll=false: two fingers down" "$(axis 0)"
gs $T natural-scroll true; mark; inp scroll2 tp1 0 300; say "natural-scroll=true: two fingers down" "$(axis 0)"
gs $T natural-scroll false
mark; inp btn bp1 BTN_LEFT; say "left-handed=mouse, mouse right-handed: pad left button" "$(btns)"
gs $T left-handed left; mark; inp btn bp1 BTN_LEFT; say "left-handed=left: pad left button" "$(btns)"
gs $T left-handed mouse; gs $M left-handed true; mark; inp btn bp1 BTN_LEFT; say "left-handed=mouse, mouse left-handed: pad left button" "$(btns)"
gs $M left-handed false
say "motion-acceleration=-1: swipe" "$(tdist)"
gs $T motion-acceleration 1.0; say "motion-acceleration=1: swipe" "$(tdist)"
gs $T motion-acceleration 10.0; say "motion-acceleration=10: swipe" "$(tdist)"
ct gsettings reset $T motion-acceleration; sleep 1.5
gs $T accel-profile flat; say "accel-profile=flat: swipe" "$(tdist)"
gs $T accel-profile adaptive; say "accel-profile=adaptive: swipe" "$(tdist)"
gs $T accel-profile default
home
mark; inp click tp1 2; say "two-finger-click=0: two-finger click" "$(btns)"
gs $T two-finger-click 3; mark; inp click tp1 2; say "two-finger-click=3 (clickfinger): two-finger click" "$(btns)"
gs $T two-finger-click 0
gs $T vertical-two-finger-scrolling false; mark; inp scroll2 tp1 0 300; say "two-finger scrolling off, edge on: two fingers down" "$(axis 0)"
gs $T vertical-two-finger-scrolling true
mark; inp scroll2 tp1 300 0; say "horizontal-two-finger-scrolling=false: two fingers right" "$(axis 1)"
gs $T horizontal-two-finger-scrolling true; mark; inp scroll2 tp1 300 0; say "horizontal-two-finger-scrolling=true: two fingers right" "$(axis 1)"
gs $T horizontal-two-finger-scrolling false
home; echo "== keyboard"
mark; gs $K rate 40; gs $K delay 300; gs $K repeat false; since | grep -a repeat_info | cut -c20-90
ct gsettings reset $K rate; ct gsettings reset $K delay; ct gsettings reset $K repeat; sleep 1
mark; inp key k1 KEY_Q; say "layouts=[]: KEY_Q" "$(since | grep -a -A1 "state: 1" | grep -a sym | tr -s " ")"
gs $X layouts "[\"fr\"]"; mark; inp key k1 KEY_Q; say "layouts=[fr]: KEY_Q" "$(since | grep -a -A1 "state: 1" | grep -a sym | tr -s " ")"
inp mk kbd k2; sleep 1; mark; inp key k2 KEY_Q; say "  keyboard plugged in after: KEY_Q" "$(since | grep -a -A1 "state: 1" | grep -a sym | tr -s " ")"; inp rm k2
gs $X layouts "[\"us\tdvorak\"]"; mark; inp key k1 KEY_Q; say "layouts=[us dvorak]: KEY_Q" "$(since | grep -a -A1 "state: 1" | grep -a sym | tr -s " ")"
ct gsettings reset $X layouts; sleep 1
mark; inp key k1 KEY_Q; say "layouts reset: KEY_Q" "$(since | grep -a -A1 "state: 1" | grep -a sym | tr -s " ")"
echo "== VT switches and warnings during the run (must be empty)"
csince | grep -a -E "switched|WARN|ERROR|won.t take"
echo "vt now: $(fgconsole)"
