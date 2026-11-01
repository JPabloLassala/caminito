#!/usr/bin/env python3
# PROTOTYPE, throw me away (CMN-296). Four layouts of the Lock screen, in an ordinary window,
# with the real GTK theme and the real lock-dialog-default.ui. No session lock, no PAM and fake
# notifications: Unlock checks nothing, and nothing here locks anything.
#
#   python3 helpers/caminito-locker/prototype_lock_screen.py
#
# In the window: left and right switch layout, any other key or a click shows the Unlock dialog,
# Esc hides it. The strip at the bottom isn't part of the design.
import argparse
import math
import os
import random
import socket
import time

import cairo
import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk, GdkPixbuf, GLib, Gtk  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
UI = os.path.join(HERE, "lock-dialog-default.ui")

VARIANTS = {"D": "Centre and notification corner", "A": "Centred stack", "B": "Corners", "C": "Bottom band"}
CORNERS = ["top-right", "top-left", "bottom-right", "bottom-left"]
MODES = ["full", "title", "none"]
WALLPAPERS = ["real", "light", "busy"]
MAX_ENTRIES = 4
# Below this, an output has no room for the dialog and a corner's notifications side by side.
NARROW_OUTPUT = 1040
BLUR_MS = 250

# app, summary, body, previews on
NOTIFICATIONS = [
    ("Signal", "Ana Ruiz", "Are we still on for lunch? I can book the place near the office.", True),
    ("Thunderbird", "Invoice 2291 is overdue", "Your account is 12 days overdue. Please pay 480.00 by Friday.", True),
    ("Bank", "Card ending 4417", "A payment of 92.10 was made at the pharmacy.", False),
    ("Calendar", "Standup in 10 minutes", "Room 2, and on the call.", True),
    ("Slack", "#deploys: a very long summary that an app sent without a thought for how narrow a list could be",
     "The body is long as well, so that the limit on an entry's length can be seen: " + "the build failed again, " * 6, True),
    ("Signal", "Marta", "Call me when you get this.", True),
]

CSS = b"""
.proto-clock { color: white; text-shadow: 0 1px 4px rgba(0,0,0,0.8); }
.proto-card { background-color: rgba(20,20,24,0.72); border-radius: 8px; padding: 10px 14px; color: white; }
.proto-plain { color: white; text-shadow: 0 1px 3px rgba(0,0,0,0.95); }
.proto-band { background-color: rgba(20,20,24,0.78); padding: 14px 24px; color: white; }
.proto-more { color: white; text-shadow: 0 1px 3px rgba(0,0,0,0.95); }
.proto-dialog { background-color: @theme_bg_color; }
.proto-strip { background-color: #c2185b; color: white; padding: 4px 10px; }
.proto-strip button { padding: 2px 8px; }
"""


def visible_notifications(mode):
    """The rule the daemon will have: (app, summary or None, body or None)."""
    if mode == "none":
        return []
    entries = []
    for app, summary, body, previews in NOTIFICATIONS:
        if not previews:
            entries.append((app, "Notification", None))
        elif mode == "title":
            entries.append((app, summary, None))
        else:
            entries.append((app, summary, body))
    return entries


def blurred(surface, w, h, passes=5):
    """A stand-in for Esmeril's look: halve and double, as dual Kawase does, without its shaders."""
    sizes = [(max(1, w >> i), max(1, h >> i)) for i in range(passes + 1)]
    current = surface
    for i in list(range(1, passes + 1)) + list(range(passes - 1, -1, -1)):
        tw, th = sizes[i]
        target = cairo.ImageSurface(cairo.FORMAT_RGB24, tw, th)
        cr = cairo.Context(target)
        cr.scale(tw / current.get_width(), th / current.get_height())
        cr.set_source_surface(current, 0, 0)
        cr.get_source().set_filter(cairo.FILTER_BILINEAR)
        cr.get_source().set_extend(cairo.EXTEND_PAD)
        cr.paint()
        current = target
    return current


class Wallpaper(Gtk.DrawingArea):
    cache = {}
    surfaces = {}
    # 0 is sharp and 1 wholly blurred, shared by every output as the dialog comes and goes.
    blur = 0.0
    strength = 5

    def __init__(self, kind):
        super().__init__()
        self.kind = kind
        self.connect("draw", self.on_draw)

    def on_draw(self, _widget, cr):
        w, h = self.get_allocated_width(), self.get_allocated_height()
        key = (self.kind, w, h, Wallpaper.strength)
        if key not in Wallpaper.surfaces:
            sharp = cairo.ImageSurface(cairo.FORMAT_RGB24, w, h)
            self.draw_kind(cairo.Context(sharp), w, h)
            Wallpaper.surfaces[key] = (sharp, blurred(sharp, w, h, Wallpaper.strength))
        sharp, soft = Wallpaper.surfaces[key]
        cr.set_source_surface(sharp, 0, 0)
        cr.paint()
        if Wallpaper.blur > 0:
            cr.set_source_surface(soft, 0, 0)
            cr.paint_with_alpha(Wallpaper.blur)

    def draw_kind(self, cr, w, h):
        if self.kind == "real":
            self.draw_real(cr, w, h)
        elif self.kind == "light":
            g = cairo.LinearGradient(0, 0, w, h)
            g.add_color_stop_rgb(0, 0.97, 0.96, 0.90)
            g.add_color_stop_rgb(1, 0.80, 0.88, 0.95)
            cr.set_source(g)
            cr.paint()
        else:
            rng = random.Random(7)
            cr.set_source_rgb(0.5, 0.5, 0.5)
            cr.paint()
            for _ in range(500):
                cr.set_source_rgb(rng.random(), rng.random(), rng.random())
                cr.arc(rng.random() * w, rng.random() * h, rng.random() * 50 + 4, 0, 2 * math.pi)
                cr.fill()

    def draw_real(self, cr, w, h):
        path = picture_filename()
        pixbuf = None
        if path and os.path.exists(path):
            key = path
            if key not in Wallpaper.cache:
                Wallpaper.cache[key] = GdkPixbuf.Pixbuf.new_from_file(path)
            pixbuf = Wallpaper.cache[key]
        if pixbuf is None:
            cr.set_source_rgb(0.15, 0.2, 0.3)
            cr.paint()
            return
        scale = max(w / pixbuf.get_width(), h / pixbuf.get_height())
        cr.scale(scale, scale)
        Gdk.cairo_set_source_pixbuf(
            cr, pixbuf, (w / scale - pixbuf.get_width()) / 2, (h / scale - pixbuf.get_height()) / 2
        )
        cr.paint()


def picture_filename():
    from gi.repository import Gio

    source = Gio.SettingsSchemaSource.get_default()
    if source is None or source.lookup("org.mate.background", True) is None:
        return None
    return Gio.Settings.new("org.mate.background").get_string("picture-filename")


def notification_corner():
    """The key the bubbles follow."""
    from gi.repository import Gio

    source = Gio.SettingsSchemaSource.get_default()
    if source is None or source.lookup("org.mate.Caminito.notifications", True) is None:
        return "top-right"
    return Gio.Settings.new("org.mate.Caminito.notifications").get_string("corner")


def clock_box(size, halign):
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
    box.set_halign(halign)
    now = time.localtime()
    for text, span in ((time.strftime("%H:%M", now), size), (time.strftime("%A, %-d %B", now), "x-large")):
        label = Gtk.Label()
        label.set_markup(f'<span size="{span}" weight="light">{GLib.markup_escape_text(text)}</span>')
        label.set_halign(halign)
        label.get_style_context().add_class("proto-clock")
        box.add(label)
    return box


def entry_widget(entry, style, width_chars=44):
    app, summary, body = entry
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
    box.get_style_context().add_class(style)
    head = Gtk.Label()
    head.set_markup(f'<span size="small" alpha="75%">{GLib.markup_escape_text(app)}</span>')
    head.set_xalign(0)
    box.add(head)
    title = Gtk.Label()
    title.set_markup(f"<b>{GLib.markup_escape_text(summary)}</b>")
    title.set_xalign(0)
    title.set_ellipsize(3)
    title.set_max_width_chars(width_chars)
    box.add(title)
    if body:
        text = Gtk.Label(label=body)
        text.set_xalign(0)
        text.set_line_wrap(True)
        text.set_lines(2)
        text.set_ellipsize(3)
        text.set_max_width_chars(width_chars)
        box.add(text)
    return box


def more_label(count):
    label = Gtk.Label(label=f"and {count} more")
    label.get_style_context().add_class("proto-more")
    return label


class Prototype(Gtk.Window):
    def __init__(self, args):
        super().__init__(title="PROTOTYPE: Lock screen (CMN-296)")
        self.variant = args.variant
        self.view = args.view
        self.mode = args.mode
        self.outputs = args.outputs
        self.wallpaper = args.wallpaper
        self.corner = notification_corner()
        self.blur_source = 0
        Wallpaper.blur = 1.0 if self.view == "dialog" else 0.0
        self.set_default_size(1600 if self.outputs == 2 else 1280, 760)

        provider = Gtk.CssProvider()
        provider.load_from_data(CSS)
        Gtk.StyleContext.add_provider_for_screen(
            Gdk.Screen.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
        )

        self.dialog = self.load_dialog()
        root = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.add(root)
        self.monitors = Gtk.Box(spacing=6)
        root.pack_start(self.monitors, True, True, 0)
        root.pack_start(self.build_strip(), False, False, 0)

        self.connect("destroy", Gtk.main_quit)
        self.connect("key-press-event", self.on_key)
        self.connect("configure-event", self.on_configure)
        self.rebuild()

    def load_dialog(self):
        builder = Gtk.Builder()
        builder.add_from_file(UI)
        dialog = builder.get_object("lock-dialog")
        dialog.set_halign(Gtk.Align.CENTER)
        dialog.set_valign(Gtk.Align.CENTER)
        dialog.get_style_context().add_class("proto-dialog")
        get = builder.get_object

        # Without its clock, and without the note: the spec's dialog.
        get("vbox4").hide()
        get("auth-note-button").hide()
        name = GLib.get_real_name() or GLib.get_user_name()
        get("auth-realname-label").set_label(get("auth-realname-label").get_label().replace("%R", GLib.markup_escape_text(name)))
        get("auth-username-label").set_label(
            get("auth-username-label").get_label().replace("%U", GLib.get_user_name()).replace("%h", socket.gethostname())
        )
        face = os.path.expanduser("~/.face")
        if os.path.exists(face):
            get("auth-face-image").set_from_pixbuf(GdkPixbuf.Pixbuf.new_from_file_at_size(face, 96, 96))
        get("auth-prompt-label").set_text("Password:")
        self.entry = get("auth-prompt-entry")
        self.entry.set_visibility(False)
        get("auth-status-label").set_text("")
        get("status-message-label").set_text("")
        get("auth-capslock-label").set_text("")
        # The layout indicator in its slot, as two layouts would show it.
        indicator = Gtk.Label(label="us")
        get("auth-prompt-kbd-layout-indicator").add(indicator)
        indicator.show()
        get("auth-switch-button").show()
        get("auth-logout-button").hide()
        get("auth-cancel-button").connect("clicked", lambda _b: self.set_view("idle"))
        get("auth-unlock-button").connect("clicked", lambda _b: self.set_view("idle"))
        self.status = get("auth-status-label")
        return dialog

    def build_strip(self):
        strip = Gtk.Box(spacing=10)
        strip.get_style_context().add_class("proto-strip")

        def button(label, callback):
            b = Gtk.Button(label=label)
            b.set_can_focus(False)
            b.connect("clicked", lambda _b: callback())
            strip.pack_start(b, False, False, 0)

        button("◀", lambda: self.cycle(-1))
        self.state_label = Gtk.Label()
        strip.pack_start(self.state_label, False, False, 0)
        button("▶", lambda: self.cycle(1))
        button("Idle / dialog", lambda: self.set_view("dialog" if self.view == "idle" else "idle"))
        button("Mode", self.next_mode)
        button("Outputs", self.next_outputs)
        button("Wallpaper", self.next_wallpaper)
        button("Corner", self.next_corner)
        return strip

    def cycle(self, step):
        keys = list(VARIANTS)
        self.variant = keys[(keys.index(self.variant) + step) % len(keys)]
        self.rebuild()

    def animate_blur(self):
        target = 1.0 if self.view == "dialog" else 0.0
        start = Wallpaper.blur
        began = time.monotonic()

        def frame():
            t = min(1.0, (time.monotonic() - began) * 1000 / BLUR_MS)
            eased = 1 - (1 - t) ** 3
            Wallpaper.blur = start + (target - start) * eased
            self.monitors.queue_draw()
            return t < 1.0

        if self.blur_source:
            GLib.source_remove(self.blur_source)
        self.blur_source = GLib.timeout_add(16, frame)

    def set_view(self, view):
        self.view = view
        self.animate_blur()
        self.entry.set_text("")
        self.rebuild()

    def next_mode(self):
        self.mode = MODES[(MODES.index(self.mode) + 1) % len(MODES)]
        self.rebuild()

    def next_outputs(self):
        self.outputs = 3 - self.outputs
        self.rebuild()

    def next_wallpaper(self):
        self.wallpaper = WALLPAPERS[(WALLPAPERS.index(self.wallpaper) + 1) % len(WALLPAPERS)]
        self.rebuild()

    def next_corner(self):
        self.corner = CORNERS[(CORNERS.index(self.corner) + 1) % len(CORNERS)]
        self.rebuild()

    def output_is_narrow(self):
        return self.get_size()[0] / self.outputs < NARROW_OUTPUT

    def on_configure(self, _window, _event):
        if self.output_is_narrow() != self.was_narrow:
            GLib.idle_add(self.rebuild)
        return False

    def on_key(self, _window, event):
        if event.keyval in (Gdk.KEY_Left, Gdk.KEY_Right) and self.view == "idle":
            self.cycle(-1 if event.keyval == Gdk.KEY_Left else 1)
            return True
        if event.keyval == Gdk.KEY_Escape:
            self.set_view("dialog" if self.view == "idle" else "idle")
            return True
        if self.view == "idle":
            self.set_view("dialog")
            self.entry.grab_focus_without_selecting()
            if event.string and event.string.isprintable():
                self.entry.set_text(event.string)
                self.entry.set_position(-1)
            return True
        return False

    def rebuild(self):
        self.was_narrow = self.output_is_narrow()
        parent = self.dialog.get_parent()
        if parent is not None:
            parent.remove(self.dialog)
        for child in self.monitors.get_children():
            self.monitors.remove(child)
        for index in range(self.outputs):
            self.monitors.pack_start(self.build_output(index == 0), True, True, 0)
        self.monitors.show_all()
        self.hide_unused_dialog_parts()
        if self.view == "dialog":
            self.entry.grab_focus_without_selecting()
        state = (
            f"{self.variant} ({VARIANTS[self.variant]}) | {self.view} | mode {self.mode} | "
            f"{self.outputs} output(s) | wallpaper {self.wallpaper} | corner {self.corner} | {'narrow: A' if self.variant == 'D' and self.was_narrow else 'wide'}"
        )
        self.state_label.set_text(state)
        print(state, flush=True)

    def hide_unused_dialog_parts(self):
        # show_all would bring them back.
        def walk(widget):
            if Gtk.Buildable.get_name(widget) in ("vbox4", "auth-note-button", "auth-logout-button"):
                widget.hide()
            if isinstance(widget, Gtk.Container):
                widget.foreach(walk)

        walk(self.dialog)

    def build_output(self, under_pointer):
        overlay = Gtk.Overlay()
        events = Gtk.EventBox()
        events.add(Wallpaper(self.wallpaper))
        events.connect("button-press-event", lambda *_a: self.view == "idle" and self.set_view("dialog"))
        overlay.add(events)
        builder = {"D": self.layout_d, "A": self.layout_a, "B": self.layout_b, "C": self.layout_c}[self.variant]
        builder(overlay, under_pointer)
        return overlay

    def entries(self, limit=MAX_ENTRIES):
        entries = visible_notifications(self.mode)
        return entries[:limit], max(0, len(entries) - limit)

    # D: the clock at the top, the dialog in the centre, and the notifications in their corner,
    # which stay where they are as the dialog comes and goes.
    def layout_d(self, overlay, under_pointer):
        if self.was_narrow:
            self.layout_a(overlay, under_pointer)
            return
        clock = clock_box("60000", Gtk.Align.CENTER)
        clock.set_halign(Gtk.Align.CENTER)
        clock.set_valign(Gtk.Align.START)
        clock.set_margin_top(24)
        overlay.add_overlay(clock)
        overlay.set_overlay_pass_through(clock, True)
        if not under_pointer:
            return
        top = self.corner.startswith("top")
        right = self.corner.endswith("right")
        column = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        column.set_halign(Gtk.Align.END if right else Gtk.Align.START)
        column.set_valign(Gtk.Align.START if top else Gtk.Align.END)
        column.set_margin_top(24)
        column.set_margin_bottom(24)
        column.set_margin_start(24)
        column.set_margin_end(24)
        shown, more = self.entries()
        widgets = [entry_widget(entry, "proto-card", 36) for entry in shown]
        if more:
            widgets.append(more_label(more))
        # The newest is nearest the corner, as the bubbles have it.
        for widget in widgets if top else reversed(widgets):
            column.add(widget)
        overlay.add_overlay(column)
        overlay.set_overlay_pass_through(column, True)
        if self.view == "dialog":
            self.dialog.set_margin_top(0)
            overlay.add_overlay(self.dialog)

    # A: everything in one column down the middle, the dialog pushing into it.
    def layout_a(self, overlay, under_pointer):
        column = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        column.set_halign(Gtk.Align.CENTER)
        column.set_valign(Gtk.Align.START)
        column.set_margin_top(48)
        clock = clock_box("xx-large" if self.view == "dialog" else "72000", Gtk.Align.CENTER)
        column.add(clock)
        column.set_spacing(10)
        if under_pointer:
            if self.view == "dialog":
                self.dialog.set_margin_top(10)
                column.add(self.dialog)
            else:
                shown, more = self.entries()
                for i, entry in enumerate(shown):
                    widget = entry_widget(entry, "proto-card")
                    if i == 0:
                        widget.set_margin_top(24)
                    column.add(widget)
                if more:
                    column.add(more_label(more))
        overlay.add_overlay(column)
        overlay.set_overlay_pass_through(column, True)

    # B: clock in the bottom left, plain text down the right edge, the dialog in the middle.
    def layout_b(self, overlay, under_pointer):
        clock = clock_box("80000", Gtk.Align.START)
        clock.set_valign(Gtk.Align.END)
        clock.set_margin_start(48)
        clock.set_margin_bottom(40)
        overlay.add_overlay(clock)
        overlay.set_overlay_pass_through(clock, True)
        if not under_pointer:
            return
        column = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=18)
        column.set_halign(Gtk.Align.END)
        column.set_valign(Gtk.Align.START)
        column.set_margin_top(36)
        column.set_margin_end(36)
        shown, more = self.entries()
        for entry in shown:
            column.add(entry_widget(entry, "proto-plain", 36))
        if more:
            column.add(more_label(more))
        overlay.add_overlay(column)
        overlay.set_overlay_pass_through(column, True)
        if self.view == "dialog":
            self.dialog.set_margin_top(0)
            overlay.add_overlay(self.dialog)

    # C: a band along the bottom holds the clock and a row of notifications.
    def layout_c(self, overlay, under_pointer):
        band = Gtk.Box(spacing=28)
        band.get_style_context().add_class("proto-band")
        band.set_valign(Gtk.Align.END)
        band.add(clock_box("36000", Gtk.Align.START))
        if under_pointer:
            shown, more = self.entries(3)
            for entry in shown:
                widget = entry_widget(entry, "proto-plain", 26)
                widget.set_valign(Gtk.Align.CENTER)
                band.add(widget)
            if more:
                band.add(more_label(more))
        overlay.add_overlay(band)
        overlay.set_overlay_pass_through(band, True)
        if under_pointer and self.view == "dialog":
            self.dialog.set_margin_top(0)
            overlay.add_overlay(self.dialog)


def shots(window, directory):
    """Writes every layout in each view and mode, for checking the prototype draws."""
    os.makedirs(directory, exist_ok=True)
    cases = [
        ("D", "dialog", "full", 1, "real", "top-right", 1.0),
        ("D", "dialog", "full", 1, "real", "top-right", 0.4),
        ("D", "dialog", "full", 1, "busy", "top-right", 1.0),
        ("D", "dialog", "title", 1, "light", "top-right", 1.0),
    ]

    def step():
        if step.pending is not None:
            name = step.pending
            gdk_window = window.get_window()
            pixbuf = Gdk.pixbuf_get_from_window(gdk_window, 0, 0, gdk_window.get_width(), gdk_window.get_height())
            pixbuf.savev(os.path.join(directory, name + ".png"), "png", [], [])
        if not cases:
            Gtk.main_quit()
            return False
        variant, view, mode, outputs, wallpaper, corner, blur = cases.pop(0)
        Wallpaper.blur = blur
        window.variant, window.view, window.mode, window.outputs, window.wallpaper, window.corner = variant, view, mode, outputs, wallpaper, corner
        window.rebuild()
        step.pending = f"{variant}-{view}-{mode}-{outputs}-{wallpaper}-{blur}"
        return True

    step.pending = None
    GLib.timeout_add(400, step)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--variant", choices=VARIANTS, default="D")
    parser.add_argument("--view", choices=["idle", "dialog"], default="idle")
    parser.add_argument("--mode", choices=MODES, default="full")
    parser.add_argument("--outputs", type=int, choices=[1, 2], default=1)
    parser.add_argument("--wallpaper", choices=WALLPAPERS, default="real")
    parser.add_argument("--shots", help="write screenshots of every case here and exit")
    args = parser.parse_args()

    window = Prototype(args)
    window.show_all()
    window.hide_unused_dialog_parts()
    if args.shots:
        shots(window, args.shots)
    Gtk.main()


if __name__ == "__main__":
    main()
