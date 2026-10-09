#!/usr/bin/env bash
# Whether the importer loses an ItemsPropertiesUpdated that comes soon after a layout reply.
# Each round takes an entry out of Open Recent, which sends a LayoutUpdated for that menu,
# and <delay> ms after it gives Reopen Closed Editor, an item of the same menu, another
# keybinding, which sends an ItemsPropertiesUpdated. It prints how long after the importer's
# GetLayout the signal went out, and the shortcut the panel's item has 2 s later.
#
#   dropped.sh <work> <delay> ...
here=$(dirname "$(realpath "$0")"); work=$1; shift
key=0
for delay in "$@"; do
  key=$((key + 1))
  entry=$("$here/in.sh" "$work" python3 "$here/look.py" labels File "Open Recent" 2>/dev/null |
    python3 -c "import json,sys; print(next(l for l in list(json.load(sys.stdin).values())[0] if l.startswith('/tmp/caminito-vscode')))")
  mark=$(wc -l <"$work/out/bus.log")
  "$here/drive.sh" "$work" "
    const fs = require('node:fs');
    await vscode.commands.executeCommand('vscode.removeFromRecentlyOpened', '$entry');
    await vscode.commands.executeCommand('workbench.action.files.newUntitledFile');
    await sleep($delay);
    fs.writeFileSync('$work/user/User/keybindings.json', JSON.stringify([{ key: 'ctrl+alt+$key', command: 'workbench.action.reopenClosedEditor' }]));" >/dev/null
  sleep 2
  gap=$("$here/events.sh" "$work" "$mark" | awk '/GetLayout int32 [0-9]+ int32 1/ { at=$1 } /ItemsPropertiesUpdated/ && at { printf "%d ms", ($1 - at) * 1000; exit }')
  has=$(HERE=$here "$here/in.sh" "$work" python3 - 2>/dev/null <<'PY'
import runpy, sys
sys.argv = ["look.py", "none"]
look = runpy.run_path(sys.argv and __import__("os").path.join(sys.stdin and sys.argv[0] and __import__("os").environ["HERE"], "look.py"))
print(look["at"](["File", "Open Recent", "Reopen Closed Editor"]).get_key_binding(0).split(";")[-1])
PY
)
  echo "written $delay ms after the tab: the signal came ${gap:-?} after GetLayout; wanted <Primary><Alt>$key, the panel has $has"
done
