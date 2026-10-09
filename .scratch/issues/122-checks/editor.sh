#!/usr/bin/env bash
# Opens a window of the editor in the session:  editor.sh <work> <argument> ...
# The editor is EDITOR_COMMAND, `code` by default. Its user data is <work>/user, and its
# windows' pages can be read on the DevTools port 9322.
work=$1; shift
exec "${EDITOR_COMMAND:-code}" --ozone-platform=wayland --user-data-dir="$work/user" \
  --extensions-dir="$work/extensions" --builtin-extensions-dir="$work/builtin" \
  --remote-debugging-port=9322 \
  --skip-welcome --skip-release-notes --disable-workspace-trust "$@" \
  >>"$work/out/editor.log" 2>&1
