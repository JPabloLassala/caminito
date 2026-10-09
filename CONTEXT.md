# Caminito

Caminito is a Wayland compositor for the MATE desktop, built to be the Wayland counterpart of Marco.

## Language

**Caminito**:
The Wayland compositor for the MATE desktop. It is display server, window manager and compositor in one process.
_Avoid_: Caminito window manager, Caminito WM

**Marco**:
MATE's X11 window manager, the behavioural and visual reference for Caminito.
_Avoid_: Metacity (Marco's ancestor, only relevant for its theme format)

**Compositor**:
The single Wayland process that owns the displays and input and manages windows. On Wayland there is no separate window manager.
_Avoid_: Wayland window manager

**Carried patch**:
A change to a MATE component, or to a panel applet from outside MATE, that Caminito's packaging applies downstream and never sends upstream. It is gated on Caminito (`MATE_WM_NAME=Caminito`, or an owner of `org.mate.Caminito`), so it falls back to the component's normal behaviour under X11 MATE, the Wayfire session, or anything else that isn't Caminito.
_Avoid_: fork, upstream patch

**Patched package**:
An Arch package named `<pkgname>-caminito` that builds a MATE component, or a panel applet from outside MATE, with its carried patches and replaces the stock package through `provides` and `conflicts`.
_Avoid_: fork package, Caminito build of MATE

**Layer surface**:
A shell surface that sits in a fixed layer of an output rather than among ordinary windows: the panel, caja's desktop, the wallpaper, notification bubbles and the screenshot area picker.
_Avoid_: panel window, dock window

**Frame**:
The decoration Caminito draws around a window from the Marco theme: titlebar, buttons and borders, plus the invisible resize border. The shadow isn't part of the frame.
_Avoid_: decoration (for Caminito's own drawing), border, titlebar (for the whole)

**Frame extents**:
How far a window's frame reaches beyond the window on each side. The theme sets them from the window's type and its state: focused or not, maximised, shaded. A focus change can change them. They're zero for a CSD or fullscreen window.
_Avoid_: borders (the theme's term for one part of them), decoration size

**SSD window**:
A window that Caminito draws a frame for.
_Avoid_: decorated window

**CSD window**:
A window that draws its own titlebar and borders, such as a header-bar or libadwaita app. Caminito never draws a frame for it.
_Avoid_: undecorated window (that means a window with no decorations at all)

**Palette**:
The GTK3 theme's colours that Marco themes refer to: the window background and text colours per state, plus the theme's named colours, for the default and dark variants.
_Avoid_: colour scheme (that's the portal's light/dark preference)

**Helper**:
A process that Caminito itself starts and supervises: Xwayland, the palette helper and the locker. MATE components aren't helpers, because mate-session starts and restarts them.
_Avoid_: daemon, child (for MATE components)

**Session**:
A "MATE on Caminito" login, from the display manager starting Caminito to mate-session exiting. Caminito is its parent process, and mate-session exiting ends it.
_Avoid_: desktop session (ambiguous with the X11 MATE session)

**Unsupported key**:
A gsettings key Caminito reads but doesn't honour yet. It is logged once if the user has changed it from its default, and it is on the roadmap. It is not the same as a key Caminito will never implement, or a key that belongs to another MATE component.
_Avoid_: ignored key, missing setting

**XWayland window**:
A window of an X11 client running on XWayland that Caminito manages like any other toplevel, through Smithay's `X11Wm`.
_Avoid_: X window (ambiguous with the X protocol's window objects), legacy window

**Pending window**:
A toplevel or XWayland window that its client has asked to show, which Caminito doesn't manage yet because it has no content. It keeps the states it asks for meanwhile, such as maximised or fullscreen, and maps with them.
_Avoid_: unmapped window (in X11 that's a withdrawn one)

**Override-redirect window**:
An X11 menu, tooltip or popup that Caminito shows in its own layer but never manages or focuses. It gets the keyboard only through an Xwayland keyboard grab.
_Avoid_: unmanaged window, X11 popup

**Caminito's X connection**:
Caminito's own ordinary client connection to Xwayland, alongside Smithay's window-manager connection. It publishes the desktop, work-area and frame-extent properties, and handles the bell, struts and the root messages Smithay drops. It never writes a property Smithay maintains.
_Avoid_: second window manager, X helper

**Locker**:
The helper that draws the lock screen and checks the password: `caminito-locker` by default, MATE's unlock dialog on ext-session-lock. The lock itself is Caminito's; the locker can crash without unlocking the session.
_Avoid_: screensaver (Caminito only blanks), lock screen (for the program)

**Screensaver shim**:
Caminito's implementation of `org.mate.ScreenSaver` and `org.freedesktop.ScreenSaver`, which turns MATE's lock, activity and inhibit calls into Caminito's idle, blanking and lock state. mate-screensaver's daemon doesn't run under Caminito.
_Avoid_: mate-screensaver (the X11 program)

**Output**:
A monitor Caminito is drawing on, with a mode, a place in the layout and an output scale. A monitor that is turned off is a disabled output.
_Avoid_: display (that's the word the user reads in caminito-displays, and nowhere else), screen, head (the protocol's word)

**Primary output**:
The one output the user chooses as primary. The panel and caja's desktop go there, new windows open there when nothing says otherwise, and X11 apps see it as the RandR primary. When it is unplugged or disabled, the built-in panel stands in, else the leftmost output, until it returns.
_Avoid_: main output, default output

**Main output**:
The output a surface is mostly on, which decides the scale the surface is asked to render at.
_Avoid_: primary output (that's the user's choice; Smithay's name for this is "primary scanout output")

**Layout**:
The enabled outputs with their places, modes and output scales, and which one is the primary output. Its outputs are always contiguous: no gaps, no overlaps, and each touches another along a shared edge of at least 100 logical pixels.
_Avoid_: configuration (that's one client's request over output management), arrangement (that's the picture of the layout in caminito-displays's window, which the user drags the displays about in)

**Saved layout**:
A layout Caminito keeps for one set of connected monitors and restores when that set is connected again. A monitor is known by its make, model and serial, or by its connector when it reports none.
_Avoid_: profile, monitors file

**Dock**:
To place an output against the outline of the others, which is the only way an output is moved in a layout.
_Avoid_: snap (that's the finer alignment of matching edges while docking; centres don't snap)

**Heal**:
To make a layout contiguous again after a change that broke it: an output removed, added, turned off, or changed in size.
_Avoid_: repack, normalise

**caminito-displays**:
Caminito's display settings program, which replaces MATE's Display capplet in a Session.
_Avoid_: the capplet, display properties

**Output scale**:
The factor from logical to physical pixels for one output. It may be fractional. Caminito's default comes from KWin's rule, or from `window-scaling-factor` when that is 1 or 2, and Caminito's saved layout overrides it.
_Avoid_: DPI (that's the font setting), window scale (GTK's term)

**X11 scale**:
The single integer scale for every XWayland window: the largest rounded-up output scale. It is also published through XSETTINGS.
_Avoid_: Xwayland DPI, global scale

**Keyboard monitor**:
Caminito's `org.freedesktop.a11y.KeyboardMonitor` service, through which Orca watches and grabs keys. Only the client that owns `org.gnome.Orca.KeyboardMonitor` may use it.
_Avoid_: key logger, a11y grab

**AccessX**:
The keyboard accessibility features from `org.mate.accessibility-keyboard`: sticky, slow, bounce, toggle and mouse keys. On Wayland Caminito implements them in its keyboard path. On X11 the X server does.
_Avoid_: a11y keys, XKB controls (that's the X11 mechanism)

**Nested run**:
Caminito running as a window inside another desktop, on a private session bus. A nested run may start mate-session, but it isn't a Session: no display manager, no logind session of its own, and no systemd environment.
_Avoid_: nested session, dev session

**Golden**:
A reference rendering of one frame case (theme, frame type, state, button states, scale) produced by Marco's own C engine and committed to the repo, with its geometry. Caminito's theme engine is correct when it matches the goldens.
_Avoid_: snapshot (that's a text assertion in the integration tests), reference image

**Milestone**:
One of the nine slices of Caminito v0.1, M1 to M9. It closes when its scenario runs, CI is green with its tests, and its checklist items pass, recorded in a `Close M<n>` commit.
_Avoid_: phase, sprint, release (only v0.1.0 is a release)

**Policy**:
Caminito's window-management decisions: focus, stacking, placement, workspaces, transients and Marco's other window rules. It sees windows only through the `PolicyWindow` trait, never as protocol objects, so it treats xdg and XWayland windows alike.
_Avoid_: layout (niri's term for its tiling model), shell (cosmic-comp's term)

**Workspace grid**:
The rows and columns the workspaces are in, which the workspace switcher shows and the bindings for the workspace to the left or right move through. It comes from `org.mate.Caminito`'s three `workspace-layout-*` keys and the number of workspaces, and each workspace's cell is its ext-workspace coordinates.
_Avoid_: layout (that's the outputs'), pager layout, desktop layout (`_NET_DESKTOP_LAYOUT` is X11's, which Caminito doesn't publish)

**Window switcher**:
The popup that Alt+Tab and Marco's other `switch-*` actions open while the modifier is held: a grid of the windows' thumbnails, most recently used first, from which releasing the modifier activates the selected one. It stands in for Marco's Alt+Tab popup (ADR 0016).
_Avoid_: task switcher (KDE's term; Caminito's unit is the window), Alt+Tab popup or tab popup (Marco's own, which Caminito doesn't have), switcher alone (the workspace switcher is the panel's pager)

**Global menu**:
The focused window's menubar, shown in the panel instead of inside the window. There is one, whatever the number of panels or outputs, and a window without a menubar has none.
_Avoid_: app menu (GNOME's single per-application menu, a different thing), main menu (the panel's Alt+F1 menu), menubar alone (that's the one inside a window)

**Global menu applet**:
The mate-panel applet that draws the global menu, with the focused window's application name before it when the user has that turned on.
_Avoid_: widget, appmenu (vala-panel's and KDE's name), menu applet (the main menu is one too)

**Menu claim**:
A program with no window of its own saying that the menu it serves is the menubar of its process's focused window (ADR 0018). It is how VS Code's menubar becomes the global menu.
_Avoid_: registration (that's an X11 window's menu at the registrar), menu binding

**Global menu extension**:
Caminito's extension for VS Code and the forks that share its extension API, which rebuilds the editor's menubar, serves it and makes the menu claim for its window. The menu is all it does.
_Avoid_: Caminito integration (the working name, for an extension that would do more), plugin, VS Code extension alone (any extension is one)

**Scene**:
The order in which Caminito draws an output: the pointer's cursor, a Wayland drag's icon, the lock, then the overlay layer, override-redirect windows, a focused fullscreen window with its transients, the top layer, the popups of the bottom and background layers' surfaces, the workspace's other windows in policy's stacking order, then the bottom and background layers. It's rebuilt every frame, not kept as a tree.
_Avoid_: scene graph (Caminito has no retained one), space (Smithay's `desktop::Space`, which Caminito doesn't use)

**Night Light**:
Caminito's warming of the outputs' colours toward a lower colour temperature, on a schedule. It is one feature, and the gradual change through the day that follows the sun is one of its schedules, not a feature of its own.
_Avoid_: Night Shift and True Tone (macOS's, and True Tone reads a light sensor, which Caminito doesn't), natural light, blue-light filter, redshift

**Schedule**:
What decides Night Light's colour temperature at each moment: the sun schedule or the constant schedule.
_Avoid_: mode

**Sun schedule**:
The schedule that follows the sun's elevation at the user's location: warmest at night, coolest at solar noon, changing gradually in between. It has a noon temperature and a night temperature. The UI calls it "Follow the sun".
_Avoid_: natural light, True Tone, sunset to sunrise (that's Plasma's schedule of two flat temperatures)

**Constant schedule**:
The schedule that holds one temperature for as long as Night Light is on.
_Avoid_: always on, manual mode

**Gamma client**:
An outside program, such as gammastep, that holds an output's gamma table through `wlr-gamma-control`. Night Light steps back from that output while it does and resumes when the table is released.
_Avoid_: gamma tool, redshift client

**Pause**:
Holding Night Light off until the next sunrise while it stays enabled. It ends by itself at sunrise and the schedule takes over again.
_Avoid_: inhibit (that's an app holding it off, which Caminito doesn't offer), suspend, disable (that's turning Night Light off)
