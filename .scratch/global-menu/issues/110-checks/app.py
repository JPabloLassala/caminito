"""A GtkApplication with a menubar, of GTK 3 or 4, that logs whether the shell shows it."""
import sys
import gi

version = sys.argv[1]
log = open(sys.argv[2], "w", buffering=1)
gi.require_version("Gtk", version)
from gi.repository import Gio, GLib, Gtk  # noqa: E402

app = Gtk.Application(application_id=f"org.caminito.E2e.Gtk{version[0]}")


def activate(app):
    menu = Gio.Menu()
    file = Gio.Menu()
    file.append("Ping", "app.ping")
    menu.append_submenu("File", file)
    app.set_menubar(menu)
    ping = Gio.SimpleAction.new("ping", None)
    ping.connect("activate", lambda *_: log.write("ping\n"))
    app.add_action(ping)
    window = Gtk.ApplicationWindow(application=app, title=f"GTK {version}")
    window.set_default_size(400, 200)
    if version == "4.0":
        window.set_show_menubar(True)
        window.present()
    else:
        window.show_all()
    settings = Gtk.Settings.get_default()
    seen = []

    def look():
        shown = settings.props.gtk_shell_shows_menubar
        if seen[-1:] != [shown]:
            seen.append(shown)
            log.write(f"shell-shows-menubar={shown}\n")
        return True

    GLib.timeout_add(100, look)


app.connect("activate", activate)
app.run([])
