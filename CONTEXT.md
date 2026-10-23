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
A shell surface that sits in a fixed layer of an output rather than among ordinary windows: the panel, caja's desktop, the wallpaper, notification bubbles and mate-screenshot's area picker.
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
The output a seat's workspace bindings act on, the one the Workspace popup opens on, and the one a new window opens on. It is the focused window's Main output. When a workspace switch leaves no window focused it stays the same output, until a window on another output is focused, the user clicks on anything on another output (a window, a panel, caja's desktop, the wallpaper or a workspace of the Overview's bar), or a binding that switches the workspace runs with the pointer on another output, which it then switches. Moving the pointer there alone doesn't change it. While the Overview is open, the output its selection is on is the Active output.
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

**Application**:
The windows that belong together as one program's: those of one process, or, for a window whose process isn't known, those with its app id. Two processes of one program are two applications. It's what Marco's `switch-group` cycles through.
_Avoid_: app id alone (two processes can share one), program, task, group (that's Marco's action, not the thing)

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

**Application overview**:
The Overview opened for the focused window's Application: each output's active workspace shows only that application's windows. It's the Overview in every other way, and opens only when a window is focused. It's Plasma's Window View for the current application.
_Avoid_: app exposé (macOS's), present windows for the current application (KWin's older name), application switcher (that's `switch-group`'s Window switcher)

**Hot corner**:
A corner of an output where the Output layout stops the pointer both ways, which runs its action, the Overview, the Application overview, Show desktop or the lock, when the pointer rests in it for the Activation delay. The four corners' actions are the same on every output, and a corner is off while a pointer button is held, an Interaction other than the Overview or the Application overview is open, a fullscreen window is focused on its output, or a client has locked or confined the pointer. It's Plasma's screen edges, without the edges.
_Avoid_: screen edge (Plasma's term, which covers the edges too), active corner, hot spot

**Activation delay**:
How long the pointer rests in a Hot corner before the corner fires, the same for all four corners and set by the user. A change counts for a rest already under way, from when it began. Once a corner has fired, it fires again only after the pointer has left it and a fixed time has passed, however short the Activation delay. It's Plasma's activation delay.
_Avoid_: threshold (it could be a distance or a pressure), rest (that's what the pointer does, not how long), reactivation delay (that's the fixed time before a corner fires again)

**caminito-behaviour**:
Caminito's settings program for how Caminito itself behaves, beyond what MATE's capplets set: the Hot corners now, and later the animations and Per-output workspaces. The user reads it as Workspace Behaviour, Plasma's name for the section that holds these.
_Avoid_: Caminito settings, desktop effects (hot corners aren't an effect), the Windows dialog (that's MATE's, for Marco's keys)

**Workspace slide**:
The movement that shows a change of an output's active workspace: the old workspace's windows leave and the new one's arrive, along the line between the two in the Workspace grid. Whatever is the same on both stays still: sticky windows, a window that is carried or being moved to the new workspace, the panel, caja's desktop and the wallpaper. It's Plasma's Slide effect.
_Avoid_: desktop slide (the glossary's word is workspace), transition (the Overview has one too), workspace animation

**Opening animation**, **Closing animation**:
The zoom and fade that show a window coming on screen or leaving it: the window grows into its place as it appears, or shrinks and fades away from it. A window opens when it is shown for the first time or again after its client hid it, and closes when its client destroys or hides it while it's on screen. Minimising, unminimising and a move to another workspace aren't either. It's Plasma's Scale effect.
_Avoid_: window animation (the minimise animation is one too), map or unmap animation (X11's and Wayland's words, and an unmapped window isn't always closed), scale effect (KWin's name), fade alone

**Popup slide**:
The movement that shows a menu, a tooltip or another popup coming on screen or leaving it: it slides out of the edge it is attached to as it opens, and back into it as it closes. Popups are xdg_popups, override-redirect windows and the Window menu. It's Plasma's Sliding popups effect, with the edge chosen by Caminito (ADR 0037).
_Avoid_: menu animation (tooltips and combo box lists slide too), fading popups (Plasma's other effect), sliding popups (KWin's name)

**Maximise animation**:
The movement that shows a window being maximised, unmaximised, tiled, untiled or given another tile: its frame goes from its old place to its new one while a picture of it as it was fades out. Maximising one way, fullscreen and a drag that pulls a window off its maximised or tiled place aren't animated. It's Plasma's Maximize effect, with tiles too.
_Avoid_: resize animation (a resize by the pointer isn't one), tile animation (it covers maximising too), morph

**Wheel glide**:
A mouse wheel's click spread out over a short moment, so the view a client scrolls glides one step instead of jumping it. Caminito makes it for every client but the ones the user lists because they glide the wheel themselves, such as browsers, and the step is as long as without it. A click during a glide lengthens it by a step, and a click the other way starts a new one. What's left of a glide is lost once the pointer is over something else. A click with Ctrl, Alt or Super held isn't glided. A touchpad's scrolling isn't glided, and nothing coasts on once the wheel stops.
_Avoid_: smooth scrolling (it also covers a touchpad's precise scrolling and kinetic scrolling), kinetic scrolling (that's the coast after a touchpad flick), scroll animation

**Interaction**:
What a seat is in the middle of, which has its keyboard and holds its pointer's focus where it is: a move or resize, the Window switcher, the Window menu, the Workspace popup, the Overview or the Capture picker. A seat has one at most, and none opens while it has another. The session lock isn't one: it ends the seat's interaction and lets none open.
_Avoid_: grab (that's Smithay's pointer and keyboard grabs, an accelerator grab, and Xwayland's keyboard grab), grab op (Marco's word for it), mode (that's `focus-mode` and `placement-mode`), modal (that's a dialog)

**System shortcut**:
A combination of modifiers with a key or a mouse button that Caminito acts on before any client sees it, whoever owns the action: Caminito's own window and workspace actions, a command the user gave a shortcut to, or a media key. System Shortcuts is the control centre's dialog that lists and changes them. A binding is one of Caminito's own, and an accelerator is one another program has asked Caminito for.
_Avoid_: keyboard shortcut (a mouse button can be one), keybinding, hotkey, global shortcut, shortcut alone for an application's own (those are the client's)

**Key remap**:
A combination the user presses that Caminito swaps for another, for a sequence of others, or for nothing, before the focused window's client receives it, such as Alt+T arriving as Ctrl+T, Alt+Right as End, or Alt+BackSpace as Shift+Home then BackSpace. Remaps are listed for all applications and for each Remap group, and a group's can replace or turn off those for all. A System shortcut is matched first, so a combination that is one is never remapped, and a target that is one runs it instead of reaching the client, as Alt+Q sent as Alt+F4 closes the window. A target is never remapped again.
_Avoid_: override, translation, rebinding, modifier swap (Caminito swaps combinations, not modifiers), system shortcut (Caminito acts on that one itself)

**Remap group**:
A named list of app ids that share their Key remaps, such as Terminals or Browsers. An app id is in one group at most.
_Avoid_: app group, class list (Toshy's), group alone (that's Marco's `switch-group`)

**Remap template**:
A named, saved set of Key remaps, those for all applications and each Remap group's. Caminito comes with some, such as the macOS one, and the user saves their own; loading one replaces the Key remaps in use.
_Avoid_: preset, profile (suggests switching sets as you work), keymap (that's XKB's)

**Capture**:
A Screenshot or a Screen recording that the user takes with a System shortcut.
_Avoid_: grab, snap

**Capture picker**:
The Interaction in which the user shows Caminito what to capture: an area dragged out, a window clicked or an output clicked, with the rest of the output dimmed. Escape cancels it.
_Avoid_: area picker alone (it picks windows and outputs too), selection, crosshair

**Screenshot**:
A still picture of an output, of a window with its frame, or of an area the user drags out, saved to the Desktop or Pictures folder or put on the clipboard. Each kind has a System shortcut that saves it and one that copies it.
_Avoid_: screen capture, screen grab, snapshot

**Screen recording**:
A silent video of an output or of an area the user drags out, saved to the Desktop or Videos folder. The same System shortcut starts and stops it.
_Avoid_: screencast (that's the stream), video capture

**Screencast**:
A live stream of an output, a window or an area that Caminito hands to a program through the portal, such as a browser sharing the screen. A Screen recording is made from one.
_Avoid_: screen sharing (that's what a program does with it), screen recording (that's the saved video)

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

**caminito-network**:
Caminito's network program, which runs through the whole session as NetworkManager's secret agent, asking for the passwords and codes a connection needs. With the Network applet it takes the place of nm-applet.
_Avoid_: nm-applet (the program it replaces), network manager (that's NetworkManager, the system service), the capplet

**Network applet**:
Caminito's mate-panel applet for networking: the icon in the panel and its popup of connections, in the manner of Plasma's Networks widget. Its settings window is nm-connection-editor.
_Avoid_: widget, plasmoid, tray icon, nm-applet (the program whose tray icon it replaces)

**Power applet**:
Caminito's mate-panel applet for power: the battery icon in the panel and its popup of the power profile, the batteries, and what is keeping the computer awake, in the manner of Plasma's Power and Battery.
_Avoid_: battery monitor, mate-power-manager's tray icon, power capplet (mate-power-preferences is the settings window)

**Stay awake**:
The user's own switch in the Power applet that keeps the computer from sleeping and the screen from locking until it's turned off. An application doing the same is just an application keeping the computer awake.
_Avoid_: inhibit, caffeine, presentation mode, block sleep

**Thermal applet**:
Caminito's mate-panel applet for temperatures and fans: a thermometer in the panel with the panel sensors' temperatures over the fans' speed, and its popup of the computer's temperatures and fan speeds, in the manner of TG Pro. It only watches; it never sets a fan's speed.
_Avoid_: sensors applet (MATE's own mate-sensors-applet), temperature applet, thermal capplet, TG Pro

**Panel sensor**:
One of the temperatures, two at most, that the Thermal applet shows in the panel, chosen by pinning it in the popup. By default they're the CPU's and, when there's a GPU, the GPU's.
_Avoid_: main sensor, primary temperature, pinned sensor

**Sound applet**:
Caminito's mate-panel applet for sound: the volume icon in the panel and its popup of the sound devices and the applications playing or recording. In a Session it takes the place of mate-media's volume icon, and its settings window is mate-volume-control.
_Avoid_: widget, plasmoid, tray icon, volume applet, GvcApplet (mate-media's own), mixer

**Sound device**:
An output or an input of the sound server, such as the speakers, an HDMI output, a headset or a microphone: the sound server's sink or source, not the card behind it.
_Avoid_: sink, source (the sound server's names), card (it can have several), endpoint

**Default output**, **Default input**:
The sound device that new streams play to or record from unless they're moved, chosen by the user. The panel icon shows the default output's volume.
_Avoid_: primary device, fallback device, preferred device

**Sound application**:
A program playing or recording sound, as the Sound applet shows it: all its streams under one row, whose volume sets them all.
_Avoid_: client (the sound server's connection), sink input, source output, app stream

**Stream**:
One of a sound application's channels of playback or recording, such as one browser tab's video or one side of a call.
_Avoid_: sink input, source output, channel (that's left and right)

**Follow the default**:
A Sound application's choice to play to whatever the Default output is, moving when the user changes it. Choosing a sound device for it moves it there while it runs; only pinning keeps the choice for the next time.
_Avoid_: auto, system default, unpinned

**Pinned application**:
A program the user has pinned to a sound device, so that it plays to it or records from it instead of following the default. It stays pinned when it stops playing or isn't running, until the user unpins it.
_Avoid_: custom setting, override, routed app

**System sounds**:
The Sound applet's row for event sounds, such as the bell, the workspace sounds and notification sounds, whichever program plays them, with one volume for them all.
_Avoid_: event sounds, notification sounds (only some of them), alerts

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
