#!/usr/bin/env bash
# CMN-116 (ticket 250): shows that each new test can fail. Each break changes one thing in
# the code, runs the tests it should fail, and puts the file back from git. Run it in the
# OrbStack machine from the worktree, with the work committed:
#
#   orb -m camino bash -c 'cd <worktree> && .scratch/checks/CMN-116/break.sh'
set -u
cd "$(git rev-parse --show-toplevel)"
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$HOME/target-cmn-116}
ulimit -n "$(ulimit -Hn)"
src=caminito/src
status=0

# break <name> <file> <perl substitution> <test filter>...
brk() {
    local name=$1 file=$2 subst=$3
    shift 3
    perl -0pi -e "$subst" "$file"
    if git diff --quiet -- "$file"; then
        echo "BROKEN SCRIPT: $name changed nothing"
        status=1
        return
    fi
    touch "$file"
    local out
    out=$(dbus-run-session -- cargo test -q -p caminito --lib -- "$@" 2>&1)
    if grep -q "test result: FAILED" <<<"$out"; then
        echo "ok: $name fails $(grep -c '^    tests::\|^    overview::' <<<"$out") test(s)"
    else
        echo "NOT CAUGHT: $name"
        tail -5 <<<"$out"
        status=1
    fi
    git checkout -q -- "$file"
}


brk "a window that stays is at its new slot at once" $src/overview/reflow.rs \
    's/\QSome((_, from)) => between(*from, slot, 1.0 - way_left(now.saturating_sub(self.start))),\E/Some(_) => slot,/' \
    overview::reflow tests::overview_reflow
brk "a window leaving the layout doesn't ease the others" $src/overview/mod.rs \
    's/\QNone => left || moved_in,\E/None => moved_in,/' \
    tests::overview_reflow::after_a_drop tests::overview_reflow::a_window_that_closes
brk "a window joining from another output doesn't ease the others" $src/overview/mod.rs \
    's/\QNone => left || moved_in,\E/None => left,/' \
    tests::overview_reflow::with_two_outputs
brk "a window that opens eases the others" $src/overview/mod.rs \
    's/\QNone => left || moved_in,\E/None => left || joined.count() > 0,/' \
    tests::overview_reflow::a_window_that_closes
brk "a change part-way eases from the old slots" $src/overview/mod.rs \
    's/\Qlet stay = was.places.into_iter();\E/let stay = was.slots.into_iter();/' \
    tests::overview_reflow::a_second_drop
brk "it eases with the animation off" $src/overview/mod.rs \
    's/\Qif !reflow || !self.settings.overview_animation {\E/if !reflow {/' \
    tests::overview_reflow::without
brk "the scene draws the windows at their new slots" $src/scene.rs \
    's/\Qopen.place(id, slot, now)\E/slot/' \
    tests::overview_reflow::after_a_drop
brk "a click part-way hits the new slots" $src/overview/mod.rs \
    's/\Qif open.reflowing(self.now()).is_some() {\E/if false {/' \
    tests::overview_reflow::a_click_part_way
brk "the Overview closing doesn't cut the easing" $src/overview/mod.rs \
    's/\Q                shown.reflow = None;\E//' \
    tests::overview_reflow::the_overview_closing
brk "a switch of the workspace doesn't cut the easing" $src/overview/mod.rs \
    's/\Qif was.workspace != new.laid.workspace || was.closing {\E/if was.closing {/' \
    tests::overview_reflow::a_switch
brk "the dump has no easing" $src/debug.rs \
    's/\Q.filter(|laid_out| laid_out.reflowing(now).is_some())\E/.filter(|_| false)/' \
    tests::overview_reflow

exit $status
