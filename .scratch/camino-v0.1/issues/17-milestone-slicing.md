# Milestone slicing

Type: grilling
Status: resolved
Blocked by:
Map: [Camino v0.1](../map.md)

## Question

Order v0.1 into milestones, each ending in something runnable. For example: nested compositor with xdg-shell; decorations; XWayland; layer-shell and mate-panel; the full session; then parity polish. For each milestone decide its scope, its exit criterion, and which decisions from tickets 01–16 it realises. Also decide what counts as 'v0.1 done'. This is the last ticket on the map: when it resolves, the architecture spec and milestone plan are complete.

## Answer

Decided on 2026-09-28. The user asked for the recommended answer to every question to be taken, so there was no round-by-round grilling. The inputs were what [14](14-development-and-testing-loop.md), [15](15-crate-and-module-architecture.md) and [16](16-packaging.md) handed on, and the decisions of 01–16 as they stand.

There's no ADR. The order is cheap to change, since every milestone builds on the architecture 15 fixed rather than on the milestone before it.

### Summary

| # | Milestone | Runs as | Ends with |
|---|---|---|---|
| M1 | Nested compositor | bare `camino`, nested under winit | GTK3, GTK4 and Qt apps mapped, focused, stacked, moved and resized on several workspaces; CI, the headless fixture, the policy proptests and `camino-git` all in place |
| M2 | Marco frames | nested | SSD windows framed from the Marco theme, matching the goldens for all ten themes at scales 1 and 2, with live settings |
| M3 | XWayland | nested | X11 apps managed and framed like native ones, Camino's X connection and XSETTINGS, the restart path, the X11 test tier |
| M4 | On the metal | bare `camino` on a tty | DRM/KMS and libinput, several outputs at fractional scales, wlr-output-management, input settings and xkb from gsettings |
| M5 | MATE session | `camino --session`, on a tty as `caminotest` and from the display manager | "MATE on Camino" logs in and out, with mate-panel (window list and pager), caja's desktop and msd; three patched packages |
| M6 | MATE integrations | session | every row of 06's integration table working; the other two patched packages |
| M7 | Lock, idle and accessibility | session | the lock fails closed, idle, blanking and DPMS work, and Orca and AccessX work; the developer starts using Camino daily |
| M8 | Marco parity | session | every `org.mate.Marco.*` key and all 103 actions honoured, never implemented, or on the roadmap |
| M9 | Flatpak | session, and nested | a Flatpak app runs in its sandbox and is offered no privileged global; then v0.1.0 is tagged |

### How milestones work

- **A milestone** is a slice of v0.1 that ends in something the developer runs, and closes when its exit criterion holds (CONTEXT.md).
- **Every exit criterion has three parts:**
  1. **Runnable:** the scenario the milestone names works, run by hand the way the "Runs as" column says.
  2. **Tested:** CI is green on push, with the milestone's automated tests from 14's layers.
  3. **Checked:** every checklist item tagged with this milestone or an earlier one that still applies has been run, and the results are recorded in the closing commit message (14).
- **The closing commit** is titled `Close M<n> <name>`. It also moves the milestone's checklist tags, if any item was deferred, and it's the only commit that says a milestone is done.
- **Features grow in place.** The policy model, the scene's layer order and the refresh pass are built in M1 in their final shape (15), and later milestones add to them. The only stand-ins are three small ones in M1, each named where it's replaced: hard-coded default bindings (until M8), a fixed workspace count (until M2) and decorations answered client side (until M2).
- **Checklist tags are `M1` to `M9`.** An item that needs something from a later milestone (msd, logind, Orca, two monitors) is tagged with that milestone, not the one that wrote the code.
- **Packaging keeps up from M1.** Every milestone's code is installed through `camino-git` on the developer's machine, never only run from `target/` (16).

### M1 Nested compositor

**Scope:**

- **Workspace and tooling (15, 14):** the virtual Cargo workspace with its four crates, `rust-toolchain.toml`, clippy, rustfmt and cargo-deny, the pinned Smithay `rev`, `tools/dev-nested`, `logging` with `tracing` (stderr, and journald under `--session` later), and `docs/testing/checklist.md`.
- **Core (15):** `State { backend, camino }` in one calloop loop, the `Backend` enum with `Winit` and `Headless` (`cfg(test)`, pixman, virtual outputs at any scale), the refresh pass with no views yet, and the `scene` module's layer order with only the window layer filled.
- **Protocols (06):** the core set Smithay provides (xdg-shell, xdg-output, viewporter, fractional-scale, xdg-activation, primary-selection, the input protocols, cursor-shape, presentation-time and the other rendering protocols except drm-syncobj, which needs M4's DRM), and xdg-decoration and KDE server-decoration answering *client side* until M2.
- **Policy (05, 15):** `camino-policy` with `Policy<W: PolicyWindow>`, `TestWindow`, and `Mapped` implementing the trait. It covers Marco's default behaviour: click focus, stacking with Marco's layers, transient rules, the *automatic* placement mode, a fixed count of workspaces, maximise, minimise and fullscreen, and interactive move and resize from client requests and modifier-drag.
- **Input:** the keyboard path's stages in their final order (15), with only `input::bindings` and the client stage doing work. A hard-coded set of Marco's default bindings: workspace switching, close, maximise and Alt+Tab without the popup.
- **Tests (14):** the headless `Fixture` and the first `camino-test-client` (xdg-shell), insta snapshots of configure sequences, and the proptest invariants (one focus per seat, transients above parents, one workspace per window, stacking is a permutation).
- **CI (14):** the push workflow in `archlinux:latest`, and the weekly proptest and Smithay canary jobs.
- **Packaging (16):** the top-level `meson.build` (installing nothing yet), `camino-git`'s PKGBUILD, and the `makepkg` and `namcap` CI job.

**Exit:**

- `tools/dev-nested` runs the installed `camino`. gtk3-demo, gtk4-demo, a Qt app and foot all map, take focus by click, stack, move, resize, maximise and switch workspaces.
- The headless snapshots and the proptest invariants pass in CI.

**Checklist:** the first items for nested runs: the private bus, the scratch `XDG_CONFIG_HOME`, and Camino not touching the host session's systemd environment (14).

### M2 Marco frames

**Scope:**

- **Theme engine (07, ADR 0003):** `camino-theme` with everything Marco draws: the lenient parser from 04, every format version up to 3.5, the approximated `gtk_*` ops, and the built-in fallback theme.
- **Goldens (14, ADR 0009):** `tools/marco-frame-dump`, the regeneration script, the ten themes and their cases committed under `tests/goldens/`, and the comparisons in CI.
- **Palette (07, ADR 0003):** `helpers/camino-gtk-palette`, its stdout format and on-disk cache in `camino-theme`, the exact palette test, and the `helpers` supervisor (spawn, stderr to the journal, restart with backoff, give up).
- **Settings (09):** the GLib thread, the first full snapshot before anything maps, the pure translation, and the "unsupported key" log. M2 honours the decoration keys 07 lists, plus `num-workspaces` and `workspace-names` so M1's fixed count goes away.
- **Frames (07):** the `decoration` module with four cached strips per window, hit-testing, hover and press, the invisible resize border, cursors (theme and size applied live, loaded at each output's rounded-up scale, 09, 13), titles, window icons, and `action-*-titlebar`. xdg-decoration and KDE server-decoration now follow 07's policy, so MATE, plain GTK and Qt apps are SSD and header-bar apps stay CSD. Shade on SSD windows only.
- **Scale:** strips at the largest rounded-up scale of the outputs a window overlaps (13), tested with the headless backend's fractional outputs.
- **Debug (14):** the frame-debug toggle and the first `org.mate.Camino.Debug` dump, which brings up the `dbus` thread.

Shadows wait for M8 with the other effects, since `compositing-manager` governs them together (05).

**Exit:**

- Every golden case passes for all ten themes at scales 1 and 2, and the palette test passes.
- Nested, a GTK3 app gets a Menta frame, and a GTK4 header-bar app stays CSD. Changing `theme`, `titlebar-font` or `button-layout` with `gsettings` applies live, and a broken theme falls back to the built-in one with a single log line.

**Checklist:** frames at a nested scale of 2; a theme switch while windows are open.

### M3 XWayland

**Scope:** everything 11 put in v0.1, and 10's facts it rests on.

- **Spawn and restart (08, 11):** Xwayland started eagerly, `XCURSOR_*` through `envs`, and the restart on the same display.
- **Two X connections (11, 15):** `xwayland::xwm` maps XWayland windows into `Mapped` and policy with Marco's type, placement and focus rules. `xwayland::xconn` publishes the desktop atoms, work area and frame extents before `DISPLAY` is exported, reads struts, selects `XkbBellNotify`, and handles the root messages Smithay drops.
- **XSETTINGS (ADR 0006):** `xwayland::xsettings`, re-applied after every `start_wm`.
- **Scene:** the override-redirect layer.
- **Clipboard:** access gated on X11 focus (11).
- **X11 scale (ADR 0008):** the largest integer output scale, with msd's DPI formulas, published through XSETTINGS.
- **Tests (14):** the X11 tier with a real Xwayland and the `x11rb` client, including `-terminate` not firing and the restart path.

**Exit:**

- Nested, xterm, xeyes and a GTK3 app with `GDK_BACKEND=x11` map, get Marco frames, take focus and move. Their menus and tooltips sit in the override-redirect layer, and `_NET_FRAME_EXTENTS` and `_NET_WORKAREA` are right.
- Killing Xwayland brings it back on the same display with the root properties republished.
- The X11 tier passes in CI.

**Checklist:** an X11 app's copy and paste to and from a native app; `DISPLAY` only exported after the root properties exist.

### M4 On the metal

**Scope:**

- **Backend:** `backend::tty` with DRM/KMS, libinput, libseat, VT switching, hot-plug and drm-syncobj.
- **Outputs (13, ADR 0008):** the output lifecycle, the default-scale rule as a pure function with KWin's EDID cases and the developer's monitors, `window-scaling-factor` forcing an integer, and outputs placed left to right in their preferred modes (06).
- **wlr-output-management v4 (06):** the first protocol Camino builds, tested with `wlr-randr`. msd, its real user, arrives in M5.
- **Input settings (09):** the `org.mate.peripherals-*` keys to libinput, and `org.mate.peripherals-keyboard-xkb.kbd` to xkb, live, exactly as msd applies them on X11.
- **Headless tests (13, 14):** enter, leave and preferred scales for a window dragged across a 1.0 and a 2.0 output.

**Exit:** as `caminotest` on a spare tty, bare `camino` drives both of the developer's monitors at their default scales. `wlr-randr` changes modes, scales and positions. Mouse, touchpad and keyboard-layout settings changed with `gsettings` apply live, and switching VT away and back works.

**Checklist:** a GTK3 window dragged across a 1.0 and a 2.0 output (13's visual part); hot-plugging a monitor; the scale picked for each of the developer's monitors.

### M5 MATE session

**Scope:**

- **Session (08, ADR 0004):** `camino --session`, the `session` module's start-up order, the environment export with 14's systemd check, mate-session as Camino's child on XWayland, the session ending with it, `mate-camino.desktop`, and journald logging.
- **Protocols (06):** wlr-layer-shell, wlr-foreign-toplevel-management v3, ext-foreign-toplevel-list, read-only ext-workspace, the minimal `gtk_shell1`, and data-control, which waited for the other privileged globals. They're all hidden from `wp_security_context_v1` clients.
- **Screensaver names (12):** `dbus::screensaver` owns `org.mate.ScreenSaver` and `org.freedesktop.ScreenSaver` before mate-session starts, as 12 requires, so mate-screensaver's daemon exits and never draws an X11 lock. Until M7, `Lock` is logged and refused, so the session can't lock yet.
- **Views:** foreign-toplevel and ext-workspace in the refresh pass (15).
- **Scene:** the four layer-shell layers, with caja's `desktop` surface above msd's `mate-background` (06).
- **Patched packages (16, ADR 0011):** the three pkgbases the session can't start without, with their CI jobs and the weekly patch-on-head job:
  - `mate-session-manager-camino`: `GDK_BACKEND` unset, and no `windowmanager` component;
  - `mate-panel-camino` on its pinned master commit with libxfce4windowing: the minimise rectangle and Shut Down;
  - `mate-settings-daemon-camino`: the xsettings mirror-only path, the background plugin, and the typing-break skip.

  `camino-git` depends on these three by their real names. mcc and mate-utils wait for M6. The session starts without their patches, and a patched package with no patches would only be churn, so this narrows 16's "all five with the full session" on purpose.
- **Tests:** layer-shell, foreign-toplevel and ext-workspace through the test client, as a panel would use them.

**Exit:**

- From the display manager, and as `caminotest` on a tty, "MATE on Camino" starts mate-session. mate-panel shows the window list and the workspace switcher, caja draws the desktop icons, and msd sets the wallpaper and applies the saved display layout over wlr-output-management.
- Logging out through the panel ends the session, and Camino exits with mate-session's status.
- X11 MATE on the same machine still works with the patched packages installed (the fall-back rule, ADR 0011).

**Checklist:** X11 clipboard contents survive the app quitting (11); a login on the developer's own user under X11 MATE and the Wayfire session unchanged; logout and Shut Down from the panel.

### M6 MATE integrations

**Scope:** every row of 06's integration table not already done, with their carried patches.

- **`org.mate.Camino` (06, 15):** `ShowingDesktop` with Marco's per-workspace semantics, shared with Ctrl+Alt+D; `GetActiveToplevel`; `GrabAccelerators` and `AcceleratorActivated`.
- **The panel:** Alt+F1 and Alt+F2 through `org.mate.Panel` with the one-second popup grab, and the pager's layout through ext-workspace `coordinates` from the three `workspace-layout-*` keys (09).
- **Capture:** ext-image-capture-source with output and toplevel sources, and ext-image-copy-capture. Toplevel captures include the frame and leave out the shadow.
- **Bells:** the `gtk_shell1`, xdg-system-bell and X11 sources feeding one bell event (06). The visual and audible bell themselves come in M8.
- **Patches (16):**
  - `mate-panel-camino` gains the `org.mate.Panel` export, Show Desktop, the pager's coordinates and layout keys, and the workspace count and names;
  - `mate-settings-daemon-camino` gains media-keys, keybindings, the OSD on gtk-layer-shell, and touchpad;
  - `mate-control-center-camino` and `mate-utils-camino` arrive, with WM identity, touchpad, the X11 backend for Mouse and Keyboard, 5% scale steps, and `<Alt>Print`.

  `camino-git` now depends on all five.
- **Data files (05, 09, 16):** the `org.mate.Camino` schema and the `50-camino-*.xml` keybinding manifests, installed by the top-level Meson project.

**Exit:** in a session, each of these works: Alt+F1 and Alt+F2; the panel's Show Desktop and Ctrl+Alt+D agreeing; the pager's rows, names and count; the minimise animation target reaching Camino; volume, brightness and custom shortcuts from msd, with its OSD; mcc's Windows, Keyboard Shortcuts, Mouse, Keyboard and Display dialogs; `Print`, `<Shift>Print` and `<Alt>Print`.

**Checklist:** one item per row of 06's integration table, run in a session.

### M7 Lock, idle and accessibility

**Scope:**

- **Lock (12, ADR 0007):** the `lock` state machine, ext-session-lock, `helpers/camino-locker` ported from mate-screensaver's dialog with its themes and PAM service, `locker-command`, the restart and give-up paths, and the override-redirect layer and Xwayland keyboard grabs refused while locked.
- **Screensaver shim (12):** the names M5 reserved now drive the lock, idle and inhibit state, with `Unlock` refused. `dbus::logind` provides `Lock`, `Unlock`, `SetLockedHint`, and the sleep delay held until the lock is on screen.
- **Idle (06, 09):** `idle` with `org.mate.session idle-delay`, blanking and DPMS, ext-idle-notify, idle-inhibit, and mate-session's `IsInhibited`.
- **Accessibility (13):** `input::accessx` with sticky, slow and bounce keys and the toggle-key beeps, and `dbus::a11y`'s Keyboard monitor for Orca. Both keep working while locked.
- **Capture while locked (12):** image capture shows the lock, never the session behind it.
- **Tests (14):** killing the locker while locked, the give-up state, and the Keyboard monitor's allowlist, grabs and lone-modifier rule as headless tests with a D-Bus test client; AccessX as unit tests.

**Exit:**

- The session locks from the panel, from `org.mate.ScreenSaver`, on idle and before suspend. Killing `camino-locker` while locked leaves the session locked and brings it back.
- Idle blanks, then powers the outputs off, and a playing video inhibits both.
- Orca reads the panel and `camino-locker`, and sticky keys work.
- **The developer switches to "MATE on Camino" as their daily session from here.** A session that can't lock isn't safe to leave, so daily use waits for M7, and M8 is built inside a Camino session.

**Checklist:** the suspend race with a slow locker; `loginctl unlock-session` from a console; how the give-up state looks; Orca with the Keyboard monitor and on the lock screen (12, 13, 14).

### M8 Marco parity

**Scope:** everything from 05's v0.1 list not done earlier.

- **Policy:** every focus mode with auto-raise, every placement mode, snap-move, edge, corner and keyboard tiling with tile cycling, edge resistance, 1–36 and dynamic workspaces with wrap styles, above, below and sticky, the full window menu, and "Move titlebar on-screen".
- **Popups:** Alt+Tab with thumbnails and every option, the workspace popup, and switch sounds.
- **Effects:** shadows, the minimise animation, `compositing-manager` and `reduced-resources` (05, 07).
- **Bell:** the visual and audible bell.
- **Focus-stealing prevention:** *smart* and *strict* on xdg-activation tokens, with urgency feeding Alt+Tab and the window list.
- **The rest:** the "not responding" dialog and Force Quit, the zenity helper dialogs, `switch-panels` and `cycle-panels`, `show-desktop-skip-list`, and XWayland windows' above, below and sticky requests, which reach policy from M3 but take effect once those states exist here.
- **Bindings:** every one of the 103 actions bound from `org.mate.Marco.*` keybindings, replacing M1's hard-coded set.
- **Audit:** a parity table in the repo, `docs/parity.md`. It lists every key and action from 01's inventory with its status (honoured, never, left to another component, or unsupported) and the test or checklist item that covers it. The unsupported rows are the "later" roadmap (05, 09).

**Exit:** the parity table has no row that is still to do. The unsupported rows are exactly 09's roadmap and 05's "later" list, and a row moves there only with a reason written beside it.

**Checklist:** a pass over the parity table's rows that no automated test covers.

### M9 Flatpak

Added on 2026-10-07, by the developer's decision at M5's closing ([73](73-packaging-and-closing-m5.md)): the checklist's Flatpak item was tagged M5 and couldn't be run there, since the developer's machine has no Flatpak runtime, and Flatpak gets a milestone of its own instead of a line left not run.

**Scope:** Flatpak apps under Camino. So far it's the one checklist item, moved here from M5: the security-context protocol and the filter on the privileged globals are M5's ([68](68-data-control-and-privileged-globals.md)), with their headless tests, and what's left is the check with a real sandbox. What else a Flatpak app needs of Camino is found when the milestone starts, as each milestone's tickets are.

**Exit:** with a Flatpak runtime installed, an app in its sandbox shows a window, and the sandbox is offered none of the privileged globals.

**Checklist:** *A Flatpak app's sandbox is offered no privileged global*.

### v0.1 done

v0.1 is done when all of these hold at one commit:

1. **M1 to M9 are closed**, each with its closing commit.
2. **The full checklist** passes on that commit, and the results are recorded in its message.
3. **CI is green** on push and on the weekly jobs: every test tier, the goldens for all ten themes, and all six packages building with `makepkg` and `namcap`.
4. **Packaging (16):** `camino-git` and the five patched packages are installed on the developer's machine, and X11 MATE still works on it.
5. **Soak:** the developer has used "MATE on Camino" as their daily session for two weeks, and every crash, lock failure or data loss found in that time is fixed.
6. **The parity table** (M8) has nothing left to do outside the roadmap.

Then the commit is tagged `v0.1.0`, and a tagged `camino` package replaces `camino-git` (16).

### What each milestone realises

| Ticket | Realised in |
|---|---|
| 01 Marco feature inventory | M8's parity table |
| 02 MATE on Wayland audit | M5, M6 |
| 03 Smithay capability audit | M1 (pinned `rev`), M4, M5 |
| 04 Theme format | M2 |
| 05 Feature cut | M1 (core policy), M6 (integrations), M8 (the rest) |
| 06 Protocols and integration | M1 (Smithay's set), M4 (output management), M5 (layer-shell, foreign-toplevel, ext-workspace, `gtk_shell1`), M6 (the integration table), M7 (lock and idle) |
| 07 Decorations, ADR 0003 | M2; shadows in M8 |
| 08 Session launch, ADR 0004 | M5 |
| 09 gsettings mapping, ADR 0005 | M2 (settings thread), M4 (input), M5 (the msd mirror), M6 (`org.mate.Camino` schema), M7 (idle), M8 (the rest) |
| 10–11 XWayland, ADR 0006 | M3, with ADR 0008's X11 scale; struts and the bell source reach policy in M5 and M6 |
| 12 Screen locking, ADR 0007 | M7 |
| 13 HiDPI and accessibility, ADR 0008 | M2 (frame scale), M3 (X11 scale), M4 (output scales), M6 (5% steps), M7 (AccessX, Keyboard monitor) |
| 14 Development and testing, ADR 0009 | M1 (CI, fixture, proptests), M2 (goldens), M3 (X11 tier) |
| 15 Crate and module architecture, ADR 0010 | M1 |
| 16 Packaging, ADR 0011 | M1 (`camino-git`), M5 (three patched packages), M6 (the other two), v0.1 done (the tagged package) |

### Handed on

- **Implementation:** the map is complete. Each milestone's implementation tickets are written when it starts, not now, since each milestone's findings change what the next one needs. They go in the same local tracker, in `issues/` numbered from 18, with a `Milestone: M<n>` line beside `Blocked by:`.
- **Known risks, by milestone:**
  - **M2** is the largest single piece of work: a port of `theme.c` and `theme-parser.c` with golden-exact output.
  - **M5** depends on mate-session and mate-panel master, and msd 1.29.1, staying buildable at their pinned baselines (16).
  - **M3's restart path and M7's fail-closed lock** are the two places where a bug ends or exposes the session, so both have headless tests before their milestones close.
