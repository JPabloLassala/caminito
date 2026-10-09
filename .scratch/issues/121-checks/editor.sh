#!/usr/bin/env bash
# Opens a window of the editor in the session:  editor.sh <work> <user data> <path> ...
# The editor is EDITOR_COMMAND, `code` by default.
work=$1; user=$2; shift 2
exec "${EDITOR_COMMAND:-code}" --ozone-platform=wayland --user-data-dir="$user" \
  --extensions-dir="$work/extensions" --builtin-extensions-dir="$work/builtin" \
  --skip-welcome --skip-release-notes --disable-workspace-trust --new-window "$@" \
  >>"$work/out/editor.log" 2>&1
