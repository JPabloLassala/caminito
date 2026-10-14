# The Power applet in the default panel, and the packaging

Epic: power-applet
Type: implementation
Status: ready-for-agent
Blocked by: 261, 263, 264, 265
Spec: [Power applet](../power-applet/spec.md)

## What to build

"mate-power-manager's own icon" and the packaging in [the spec](../power-applet/spec.md).

- **The default panel:** the Power applet is in a new session's top panel beside the Bluetooth applet, the same way 261 chooses for that one.
- **mpm's icon:** a gsettings override sets `org.mate.power-manager icon-policy` to `never`, so X11 MATE doesn't show two battery icons.
- **Arch:** `caminito-git` builds and installs the applet. It depends on `upower` and optionally on `power-profiles-daemon`.
- **Debian:** `packaging/debian/caminito` does the same.
- **README:** the Power applet, and that mpm's icon is off by default.
- **[checklist.md](../../docs/testing/checklist.md):** unplugging and replugging, a profile change seen in `powerprofilesctl`, the charge limit, and Stay awake.

## Acceptance criteria

- [ ] A fresh user on a Session gets the applet in the top panel, and X11 MATE shows only one battery icon.
- [ ] `makepkg` and the Debian build install the applet and the override.
- [ ] The README and the checklist are updated.
