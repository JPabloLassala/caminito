#!/usr/bin/env bash
# CMN-119 (ticket 249): shows that each new test can fail. Each break changes one thing in
# the code, runs the tests it should fail, and puts the file back from git. Run it in the
# OrbStack machine from the worktree, with the work committed:
#
#   orb -m camino bash -c 'cd <worktree> && .scratch/checks/CMN-119/break.sh'
set -u
cd "$(git rev-parse --show-toplevel)"
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$HOME/target-cmn-119}
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
        status=1
    fi
    git checkout -q -- "$file"
}

brk "the cell's scale isn't applied to the place" $src/overview/drag.rs \
    's/\Qcell.loc + Point::from((at(rect.loc.x), at(rect.loc.y)))\E/cell.loc + rect.loc/' \
    overview::drag tests::overview_drag
brk "the grab point is ignored" $src/overview/drag.rs \
    's/\Qlet corner = pointer - Point::from((grab.x * size.w, grab.y * size.h));\E/let corner = pointer;/' \
    overview::drag tests::overview_drag
brk "a change of aim starts from the aim's size" $src/overview/drag.rs \
    's/\Qfrom: self.size(size(self.aim), now),\E/from: size(self.aim),/' \
    overview::drag tests::overview_drag
brk "it always eases" $src/overview/drag.rs \
    's/\Qif !self.animated {\E/if false {/' \
    overview::drag tests::overview_drag::without
brk "it never shrinks over a cell" $src/overview/mod.rs \
    's/\QSome((open, at)) if open.bar.over_cells(at) =>\E/Some((open, _)) if false =>/' \
    tests::overview_drag
brk "it takes the first output's scale everywhere" $src/overview/mod.rs \
    's/\QAim::Scale(drag::cell_scale(open.bar.cells[0], output))\E/Aim::Scale(drag::cell_scale(self.overview[0].bar.cells[0], output_geometry(&self.overview[0].output).size))/' \
    tests::overview_drag::a_drag_onto
brk "a drop isn't eased" $src/overview/mod.rs \
    's/\Qself.overview_drop = Some(Dropping::new(id, from, cell, self.now()));\E//' \
    tests::overview_drag
brk "the picture draws a dropped window before it's there" $src/overview/mod.rs \
    's/\Q.filter(|id| dropping != Some(*id))\E//' \
    tests::overview_drag::a_drop_on_another
brk "the slot draws a dropped window before it's there" $src/scene.rs \
    's/\Q && dropping != Some(**id)\E//' \
    tests::overview_drag::a_drop_on_nothing
brk "the row of cells is the cells alone" $src/overview/bar.rs \
    's/\Qfirst.merge(*last).to_f64().contains(point)\E/self.cell_at(point).is_some() \&\& first != last/' \
    overview::bar
brk "any switch cuts a drop" $src/overview/mod.rs \
    's/\Qself.policy.active_workspace_on(output) == workspace\E/true/' \
    tests::overview_drag::a_switch_to_another
brk "a drop with nowhere to go isn't cut" $src/overview/mod.rs \
    's/\Qswitched || self.overview_drop_to(&drop).is_none()\E/switched/' \
    tests::overview_drag::a_switch_away
brk "the grabbed point is found again from the slot" $src/overview/mod.rs \
    's/\Q(shrink.grab, shrink.size(to, now))\E/(drag::grab_point(slot, drag.from), shrink.size(to, now))/' \
    tests::overview_drag::the_grabbed_point
brk "a switch to its workspace doesn't cut a drop" $src/overview/mod.rs \
    's/\Qswitched || self.overview_drop_to\E/self.overview_drop_to/' \
    tests::overview_drag::a_switch_to_the
brk "a press on the window doesn't cut a drop" $src/overview/mod.rs \
    's/\Qif dropping.is_some() {\E/if false \&\& dropping.is_some() {/' \
    tests::overview_drag::a_press
brk "the Overview closing doesn't cut a drop" $src/overview/mod.rs \
    's/\Q        self.overview_shrink = None;
        self.overview_drop = None;
        let outputs = &self.outputs;\E/        let outputs = &self.outputs;/' \
    tests::overview_drag::the_overview_closing
brk "Esc doesn't ease the window back" $src/overview/mod.rs \
    's/\Q            self.refresh_overview();
            self.overview_dropped(dragged);\E/            self.refresh_overview();/' \
    tests::overview_drag::after_esc

exit $status
