// Firefox's menubar in the panel's global menu (ADR 0053). Firefox loads every file in its
// defaults/pref as default preferences, so about:config can still turn these off.
pref("widget.gtk.global-menu.enabled", true);
pref("widget.gtk.global-menu.wayland.enabled", true);

// The autoconfig file that keeps the empty Menu Bar toolbar hidden. A release build runs it in
// a sandbox with no XPCOM unless it's told not to; it isn't obscured.
pref("general.config.filename", "caminito-firefox-global-menu.cfg");
pref("general.config.obscure_value", 0);
pref("general.config.sandbox_enabled", false);
