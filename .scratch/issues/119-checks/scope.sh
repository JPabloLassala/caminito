#!/usr/bin/env bash
# Whether a setting read with no scope is what VS Code's `config.*` key holds. It opens a
# folder and then a multi-root workspace whose settings differ by layer (the user's, the
# workspace's, a folder's, a language's), with a text file of the layered folder in front.
# Each is opened twice: with a native title bar, where VS Code writes what its own menubar
# checks to `lastKnownMenubarData`, and with the custom one, where the extension serves the
# menu. It prints every toggle the two disagree on, and how many they agree on.
#
#   scope.sh <work>      in the session up.sh left up, with no editor of its own open
here=$(dirname "$(realpath "$0")"); work=$1; q=$work/scope
rm -rf "$q"; mkdir -p "$q"/{a,c,d}/.vscode
echo text >"$q/a/x.txt"; echo text >"$q/d/y.txt"
cat >"$q/a/.vscode/settings.json" <<'JSON'
{
  "files.autoSave": "off",
  "editor.renderWhitespace": "all",
  "[plaintext]": { "editor.stickyScroll.enabled": false, "editor.renderControlCharacters": false },
  "breadcrumbs.enabled": false,
  "workbench.editor.showTabs": "none"
}
JSON
cat >"$q/cd.code-workspace" <<'JSON'
{
  "folders": [{ "path": "c" }, { "path": "d" }],
  "settings": { "files.autoSave": "off", "editor.renderWhitespace": "boundary", "editor.columnSelection": true }
}
JSON
cat >"$q/d/.vscode/settings.json" <<'JSON'
{
  "files.autoSave": "afterDelay",
  "editor.renderWhitespace": "none",
  "editor.minimap.enabled": true,
  "editor.stickyScroll.enabled": false,
  "editor.renderControlCharacters": false,
  "breadcrumbs.enabled": false
}
JSON
settings() {
  mkdir -p "$1/User"
  cat >"$1/User/settings.json" <<JSON
{
  "window.titleBarStyle": "$2",
  "window.menuBarVisibility": "classic",
  "workbench.startupEditor": "none",
  "update.mode": "none",
  "telemetry.telemetryLevel": "off",
  "caminito.globalMenu.offerToHideMenuBar": false,
  "files.autoSave": "afterDelay",
  "editor.minimap.enabled": false,
  "[plaintext]": { "editor.minimap.enabled": true, "editor.renderWhitespace": "trailing" },
  "workbench.statusBar.visible": false,
  "workbench.activityBar.location": "top",
  "workbench.editor.editorActionsLocation": "default"
}
JSON
}
for state in folder workspace; do
  if [ $state = folder ]; then target=("$q/a" "$q/a/x.txt"); else target=("$q/cd.code-workspace" "$q/d/y.txt"); fi
  for bar in native custom; do
    user=$q/$state-$bar; settings "$user" $bar
    "$here/in.sh" "$work" "$here/editor.sh" "$work" "$user" "${target[@]}"
    # Up to a minute for the claim or for VS Code's first write, then for both to settle.
    for _ in $(seq 60); do
      sleep 1
      if [ $bar = custom ]; then
        "$here/in.sh" "$work" python3 "$here/look.py" served 2>/dev/null | grep -q toggle && break
      else
        grep -q lastKnownMenubarData "$user/User/globalStorage/storage.json" 2>/dev/null && break
      fi
    done
    sleep 5
    if [ $bar = custom ]; then
      "$here/in.sh" "$work" python3 "$here/look.py" served 2>/dev/null >"$q/$state-served.json"
    else
      cp "$user/User/globalStorage/storage.json" "$q/$state-native.json"
    fi
    pkill -f -- "--user-data-dir=$user"; sleep 2
  done
  python3 - "$q/$state-native.json" "$q/$state-served.json" "$state" <<'PY'
import json, sys
native, served, state = sys.argv[1:]
plain = lambda label: label.replace("&&", "")
checked = {}
def walk(items, path):
    for item in items:
        here = path + [plain(item.get("label", ""))]
        checked[" > ".join(here)] = item.get("checked", False)
        walk(item.get("submenu", {}).get("items", []), here)
for name, menu in json.load(open(native))["lastKnownMenubarData"]["menus"].items():
    walk(menu["items"], [name])
toggles = list(json.load(open(served)).values())[-1]
same = 0
for path, (kind, on) in toggles.items():
    if path not in checked:
        print(f"{state}: VS Code's menubar has no {path}")
    elif checked[path] != (on == 1):
        print(f"{state}: {path} ({kind}) is {'on' if on else 'off'} in the served menu and {'on' if checked[path] else 'off'} in VS Code's")
    else:
        same += 1
print(f"{state}: {same} of {len(toggles)} toggles are VS Code's; on: {sorted(p.split(' > ', 2)[-1] for p, (_, on) in toggles.items() if on)}")
PY
done
