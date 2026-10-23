# 43. A Session's desktop is `Caminito:MATE`, so the portal can pick Caminito's backend

Date: 2026-10-23
Status: Accepted

xdg-desktop-portal chooses its backends from `<desktop>-portals.conf`, by `XDG_CURRENT_DESKTOP`. A Session sets it to `MATE`, and `mate-portals.conf`, another package's file, sends everything to the GTK portal, which has no ScreenCast outside GNOME. So a Session sets `XDG_CURRENT_DESKTOP=Caminito:MATE`, and Caminito ships `caminito-portals.conf`: GTK by default, `caminito` for ScreenCast and Screenshot (ADR 0042). Desktop files shown only in MATE still match, because the variable is a list. Patching or replacing `mate-portals.conf` in packaging was rejected: it fights the package that owns it, on every distribution, and would change X11 MATE's portals too.
