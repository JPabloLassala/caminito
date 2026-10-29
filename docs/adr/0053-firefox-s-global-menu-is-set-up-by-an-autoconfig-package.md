# 53. Firefox's global menu is set up by a package, through autoconfig

Date: 2026-10-29
Status: Accepted. Qualifies ADR 0017, which left Firefox's two prefs to the user.

## Context

Firefox serves its menubar to the global menu only with `widget.gtk.global-menu.enabled` and `widget.gtk.global-menu.wayland.enabled` set, and the user set both in `about:config` (ADR 0017).

With the menubar in the panel, an empty strip showed above the tabs, sometimes. The applet sends Firefox the dbusmenu `opened` event for a menu it opens, which Firefox needs to fill History and Bookmarks in. Firefox turns it into a `popupshowing` on the XUL menu (`DBusEventCallback`, `widget/gtk/NativeMenuGtk.cpp`). The event bubbles to the autohide Menu Bar toolbar, and `AutoHideMenubar` (`browser/base/content/browser-customization.js`) shows the toolbar for any `popupshowing` in it, as when Alt is pressed. The menubar in it is `display: none` while it's native (`xul.css`), so the toolbar is empty, and the window's buttons, which move up to a shown toolbar, are hidden in the tab strip. Only `DOMMenuBarInactive` hides the toolbar again, and a menubar in the panel isn't made active or inactive in XUL. It's Firefox's bug, and there was none open for it.

The only way to style Firefox's own window for every profile is autoconfig: a pref file in `defaults/pref` names a `.cfg` in Firefox's directory, which Firefox runs with chrome privileges at start. A release build runs it in a sandbox unless `general.config.sandbox_enabled` is false. `userChrome.css` is in each profile and needs a pref of its own, and a WebExtension can't style Firefox's window.

## Decision

- **A package, `caminito-firefox-global-menu`, sets Firefox up.** It ships a pref file, `/usr/lib/firefox/defaults/pref/caminito-firefox-global-menu.js`, and an autoconfig file, `/usr/lib/firefox/caminito-firefox-global-menu.cfg`. It depends on `firefox`, `libdbusmenu-glib` and `libdbusmenu-gtk3`.
- **The two prefs are default prefs,** so `about:config` can still turn them off.
- **The autoconfig file registers a user sheet** with `nsIStyleSheetService`. While the menubar is native, the autohide toolbar is kept as Firefox keeps it when inactive, at no height, and the window's buttons stay visible in the tab strip. A Menu Bar toolbar the user keeps shown has no autohide, and is left alone, with whatever buttons are on it.
- **It isn't a `-git` package.** It builds nothing, and its files are beside its PKGBUILD, as its AUR repo has them (ADR 0052). It's versioned by hand.
- **`caminito-git` lists it as an optional dependency,** as it lists `caminito-vscode-git`.

## Consequences

- Installing the package is all a user does for Firefox's global menu, for every profile.
- Firefox reads one autoconfig file. Another package or a hand-made `mozilla.cfg` that sets `general.config.filename` takes the place of this one, or this one of it, whichever pref file Firefox reads last.
- The package puts two files into `firefox`'s tree, which ADR 0017 had rejected for the patched packages. They're new files and change none of Firefox's, and the package is installed only by a user who asks for it, as ADR 0022 says of `caminito-vscode-git`.
- Other builds in other directories (`firefox-developer-edition`, `firefox-nightly`, LibreWolf) aren't reached.
- The sheet hides the toolbar's strip and doesn't stop Firefox from making it active, so the fix should go upstream, in `AutoHideMenubar`, by ignoring a `popupshowing` from a native menubar. The sheet can then go.
- Run: Firefox 159 from a copy of `/usr/lib/firefox`, headless, with `native` on `#main-menubar` and a `popupshowing` fired on the File menu. Without the sheet the toolbar was 20 px tall and the window's buttons hidden. With it the toolbar was 0 px and the buttons visible. Not run: the package installed, with the applet and a real window.
- Rejected:
  - `userChrome.css`, documented (each profile, by hand, and a pref to turn it on).
  - Dropping `opened` in the applet (History, Bookmarks and other menus Firefox fills when they open would stay empty).
  - Patching Firefox's JavaScript from the autoconfig file (it would break when Firefox changes `AutoHideMenubar`, where the sheet only reads two IDs and two attributes).
