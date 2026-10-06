#!/bin/sh
# Ticket 90: turns each monitor off and on again over output management, with no mode,
# place or scale asked, and says whether the monitors and the primary output are as they
# were. Run it in the developer's Session, as it starts: the layout is then the saved one.
#
#   .scratch/camino-v0.1/issues/90-off-on.sh
#
# Each monitor is off for 3 s.

set -u

# Each head's name, whether it's enabled, its current mode, place and scale, and the
# primary output.
state() {
    wlr-randr | grep -E '^[^ ]|Enabled:|\(current\)|\(preferred, current\)|Position:|Scale:' |
        sed 's/ "[^"]*"$//'
    busctl --user get-property org.mate.Camino /org/mate/Camino org.mate.Camino PrimaryOutput
}

echo "camino-git: $(pacman -Q camino-git 2>/dev/null | cut -d' ' -f2), running since $(ps -o lstart= -C camino | head -n 1)"
before=$(state)
printf '\nBefore:\n%s\n' "$before"

failed=0
for output in $(wlr-randr | grep -oE '^[^ ]+'); do
    printf '\n%s goes off for 3 s.\n' "$output"
    wlr-randr --output "$output" --off
    sleep 3
    wlr-randr --output "$output" --on
    sleep 2
    after=$(state)
    if [ "$after" = "$before" ]; then
        echo "$output is back, and the layout with it."
    else
        failed=1
        echo "$output is back, and the layout ISN'T as it was:"
        printf '%s\n' "$before" >"${TMPDIR:-/tmp}/90-before"
        printf '%s\n' "$after" | diff "${TMPDIR:-/tmp}/90-before" -
    fi
done
exit $failed
