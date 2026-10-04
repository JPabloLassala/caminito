# Ticket 56's checks on the tty

The scripts that checked [56](../56-libinput-on-the-tty.md)'s acceptance criteria on 2026-10-04, in the Arch Linux ARM virtual machine, as `caminotest` (uid 1001) in a logind session on tty4. They ran as root from `/root`, named `t56-env`, `t56-inputd.py`, `t56-start.sh` and `t56-checks.sh`, and they need `wev` and `python-evdev`.

- `inputd.py`: makes uinput mice, keyboards and touchpads, and sends their events, as commands on `/tmp/t56.fifo` tell it.
- `start.sh`: starts `/home/caminotest/camino` on tty4, the devices and `wev`.
- `env.sh`: the helpers the others source.
- `checks.sh`: sets each of 53's keys with `gsettings`, sends input, and prints what `wev` received.

A VT switch or a pointer outside `wev`'s window makes a check print nothing: `checks.sh` lists the VT switches at its end, and brings the pointer back before each section.
