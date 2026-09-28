# Session launch model

Type: grilling
Status: resolved
Blocked by: 02
Map: [Camino v0.1](../map.md)

## Question

How does a 'MATE on Camino' session start and stop? Is it a wayland-sessions .desktop file, does Camino launch mate-session or the other way round, what environment is exported (WAYLAND_DISPLAY, DISPLAY, XDG_CURRENT_DESKTOP, the systemd/dbus activation environment), what's the start-up order relative to XWayland and mate-settings-daemon, and what's the logout and shutdown handshake?

## Answer

Decided in a grilling session on 2026-09-28. Inputs were the [MATE Wayland audit](02-mate-wayland-audit.md) (branch `research/mate-wayland-audit`), what [06](06-v01-protocol-and-integration-set.md) and [07](07-decoration-strategy.md) handed on, and a source read during the session of mate-session-manager `57a9178`, mate-submodules `4074560`, caja `a57395d`, mate-panel `58ac6c1`, mate-settings-daemon `927e9dc`, mate-menus `182ff87`, GLib `36c60f0`, niri `1f03391`, labwc `c2a95cf` and cosmic-session `1e4f10b`. The relevant facts are quoted inline.

The hard-to-reverse part, running mate-session as Camino's child on XWayland with a carried patch, is recorded in [ADR 0004](../../../docs/adr/0004-mate-session-as-camino-child-on-xwayland.md).

### Process tree

- **Session entry:** `/usr/share/wayland-sessions/mate-camino.desktop`, with `Name=MATE on Camino`, `Exec=camino --session` and `DesktopNames=MATE`. There's no wrapper script and no systemd unit. The file name avoids a clash with the Wayfire session's `MATE.desktop`. mate-session needs nothing from the display manager (no `X-GDM-SessionRegisters`), but it does need to already be inside a logind session.
- **Camino is the parent.** `--session` spawns `mate-session` by default and takes an override (`--session=CMD`). Plain `camino`, without `--session`, runs with no session, for development.
- **Helpers:** Camino supervises only its own helpers: Xwayland, `camino-gtk-palette` (07) and the ext-session-lock locker (06). Each is restarted with backoff. MATE components are mate-session's job, since it restarts them through `X-MATE-AutoRestart`.
- **What ends the session:** mate-session exiting ends it, and Camino exits with its status. There is no crash recovery for Camino in v0.1, so its death ends the session. Camino has **no quit binding**. Its only exit triggers are mate-session exiting and SIGTERM/SIGINT.

### XWayland

- **Eager:** Xwayland starts before any session child. Camino waits for `-displayfd` before exporting `DISPLAY`. Lazy start would gain nothing, because msd's xsettings plugin connects at once and mate-session itself runs on X11.
- **On a crash,** Camino restarts Xwayland **on the same display number** with backoff, so `DISPLAY` stays valid. X11 clients die. msd's xsettings plugin does not reconnect after an X restart; under Camino, Camino serves XSETTINGS instead (corrected by [11](11-xwayland-integration.md)). After repeated failures, Camino logs the failure and carries on without X.
- **If Xwayland never comes up,** Camino still spawns mate-session, with no `DISPLAY`, and logs loudly rather than refusing to start the session.

### How mate-session runs

- **mate-session runs on XWayland, with `GDK_BACKEND=x11`.** It isn't Wayland-safe: the logout and shutdown dialogs call `gdk_x11_get_server_time` (`gsm-manager.c:3500`, `:3526`), and `msm-gnome.c` and `gsm-inhibit-dialog.c` use X error traps. Running on X11 also avoids the `DISPLAY=$WAYLAND_DISPLAY` bug (`main.c:699-703`). Its dialogs are X11 windows, and Camino decorates them like any other.
- **`--disable-acceleration-check`.** Camino already needs working GL. The check can only get it wrong, by setting `LIBGL_ALWAYS_SOFTWARE` for the whole session (`main.c:667-690`).
- **Carried mate-session patch**, active only when `MATE_WM_NAME=Camino`:
  - It calls `g_unsetenv("GDK_BACKEND")` right after `gtk_init` (`main.c:637`). Without this, the variable reaches every child (libegg copies `g_listenv()`, `eggdesktopfile.c:1050-1071`). It also reaches the D-Bus and systemd environments, because mate-session exports its whole environment at `main.c:651-654`.
  - It skips the `windowmanager` required component in `append_required_apps` (`main.c:237-297`), so Marco isn't started. The WINDOW_MANAGER phase then ends at once.
- **Why not a gschema override:** `XDG_CURRENT_DESKTOP` must be exactly `MATE`. caja does `g_strcmp0(…, "MATE")` (`caja-application.c:1900-1906`), and mate-menus compares the whole string against each `OnlyShowIn` entry (`desktop-entries.c:94-119`). So `Camino:MATE`, and with it a `[org.mate.session:Camino]` override, would lose the desktop and hide every `OnlyShowIn=MATE` menu entry. The `--autostart DIR` option was also rejected, because it replaces the default and required-apps logic entirely (`main.c:748-755`).

### Environment

Camino exports these variables before it spawns anything:

| Variable | Value |
|---|---|
| `WAYLAND_DISPLAY` | Camino's socket |
| `DISPLAY` | Xwayland's display, once `-displayfd` reports |
| `XDG_SESSION_TYPE` | `wayland` |
| `XDG_CURRENT_DESKTOP` | `MATE`, only if the display manager didn't already set it from `DesktopNames` |
| `MATE_WM_NAME`, `MATE_WM_KEYBINDINGS` | `Camino` (06) |
| `XCURSOR_THEME`, `XCURSOR_SIZE` | read once at start-up from `org.mate.peripherals-mouse`, for non-GTK clients and Xwayland |

- **Nothing forces toolkit backends.** Qt and SDL choose their own; there's no `QT_QPA_PLATFORM`.
- **Where the variables go.** Camino sets them in its own environment and pushes them into the D-Bus activation environment (`UpdateActivationEnvironment`) and the systemd user manager (`SetEnvironment`). It waits for both calls to finish before spawning.
- **mate-session exports again.** At start-up it exports its whole environment to both places (`gsm-util.c:518-661`). Camino's own export exists for what runs before mate-session (the palette helper, and bare `camino`) and so Camino knows what to clean up.
- **On exit,** Camino unsets `WAYLAND_DISPLAY`, `DISPLAY`, `MATE_WM_*` and `XCURSOR_*` in the systemd user manager, so they can't leak into the next login.
- **No `graphical-session.target` in v0.1.** Nothing in the v0.1 scope is `PartOf=` it. It can be added later, as niri and cosmic do.

### Start-up order

1. Backend up and outputs lit, each in its preferred mode (06).
2. Wayland socket open.
3. Xwayland and `camino-gtk-palette` start in parallel. The palette helper needs only the socket and dconf, and the on-disk palette cache covers the gap (07).
4. Wait for Xwayland's `-displayfd`.
5. Export the environment.
6. Spawn `mate-session --disable-acceleration-check` with `GDK_BACKEND=x11`.
7. mate-session's phases (`gsm-manager.h:58-74`) run as follows. Each phase before Application waits for its apps to register, for up to 30 s.
   - **Initialization:** msd. It re-applies `monitors-wayland.xml`, which may mean a second modeset (a known gap from 06).
   - **WindowManager:** empty.
   - **Panel:** mate-panel.
   - **Desktop:** caja.
   - **Application:** the notification daemon, the polkit agent and user autostart.

### Logout and shutdown

- **Log Out:**
  1. The panel calls `org.gnome.SessionManager.Logout` (`panel-action-button.c:199-222`).
  2. QueryEndSession and inhibitors run, then EndSession with a 30 s timeout (`gsm-manager.c:808-870`, `:1546-1575`).
  3. mate-session stops its registered clients and exits 0.
  4. Camino tears down.
- **Shut Down / Reboot:** mate-panel's Wayland path bypasses mate-session and runs `systemctl` itself whenever GDK is on Wayland (`panel-action-button.c:250-350`, `:423-436`).
  - **Carried mate-panel patch** (about 10–15 lines): take the session-manager path whenever `org.gnome.SessionManager` has an owner. Shut Down then gets the confirm dialog and inhibitors, as on X11.
  - mate-session calls logind `PowerOff`/`Reboot` synchronously *before* it exits (`gsm-manager.c:458-535`), so systemd's SIGTERM reaches Camino during or after that.
- **Camino's teardown**, when mate-session exits:
  1. SIGTERM its helpers and Xwayland.
  2. Close the Wayland socket. MATE processes that mate-session didn't stop (it stops only registered clients) die when their connection drops.
  3. Unset the environment in systemd.
  4. Restore the VT.
  5. Exit with mate-session's status.
- **Camino getting SIGTERM/SIGINT** (the display manager or system shutdown): it forwards SIGTERM to mate-session, which does a forced logout (`main.c:458-465`), waits up to 5 s for it to exit, then runs the same teardown.

### Handed on

- **Packaging:**
  - the `mate-camino.desktop` session file;
  - the carried mate-session patch (`GDK_BACKEND` unset and no `windowmanager` component, both keyed on `MATE_WM_NAME=Camino`);
  - the carried mate-panel shutdown patch.
- **XWayland fog:** Camino's own X connection (bells, struts, EWMH atoms) has to reconnect after an Xwayland restart.
- **Development loop fog:** bare `camino`, without `--session`, is the nested and test entry point.
- **Screen locking fog:** the locker is a Camino helper, supervised like the others.
- **Later:** `graphical-session.target`, and crash recovery for Camino.
