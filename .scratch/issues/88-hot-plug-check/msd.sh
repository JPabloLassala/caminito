#!/bin/bash
# Runs inside dbus-run-session: only the background plugin, on a scratch dconf.
for p in a11y-keyboard a11y-settings clipboard datetime housekeeping keybindings keyboard media-keys mouse mpris rfkill smartcard sound typing-break wlrandr xrandr xrdb xsettings; do
  gsettings set org.mate.SettingsDaemon.plugins.$p active false
done
gsettings set org.mate.background picture-filename ''
gsettings set org.mate.background color-shading-type solid
gsettings set org.mate.background primary-color '#ff0000'
G_MESSAGES_DEBUG=all WAYLAND_DEBUG=1 exec ${MSD:-/usr/lib/mate-settings-daemon/mate-settings-daemon} --debug --replace
