/* Imports serve.py's menu with the applet's translator and draws it with menu-chords.vala, as
 * the applet does, then says what each item of File draws for its shortcut (145).
 *
 *   look <png>
 *
 * For each of three states of Open Folder... (a chord, one press, a chord again) it prints a
 * line of JSON per item: its label, the accelerator GTK's accel label draws, the chord drawn
 * beside it, and the x in the menu where that text ends. The first state's menu is drawn to
 * <png>. It exits 0 when every line is as expected and 1 otherwise.
 */

private Gtk.Menu? file_menu = null;

/* What GTK writes for each chord serve.py serves. */
private const string[] CHORDS = { "Ctrl+K Ctrl+O", "Ctrl+K F" };

private string? text_of(Gtk.Widget item)
{
    string? text = null;
    item.get("text", out text);
    return text;
}

private void find_shortcut(Gtk.Widget widget, ref string accel, ref string chord, ref int end, Gtk.Widget menu)
{
    if (widget is Gtk.AccelLabel)
    {
        uint key;
        Gdk.ModifierType mods;
        ((Gtk.AccelLabel) widget).get_accel(out key, out mods);
        if (key != 0)
        {
            accel = Gtk.accelerator_get_label(key, mods);
            int x, y;
            widget.translate_coordinates(menu, widget.get_allocated_width(), 0, out x, out y);
            /* The accel label draws its accelerator 3 pixels before its end. */
            end = x - 3;
        }
        return;
    }
    if (widget.get_type().name() == "AppmenuChordsChord")
    {
        /* menu-chords.vala's Chord: its text, then its padding of 3. */
        int x, y;
        widget.translate_coordinates(menu, widget.get_allocated_width(), 0, out x, out y);
        end = x - 3;
        /* Its text is its own, so it's known by its width: that of the text GTK writes. */
        int width = widget.get_allocated_width() - 3;
        chord = "%d px".printf(width);
        foreach (unowned string text in CHORDS)
        {
            int w, h;
            widget.create_pango_layout(text).get_pixel_size(out w, out h);
            if (w == width)
                chord = text;
        }
        return;
    }
    if (widget is Gtk.Container)
        foreach (var child in ((Gtk.Container) widget).get_children())
            find_shortcut(child, ref accel, ref chord, ref end, menu);
}

private string[] describe(Gtk.Menu menu)
{
    string[] lines = {};
    foreach (var item in menu.get_children())
    {
        string accel = "", chord = "";
        int end = -1;
        find_shortcut(item, ref accel, ref chord, ref end, menu);
        var builder = new Json.Builder();
        builder.begin_object();
        builder.set_member_name("label").add_string_value(text_of(item) ?? "");
        builder.set_member_name("accel").add_string_value(accel);
        builder.set_member_name("chord").add_string_value(chord);
        builder.set_member_name("end").add_int_value(end);
        builder.end_object();
        var gen = new Json.Generator();
        gen.set_root(builder.get_root());
        lines += gen.to_data(null);
    }
    return lines;
}

private async void wait(uint ms)
{
    Timeout.add(ms, () => { wait.callback(); return Source.REMOVE; });
    yield;
}

private async bool until(owned GLib.SourceFunc ready)
{
    for (int i = 0; i < 100; i++)
    {
        if (ready())
            return true;
        yield wait(50);
    }
    return false;
}

private string? chord_of(string label)
{
    foreach (var item in file_menu.get_children())
    {
        if (text_of(item) != label)
            continue;
        string accel = "", chord = "";
        int end = -1;
        find_shortcut(item, ref accel, ref chord, ref end, file_menu);
        return accel + "|" + chord;
    }
    return null;
}

private async int run(Gtk.Window window, Gtk.MenuBar bar, string png) throws Error
{
    var bus = yield Bus.get(BusType.SESSION);
    if (!yield until(() => {
        foreach (var top in bar.get_children())
        {
            var sub = ((Gtk.MenuItem) top).get_submenu() as Gtk.Menu;
            if (sub != null && sub.get_children().length() == 4)
            {
                file_menu = sub;
                return true;
            }
        }
        return false;
    }))
    {
        printerr("File's four items never came\n");
        return 1;
    }
    file_menu.popup_at_widget(window, Gdk.Gravity.NORTH_WEST, Gdk.Gravity.NORTH_WEST, null);
    yield until(() => file_menu.get_mapped());
    yield wait(200);

    int failed = 0;
    string[] states = {
        "chord",
        "[[\"Control\",\"o\"]]",
        "[[\"Control\",\"k\"],[\"Control\",\"o\"]]",
    };
    string[] expected = { "|Ctrl+K Ctrl+O", "Ctrl+O|", "|Ctrl+K Ctrl+O" };
    for (int s = 0; s < states.length; s++)
    {
        if (s > 0)
        {
            yield bus.call("org.caminito.Check145", "/MenuBar", "org.caminito.Check145", "Shortcut",
                           new Variant("(is)", 3, states[s]), null, DBusCallFlags.NONE, 2000);
            var want = expected[s];
            yield until(() => chord_of("Open Folder...") == want);
            yield wait(200);
        }
        print("# Open Folder... served as %s\n", states[s]);
        foreach (var line in describe(file_menu))
            print("%s\n", line);
        var got = chord_of("Open Folder...") ?? "";
        int end = -1;
        foreach (var item in file_menu.get_children())
        {
            string accel = "", chord = "";
            int here = -1;
            find_shortcut(item, ref accel, ref chord, ref here, file_menu);
            if (here != -1 && end != -1 && here != end)
            {
                printerr("%s's shortcut ends at %d, not %d\n", text_of(item), here, end);
                failed++;
            }
            if (here != -1)
                end = here;
        }
        if (chord_of("Close Folder") != "|Ctrl+K F")
        {
            printerr("Close Folder drew %s\n", chord_of("Close Folder"));
            failed++;
        }
        if (got != expected[s])
        {
            printerr("Open Folder... drew %s, not %s\n", got, expected[s]);
            failed++;
        }
        if (s == 0)
        {
            var top = file_menu.get_toplevel();
            var surface = new Cairo.ImageSurface(Cairo.Format.ARGB32, top.get_allocated_width(), top.get_allocated_height());
            var cr = new Cairo.Context(surface);
            top.draw(cr);
            surface.write_to_png(png);
        }
    }
    return failed == 0 ? 0 : 1;
}

int main(string[] args)
{
    Gtk.init(ref args);
    var window = new Gtk.Window();
    window.set_default_size(400, 300);
    var bar = new Gtk.MenuBar();
    window.add(bar);
    Appmenu.Chords.track(bar);
    var importer = new DBusMenu.Importer("org.caminito.Check145", "/MenuBar");
    importer.notify["model"].connect(() => {
        bar.insert_action_group("dbusmenu", importer.action_group);
        bar.bind_model(importer.model, null, true);
    });
    window.show_all();
    int status = 1;
    var loop = new MainLoop();
    run.begin(window, bar, args[1], (obj, res) => {
        try
        {
            status = run.end(res);
        }
        catch (Error e)
        {
            printerr("%s\n", e.message);
        }
        loop.quit();
    });
    loop.run();
    return status;
}
