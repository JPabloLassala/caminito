# Spec: Wallpaper

Written on 2026-10-12 from a grilling session with the developer. It holds the decisions, and links the ADR that holds the one hard to reverse. Where the two differ, the ADR is right and this file is to be fixed.

Nothing here is built, and no tickets are cut yet.

## Problem

MATE's Appearance → Background sets one picture for every output. It knows only the pictures added to it by hand, has no slideshow to choose from the dialog, no picture from an online source, and no wallpaper that follows the time of day. Plasma has all four.

## Solution

Each output has its own **wallpaper** (CONTEXT.md), of one of five **wallpaper kinds**: a picture, a colour, a **slideshow**, a **picture of the day** or a **dynamic wallpaper**. A new program, **caminito-wallpaper**, draws them in place of mate-settings-daemon's background plugin ([ADR 0031](../../docs/adr/0031-caminito-wallpaper-draws-the-wallpaper-in-place-of-msds-plugin.md)), and its dialog, "Wallpaper" in the Control Center, sets them.

## User stories

1. As a user with two monitors, I want a different wallpaper on each, and one switch to keep them the same.
2. As a user, I want to pick from every wallpaper already on my disk, MATE's and Plasma's, without adding them one by one.
3. As a user, I want to add a picture or a whole folder of mine.
4. As a user, I want what I click to be on my desktop at once.
5. As a user, I want a folder of pictures to rotate on a timer.
6. As a user, I want today's picture from NASA, Bing or Wikimedia Commons, with no account and no key.
7. As a user offline, I want yesterday's picture to stay rather than a blank desktop.
8. As a user, I want to keep a picture of the day I like.
9. As a user, I want a wallpaper that follows the day where I am, and changes together with Night Light.
10. As a user, I want changes to fade rather than jump.
11. As a user, I want a monitor I plug in again to have the wallpaper it had, and a new one to have something sensible.
12. As a user, I want the lock screen of each monitor to show that monitor's wallpaper.
13. As a user, I want "Set as wallpaper" in caja to keep working.

## Decisions

### Who draws

- caminito-wallpaper draws, as a layer-shell client with one surface per output in the background layer. mate-settings-daemon's background plugin is off in a Session. See ADR 0031.
- One crate, `caminito-wallpaper`, with two programs: the drawer, started with the Session and free of GTK, and the dialog, in GTK4 without libadwaita as caminito-displays and caminito-about are (ADR 0014). The two programs' names are not decided.
- Every change of wallpaper cross-fades over about a second.

### An output's wallpaper

- Each output has one wallpaper of one kind. There is no wallpaper per workspace, and no single picture spanned across outputs.
- **"Same on all outputs"** is a switch, on by default. While it is on, every output has the same settings. A slideshow still draws independently on each output, so two outputs show different pictures of the folder.
- An output is known as a saved layout knows it: by its monitor's make, model and serial, or by its connector when it reports none.
- An output never seen before takes the primary output's wallpaper.

### The kinds

- **Picture:** one file.
- **Colour:** one plain colour. No gradients.
- **Slideshow:** a folder. The interval is one of 5 minutes, 15 minutes, 1 hour and 1 day. The order is random, or in order with a switch.
- **Picture of the day:** one source of NASA's Astronomy Picture of the Day, Bing and Wikimedia Commons. None needs a key. The sources are behind one interface so that more can follow.
  - It is fetched at session start, on wake from suspend and shortly after local midnight.
  - With no network, or when the day's item isn't a usable picture (NASA's is sometimes a video), the last one stays.
  - Only the latest of each source is kept, in `~/.cache`.
  - "Keep this picture" in the dialog copies it to `~/Pictures/Wallpapers` and adds it to the picture grid.
  - The dialog shows the picture's title and credit.
- **Dynamic:** three formats.
  - KDE's packages with a light and a dark picture, as found in `/usr/share/wallpapers`: day and night.
  - GNOME's timed XML, as `/usr/share/backgrounds/cosmos`: pictures at clock times, with cross-fades.
  - Apple's HEIC files with pictures keyed to the sun's position. `libheif` is an optional dependency; without it these files are greyed out in the dialog with a line that says why.
  - Caminito ships no dynamic wallpaper of its own. The dialog lists the pairs and timed XMLs it finds, and "Add…" takes a `.heic` or `.xml` file.
- **Positioning** is chosen per output for every kind but Colour: Fill (the default), Fit, Stretch, Centre and Tile. The output's colour fills the bars that Fit and Centre leave.

### The sun

- A dynamic wallpaper follows the same sun as Night Light's sun schedule, so the two agree about dusk.
- Caminito stays the only geoclue client. It keeps the location current whether or not Night Light is on, and publishes the sun's elevation and azimuth on its D-Bus interface. caminito-wallpaper reads them and does no sun sums of its own.
- With no location, Night Light's fixed times stand in (night until 07:00 and from 19:00).
- A timed XML follows the clock, as its format says.

### The settings

- gsettings under `org.mate.Caminito`, one relocatable set of keys per monitor identity, written by the dialog and read live by the drawer. The folders and pictures the user added are kept there too.
- `org.mate.background` is bridged one way in each direction:
  - the drawer writes the primary output's current picture to `picture-filename`, for whatever still reads it;
  - a write to `picture-filename` by anything else, such as caja's "Set as wallpaper", makes that file the Picture of every output.
- On first run, the picture `org.mate.background` names becomes every output's wallpaper.

### The lock screen

Each output's lock screen shows that output's current wallpaper, whatever its kind. caminito-locker reads `org.mate.background` today, so it needs a way to learn each output's current picture from caminito-wallpaper. How is not decided.

### The dialog

- The layout is variant B of the prototype (below): a sidebar and a controls pane.
- **The sidebar** has, from the top:
  - a small picture of each output showing its wallpaper, stacked in the layout's left-to-right order, all the same width, each with the monitor's name under it. A click selects the output;
  - "Same on all outputs";
  - the five kinds as a list with icons.
- With one output, the sidebar shows that output's picture as a plain preview, and "Same on all outputs" is hidden.
- **The controls pane** shows the selected kind's controls for the selected output.
  - Picture: one scrolling grid of thumbnails with a heading per source, in this order: the current picture, the pictures and folders the user added, MATE's (`/usr/share/backgrounds`), Plasma's (`/usr/share/wallpapers`, one picture per package). "Add picture…" and "Add folder…" under it. `~/Pictures` isn't scanned. No search and no filter.
  - The other kinds: the controls their decisions above name.
- **Everything applies on click.** There is no Apply button, and no preview other than the sidebar's pictures and the desktop itself.
- It is in English only, as caminito-displays is.

### The Control Center

- The entry is "Wallpaper", installed to `share/caminito/xdg/applications` as the other Caminito entries are, so only a Session has it.
- A carried patch to mate-control-center hides the Background tab of Appearance under Caminito. X11 MATE keeps it.

## Not decided

- The names of the two programs.
- How caminito-locker learns each output's current picture.
- The gsettings schema's keys.
- What the dialog's entry is categorised as in the Control Center.

## Prototype

Three variants of the dialog were tried on 2026-10-12: A, one column as Plasma's, with no preview; B, a sidebar of outputs and kinds; C, the outputs drawn large at their relative sizes, the kinds as tabs and the pictures as one strip per source. The developer chose B, with a heading per source from C and nothing from A.

The prototype is `tools/prototype-wallpaper.py` on the branch `prototype/wallpaper`, and isn't on master. It is PyGObject and throwaway. Choosing a Picture in it sets `org.mate.background` for real; the other kinds only show their controls.

## Tickets

None yet. They are cut once the developer has read this spec.
