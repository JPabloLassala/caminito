#!/bin/bash
# Breaks each part of 248 in turn on purpose, runs the tests that should catch it, and puts
# the file back. Run from the worktree's root: 248-break.sh
# Each break should show FAILED tests. 6 and 7 break what was there before 248, which
# the new tests keep. CARGO_TARGET_DIR is left as the caller has it.
set -u
ulimit -n "$(ulimit -Hn)"
unset DISPLAY

# break <file> <old> <new> <cargo test args...>
brk() {
    local file=$1 old=$2 new=$3
    shift 3
    cp "$file" "$file.bak"
    if ! python3 -c "import sys;p=sys.argv[1];s=open(p).read();assert sys.argv[2] in s, 'not found';open(p,'w').write(s.replace(sys.argv[2],sys.argv[3],1))" "$file" "$old" "$new"; then
        mv "$file.bak" "$file"
        echo "  the break doesn't apply"
        return
    fi
    touch "$file"
    dbus-run-session -- cargo test "$@" 2>&1 | grep -E "^error|test result|FAILED" | sort -u | head -20
    mv "$file.bak" "$file"
    touch "$file"
}

policy=caminito-policy/src/policy/overview.rs
compositor=caminito/src/overview/mod.rs

echo "1. A workspace button does nothing in the Overview:"
brk $compositor "_ if switch.is_some() => {" "_ if false => {" \
    -p caminito --lib -- tests::overview_buttons

echo "2. A click on another output's cell leaves the selection and the Active output:"
brk $policy "if output != own && self.outputs" "if false && self.outputs" \
    -p caminito-policy --lib -- tests::overview
brk $policy "if output != own && self.outputs" "if false && self.outputs" \
    -p caminito --lib -- tests::overview_buttons

echo "3. The output is made Active before the switch, which then takes the focus:"
brk $policy "        self.switch_output_workspace(output, workspace);
        if output != own" "        if output != own {
            self.active_outputs.insert(seat, output);
        }
        self.switch_output_workspace(output, workspace);
        if output != own" \
    -p caminito-policy --lib -- tests::overview

echo "4. Opening leaves the Active output where a press put it:"
brk $policy "        self.active_outputs.insert(seat, output);
        let workspace = self.active_workspace_on(output);
        let windows = self.overview_windows(output, workspace);
        let selected" "        let workspace = self.active_workspace_on(output);
        let windows = self.overview_windows(output, workspace);
        let selected" \
    -p caminito-policy --lib -- tests::overview

echo "5. A click on the selection's own output's cell is taken for another output's:"
brk $policy "if output != own && self.outputs" "if self.outputs" \
    -p caminito-policy --lib -- tests::overview

echo "6. A button bound to the Overview no longer closes it:"
brk $compositor "            _ if is_shortcut => caminito.policy.close_overview(),
" "" \
    -p caminito --lib -- tests::overview_buttons

echo "7. A press on the empty area no longer closes it:"
brk $compositor "            None => caminito.policy.close_overview(),
        }
        self.caminito.refresh_overview();" "            None => {}
        }
        self.caminito.refresh_overview();" \
    -p caminito --lib -- tests::overview_buttons

