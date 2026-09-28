# Packaging

Type: grilling
Status: open
Blocked by: 11, 12
Map: [Camino v0.1](../map.md)

## Question

How does 'MATE on Camino' get installed? Decide:

- the first target (Arch and the AUR?) and the package split: Camino itself, the palette helper, the locker if [12](12-screen-locking.md) ships one, and the patched MATE components;
- how carried patches are maintained and rebased, and which MATE baseline they apply to. The list so far:
  - mate-session: `GDK_BACKEND` unset and no `windowmanager` component (08);
  - mate-panel: Alt+F1/F2, Show Desktop, the pager's coordinates and layout keys, the minimise rectangle, and Shut Down (06, 08, 09);
  - mcc: WM identity and touchpad (06);
  - msd: media-keys, keybindings, OSD, background, and the xsettings mirror (06, [ADR 0005](../../../docs/adr/0005-msd-mirrors-mate-settings-into-gnome-schemas.md));
  - mate-screenshot: `<Alt>Print` (05).
- the MATE package conflicts, or the parallel installs, those patches need, given that they fall back to normal behaviour without Camino;
- the files Camino ships: `mate-camino.desktop` (08), the `org.mate.Camino` schema (09), the `50-camino-*.xml` keybinding manifests (05), and the capplet `.desktop` overrides (06);
- runtime dependencies, including Marco's schemas (06, 09).
