#!/usr/bin/env bash
# CMN-111 (ticket 254): shows that each new test can fail. Each break changes one thing in
# the code, runs the tests it should fail, and puts the file back from git. Run it in the
# OrbStack machine from the worktree, with the work committed:
#
#   orb -m camino bash -c 'cd <worktree> && .scratch/checks/CMN-111/break.sh'
set -u
cd "$(git rev-parse --show-toplevel)"
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$HOME/target-cmn-111}
ulimit -n "$(ulimit -Hn)"
src=caminito/src
tests=tests::open_close_animation_effects
status=0

# brk <name> <file> <perl substitution> <test filter>...
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
        echo "ok: $name fails $(grep -c '^    tests::' <<<"$out") test(s)"
    else
        echo "NOT CAUGHT: $name"
        tail -5 <<<"$out"
        status=1
    fi
    git checkout -q -- "$file"
}

brk "a window that opens mid-slide doesn't arrive with its workspace" $src/workspace_slide.rs \
    's/\Qslide.arriving.insert(window);\E/let _ = (slide, window);/' \
    $tests::during_a_slide
brk "with the animation off a window that maps mid-slide arrives all the same" $src/open_close_animation.rs \
    's/\Q        if self.settings.open_close_animation {\E/        if true {/' \
    $tests::with_the_animation_off
brk "a picture doesn't slide with its workspace" $src/open_close_animation.rs \
    's/\Qreturn slide.workspace_shifts(workspace, now);\E/return vec![Point::default()];/' \
    $tests::during_a_slide $tests::a_closing_windows_picture
brk "the scene draws a picture where its window was" $src/scene.rs \
    's/\Qlet at = drawn.loc + shift - output_geometry.loc;\E/let at = drawn.loc - output_geometry.loc;/' \
    $tests::a_closing_windows_picture
brk "a window that a slide takes away closes at once" $src/open_close_animation.rs \
    's/\Q&& (policy.is_showing(id) || slid())\E/&& policy.is_showing(id)/' \
    $tests::during_a_slide
brk "they run during the Overview's transition" $src/open_close_animation.rs \
    's/\Qself.policy.overview().is_some() || !self.overview.is_empty();\E/self.policy.overview().is_some();/' \
    $tests::neither_runs_during
brk "they run under the lock" $src/open_close_animation.rs \
    's/\Q!overview && !self.lock.is_locked()\E/!overview/' \
    $tests::locking
brk "an opening that runs isn't ended" $src/open_close_animation.rs \
    's/\Q            animations.running.clear();\E//' \
    $tests::neither_runs_while $tests::locking
brk "a picture that runs isn't ended" $src/open_close_animation.rs \
    's/\Q            runs && !output_left\E/            !output_left/' \
    $tests::neither_runs_while $tests::locking
brk "a picture outlives its output" $src/open_close_animation.rs \
    's/\Q            runs && !output_left\E/            runs/' \
    $tests::an_output_removed

exit $status
