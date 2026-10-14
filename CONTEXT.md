# Caminito

Caminito is a Wayland compositor for the MATE desktop, built to be the Wayland counterpart of Marco.

## Language

**Caminito**:
The Wayland compositor for the MATE desktop. It is display server, window manager and compositor in one process.
_Avoid_: Caminito window manager, Caminito WM, Camino (the former name)

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
A distribution package that builds a MATE component, or a panel applet from outside MATE, with its carried patches, and takes the stock package's place, so that only one of the two is installed.
_Avoid_: fork package, Caminito build of MATE

**Backported package**:
A stock MATE component built at a newer upstream version, with no carried patches, because a distribution's own is older than what Caminito or a patched package needs. It takes the stock package's place as a patched package does.
_Avoid_: patched package

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

**Colour scheme**:
The light or dark preference that apps read through the portal. It is separate from the GTK theme: choosing a dark theme doesn't by itself make the colour scheme dark.
_Avoid_: dark mode, dark theme (a GTK theme that happens to be dark)

**Helper**:
A process that Caminito itself starts and supervises: Xwayland, the palette helper, the locker and, in a Session, caminito-wallpaperd. MATE components aren't helpers, because mate-session starts and restarts them.
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

**Powered off**:
An output whose monitor idle has put to standby (DPMS) after mate-power-manager's `sleep-display` time. It keeps its mode, its place and its output scale, and clients see no change; activity powers it on. It isn't a disabled output.
_Avoid_: DPMS (the mechanism, on the tty only), asleep

**Primary output**:
The one output the user chooses as primary. The panel and caja's desktop go there, new windows open there while a session has no Active output yet, and X11 apps see it as the RandR primary. When it is unplugged or disabled, the built-in panel stands in, else the leftmost output, until it returns.
_Avoid_: main output, default output

**Main output**:
The output a surface is mostly on, which decides the scale the surface is asked to render at.
_Avoid_: primary output (that's the user's choice; Smithay's name for this is "primary scanout output")

**Active output**:
The output a seat's workspace bindings act on, the one the Workspace popup opens on, and the one a new window opens on. It is the focused window's Main output. When a workspace switch leaves no window focused it stays the same output, until a window on another output is focused or the user clicks on anything on another output: a window, a panel, caja's desktop, the wallpaper or a workspace of the Overview's bar. Moving the pointer there doesn't change it. While the Overview is open, the output its selection is on is the Active output.
_Avoid_: current output, focused output (an output has no focus), primary output (that's the user's choice)

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
One of the nine slices of Caminito v0.1, M1 to M9. It closes when its scenario runs, `tools/check` passes on the developer's machine with its tests, and its checklist items pass, recorded in a `Close M<n>` commit.
_Avoid_: phase, sprint, release (only v0.1.0 is a release)

**Policy**:
Caminito's window-management decisions: focus, stacking, placement, workspaces, transients and Marco's other window rules. It sees windows only through the `PolicyWindow` trait, never as protocol objects, so it treats xdg and XWayland windows alike.
_Avoid_: layout (niri's term for its tiling model), shell (cosmic-comp's term)

**Tile**:
A half or quarter of an output's work area that a window dropped at the output's side or corner fills, and keeps filling until it's untiled. The tiling keybindings tile a window there too, and with tile cycling repeated presses change the tile's width to another share of the work area's. A tiled window is in a state of its own beside maximise (ADR 0024); a window dropped at the top is maximised, not tiled. The tile preview is the translucent rectangle that shows where a drop will put the window.
_Avoid_: snapped (144's word for a window maximised one way), half-maximised, tiling model (Caminito isn't a tiling compositor)

**Per-output workspaces**:
Each output has its own workspaces and its own active one, so switching on one output leaves the others alone. The count, the names and the Workspace grid are the same on every output. A window is on a workspace of its Main output, and a sticky window is on every workspace of that output. A transient family is the exception: all of it is on the workspace of its root's Main output, wherever a dialog of it has been dragged. It is Caminito's default, from `org.mate.Caminito`'s `workspaces-per-output`.
_Avoid_: per-monitor workspaces, independent workspaces, separate workspaces

**Spanning workspaces**:
One set of workspaces and one active workspace for every output, as in Marco. It is what Caminito does when Per-output workspaces is off.
_Avoid_: shared workspaces, global workspaces

**Workspace grid**:
The rows and columns the workspaces are in, which the workspace switcher shows and the bindings for the workspace to the left or right move through. It comes from `org.mate.Caminito`'s three `workspace-layout-*` keys and the number of workspaces, and each workspace's cell is its ext-workspace coordinates.
_Avoid_: layout (that's the outputs'), pager layout, desktop layout (`_NET_DESKTOP_LAYOUT` is X11's, which Caminito doesn't publish)

**Window switcher**:
The popup that Alt+Tab and Marco's other `switch-*` actions open while the modifier is held: a grid of the windows' thumbnails, most recently used first, from which releasing the modifier activates the selected one. It stands in for Marco's Alt+Tab popup (ADR 0016).
_Avoid_: task switcher (KDE's term; Caminito's unit is the window), Alt+Tab popup or tab popup (Marco's own, which Caminito doesn't have), switcher alone (the workspace switcher is the panel's pager)

**Window menu**:
The menu of a window's actions, Marco's: minimise, maximise, move, resize, always on top, the workspace moves and close. Alt+Space, a right click on a titlebar, the Frame's menu button and a client's own request open it. Caminito draws it (ADR 0026).
_Avoid_: system menu, context menu, app menu (that's the Global menu's or the Frame's appmenu button)

**Workspace popup**:
The popup that a switch or a move to the workspace beside shows while the binding's modifiers are held: the Workspace grid, each workspace's cell with its name and an outline of its windows, and the active workspace marked. It's Marco's workspace popup, drawn by Caminito.
_Avoid_: workspace switcher (the panel's pager), workspace OSD, tab popup (Marco's widget for it and for Alt+Tab)

**Overview**:
A view of each output's active workspace with its windows shrunk and set side by side, none over another, and a bar of the workspaces beside them. A window is clicked to activate it, or dragged onto a workspace of the bar to move it there. It's Plasma's Overview, without its search field.
_Avoid_: exposé, present windows (KWin's older effect, with no workspaces), activities overview (GNOME's, which has a launcher), workspace switcher (the panel's pager)

**Workspace slide**:
The movement that shows a change of an output's active workspace: the old workspace's windows leave and the new one's arrive, along the line between the two in the Workspace grid. Whatever is the same on both stays still: sticky windows, a window that is carried or being moved to the new workspace, the panel, caja's desktop and the wallpaper. It's Plasma's Slide effect.
_Avoid_: desktop slide (the glossary's word is workspace), transition (the Overview has one too), workspace animation

**Opening animation**, **Closing animation**:
The zoom and fade that show a window coming on screen or leaving it: the window grows into its place as it appears, or shrinks and fades away from it. A window opens when it is shown for the first time or again after its client hid it, and closes when its client destroys or hides it while it's on screen. Minimising, unminimising and a move to another workspace aren't either. It's Plasma's Scale effect.
_Avoid_: window animation (the minimise animation is one too), map or unmap animation (X11's and Wayland's words, and an unmapped window isn't always closed), scale effect (KWin's name), fade alone

**Interaction**:
What a seat is in the middle of, which has its keyboard and holds its pointer's focus where it is: a move or resize, the Window switcher, the Window menu, the Workspace popup or the Overview. A seat has one at most, and none opens while it has another. The session lock isn't one: it ends the seat's interaction and lets none open.
_Avoid_: grab (that's Smithay's pointer and keyboard grabs, an accelerator grab, and Xwayland's keyboard grab), grab op (Marco's word for it), mode (that's `focus-mode` and `placement-mode`), modal (that's a dialog)

**System shortcut**:
A combination of modifiers with a key or a mouse button that Caminito acts on before any client sees it, whoever owns the action: Caminito's own window and workspace actions, a command the user gave a shortcut to, or a media key. System Shortcuts is the control centre's dialog that lists and changes them. A binding is one of Caminito's own, and an accelerator is one another program has asked Caminito for.
_Avoid_: keyboard shortcut (a mouse button can be one), keybinding, hotkey, global shortcut, shortcut alone for an application's own (those are the client's)

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
The order in which Caminito draws an output: the pointer's cursor, a Wayland drag's icon, the lock, then the overlay layer, override-redirect windows, a focused fullscreen window with its transients, the top layer, the popups of the bottom and background layers' surfaces, the workspace's other windows in policy's stacking order with the tile preview beneath the window being moved, then the bottom and background layers. It's rebuilt every frame, not kept as a tree.
_Avoid_: scene graph (Caminito has no retained one), space (Smithay's `desktop::Space`, which Caminito doesn't use)

**Wallpaper**:
What an output shows behind caja's desktop and every window. Each output has its own, of one wallpaper kind, unless the user keeps them the same on all outputs.
_Avoid_: background (that's the layer the wallpaper's surface sits in, and MATE's word for its own setting), desktop background, backdrop

**Wallpaper kind**:
Which of five things an output's wallpaper is: a picture, a colour, a slideshow, a picture of the day or a dynamic wallpaper.
_Avoid_: wallpaper type and wallpaper plugin (Plasma's words), mode

**Slideshow**:
A wallpaper that shows the pictures of a folder one after another, changing on a timer.
_Avoid_: rotation, dynamic wallpaper

**Picture of the day**:
A wallpaper that is the picture an online source publishes for the day, and changes when the source's does.
_Avoid_: daily wallpaper, online wallpaper

**Dynamic wallpaper**:
A wallpaper that is one set of pictures of the same scene, and shows the one that fits the time of day.
_Avoid_: slideshow (its pictures are unrelated and follow a timer), live wallpaper, animated wallpaper

**caminito-wallpaper**:
Caminito's wallpaper program, which draws each output's wallpaper in place of mate-settings-daemon's background plugin, and whose dialog replaces the Background tab of MATE's Appearance capplet in a Session.
_Avoid_: the background plugin, wallpaper daemon, swaybg

**caminito-bluetooth**:
Caminito's Bluetooth program, which runs through the whole session as the pairing agent and opens the Bluetooth settings window. Its popup in the panel is the Bluetooth applet.
_Avoid_: the capplet, blueman, bluedevil

**Bluetooth applet**:
Caminito's mate-panel applet for Bluetooth: the icon in the panel and its popup of devices, in the manner of Plasma's.
_Avoid_: widget, plasmoid, tray icon

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
