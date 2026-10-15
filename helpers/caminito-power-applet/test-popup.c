/*
 * The Power popup's table of batteries (263), from devices built by hand: what it lists,
 * and what its controls call. Without a display, GTK can't start, and it's skipped.
 */

#include "pw-popup.h"

#define BAT0  "/org/freedesktop/UPower/devices/battery_BAT0"
#define BAT1  "/org/freedesktop/UPower/devices/battery_BAT1"
#define MOUSE "/org/freedesktop/UPower/devices/mouse_0"

typedef struct
{
  GtkWidget           *anchor;
  CaminitoAppletPopup *popup;
  PwPopup             *content;
  GString             *calls;
} Fixture;

static void
on_configure (gpointer data)
{
  g_string_append (((Fixture *) data)->calls, "configure;");
}

static void
on_set_stay_awake (gboolean on, gpointer data)
{
  g_string_append_printf (((Fixture *) data)->calls, "awake %d;", on);
}

static void
on_set_limit (const char *path, gboolean enable, gpointer data)
{
  g_string_append_printf (((Fixture *) data)->calls, "limit %s %d;", path, enable);
}

static void
setup (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  static const PwPopupActions actions = { on_configure, on_set_stay_awake, on_set_limit };

  fixture->anchor = g_object_ref_sink (gtk_button_new ());
  fixture->popup = caminito_applet_popup_new (fixture->anchor);
  fixture->calls = g_string_new (NULL);
  fixture->content = pw_popup_new (fixture->popup, &actions, fixture);
}

static void
teardown (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  pw_popup_free (fixture->content);
  /* The popup goes with its anchor. */
  g_object_unref (fixture->anchor);
  g_string_free (fixture->calls, TRUE);
}

static PwDevice *
device_new (const char *path, guint type, gboolean supply, PwState state, double percentage,
            const char *model)
{
  PwDevice *device = pw_device_new ();

  device->path = g_strdup (path);
  device->type = type;
  device->power_supply = supply;
  device->present = TRUE;
  device->state = state;
  device->percentage = percentage;
  device->has_percentage = TRUE;
  device->model = g_strdup (model);
  return device;
}

static PwDevice *
battery_new (const char *path, double percentage, gboolean limit_supported)
{
  PwDevice *device = device_new (path, PW_TYPE_BATTERY, TRUE, PW_STATE_DISCHARGING, percentage,
                                 "BATTERY");

  device->capacity = 58;
  device->threshold_supported = limit_supported;
  device->threshold_end = 100;
  return device;
}

static GPtrArray *
devices_new (PwDevice *first, ...)
{
  GPtrArray *devices = g_ptr_array_new_with_free_func ((GDestroyNotify) pw_device_free);
  va_list args;

  va_start (args, first);
  for (PwDevice *device = first; device != NULL; device = va_arg (args, PwDevice *))
    g_ptr_array_add (devices, device);
  va_end (args);
  return devices;
}

static int
by_cell (gconstpointer a, gconstpointer b)
{
  GtkWidget *x = (GtkWidget *) a;
  GtkWidget *y = (GtkWidget *) b;
  GtkContainer *grid = GTK_CONTAINER (gtk_widget_get_parent (x));
  int x_top, x_left, y_top, y_left;

  gtk_container_child_get (grid, x, "top-attach", &x_top, "left-attach", &x_left, NULL);
  gtk_container_child_get (grid, y, "top-attach", &y_top, "left-attach", &y_left, NULL);
  return x_top != y_top ? x_top - y_top : x_left - y_left;
}

/* Every widget of a type under widget, in the order they are drawn: a grid's cells row by
   row, where GTK lists them last first. */
static void
collect (GtkWidget *widget, gpointer data)
{
  GPtrArray *found = ((gpointer *) data)[1];

  if (g_type_is_a (G_OBJECT_TYPE (widget), GPOINTER_TO_SIZE (((gpointer *) data)[0])))
    g_ptr_array_add (found, widget);
  if (GTK_IS_GRID (widget))
    {
      GList *cells = g_list_sort (gtk_container_get_children (GTK_CONTAINER (widget)), by_cell);

      for (GList *l = cells; l != NULL; l = l->next)
        collect (l->data, data);
      g_list_free (cells);
    }
  else if (GTK_IS_CONTAINER (widget))
    gtk_container_forall (GTK_CONTAINER (widget), collect, data);
}

static GPtrArray *
find_all (GtkWidget *widget, GType type)
{
  GPtrArray *found = g_ptr_array_new ();
  gpointer data[2] = { GSIZE_TO_POINTER (type), found };

  collect (widget, data);
  return found;
}

static GtkWidget *
find (GtkWidget *widget, GType type)
{
  g_autoptr (GPtrArray) all = find_all (widget, type);

  return all->len > 0 ? g_ptr_array_index (all, 0) : NULL;
}

static GtkWidget *
content (Fixture *fixture)
{
  return gtk_bin_get_child (GTK_BIN (caminito_applet_popup_get_window (fixture->popup)));
}

/* Every label's text under widget, in order, joined with "|". */
static char *
texts (GtkWidget *widget)
{
  g_autoptr (GPtrArray) labels = find_all (widget, GTK_TYPE_LABEL);
  GString *joined = g_string_new (NULL);

  for (guint i = 0; i < labels->len; i++)
    g_string_append_printf (joined, "%s%s", i > 0 ? "|" : "",
                            gtk_label_get_text (g_ptr_array_index (labels, i)));
  return g_string_free (joined, FALSE);
}

/* The label with this text. */
static GtkWidget *
find_label (GtkWidget *widget, const char *text)
{
  g_autoptr (GPtrArray) labels = find_all (widget, GTK_TYPE_LABEL);

  for (guint i = 0; i < labels->len; i++)
    if (g_str_equal (gtk_label_get_text (g_ptr_array_index (labels, i)), text))
      return g_ptr_array_index (labels, i);
  return NULL;
}

/* The table of batteries: the grid with the bars in it, NULL where there is none. The popup
   has the power profile and Stay awake above it, and those aren't the table's. */
static GtkWidget *
table_of (Fixture *fixture)
{
  GtkWidget *bar = find (content (fixture), GTK_TYPE_LEVEL_BAR);

  return bar != NULL ? gtk_widget_get_ancestor (bar, GTK_TYPE_GRID) : NULL;
}

/* The ⋮ buttons, a laptop battery's each, in the order of the table. */
static GPtrArray *
menu_buttons (Fixture *fixture)
{
  return find_all (content (fixture), GTK_TYPE_MENU_BUTTON);
}

static guint
menu_buttons_count (Fixture *fixture)
{
  g_autoptr (GPtrArray) buttons = menu_buttons (fixture);

  return buttons->len;
}

static GtkWidget *
menu_of (GtkWidget *button)
{
  return GTK_WIDGET (gtk_menu_button_get_popover (GTK_MENU_BUTTON (button)));
}

static void
test_lists_laptop_batteries_then_peripherals (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (GPtrArray) devices = devices_new (
    device_new (MOUSE, PW_TYPE_MOUSE, FALSE, PW_STATE_DISCHARGING, 55, "MX Master 3S"),
    battery_new (BAT1, 80, FALSE),
    battery_new (BAT0, 73, TRUE),
    NULL);
  g_autofree char *all = NULL;
  g_autoptr (GPtrArray) buttons = NULL;

  pw_popup_update_devices (fixture->content, devices);
  all = texts (table_of (fixture));
  /* Each row's name and state and percentage, and a ⋮ on the laptop's. */
  g_assert_cmpstr (all, ==,
                   "Battery 1|Discharging|73%|⋮"
                   "|Battery 2|Discharging|80%|⋮"
                   "|MX Master 3S|Discharging|55%");
  buttons = menu_buttons (fixture);
  g_assert_cmpuint (buttons->len, ==, 2);
}

static void
test_no_batteries_is_no_table (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (GPtrArray) devices = devices_new (NULL);
  g_autoptr (GPtrArray) batteries = devices_new (battery_new (BAT0, 73, FALSE), NULL);
  g_autofree char *all = NULL;

  pw_popup_update_devices (fixture->content, devices);
  all = texts (content (fixture));
  /* Nothing of the table: no bars, no ⋮. */
  g_assert_null (strstr (all, "73%"));
  g_assert_null (table_of (fixture));
  g_assert_cmpuint (menu_buttons_count (fixture), ==, 0);

  /* And back: the table is there when a battery is. */
  pw_popup_update_devices (fixture->content, batteries);
  g_assert_nonnull (find (content (fixture), GTK_TYPE_LEVEL_BAR));
  pw_popup_update_devices (fixture->content, devices);
  g_assert_null (find (content (fixture), GTK_TYPE_LEVEL_BAR));
}

static void
test_only_peripherals (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (GPtrArray) devices = devices_new (
    device_new (MOUSE, PW_TYPE_MOUSE, FALSE, PW_STATE_DISCHARGING, 55, "MX Master 3S"), NULL);
  g_autofree char *all = NULL;
  g_autoptr (GPtrArray) buttons = NULL;

  pw_popup_update_devices (fixture->content, devices);
  all = texts (table_of (fixture));
  g_assert_cmpstr (all, ==, "MX Master 3S|Discharging|55%");
  buttons = menu_buttons (fixture);
  g_assert_cmpuint (buttons->len, ==, 0);
}

static void
test_bar_follows_the_percentage (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (GPtrArray) devices = devices_new (battery_new (BAT0, 73, FALSE), NULL);

  pw_popup_update_devices (fixture->content, devices);
  g_assert_cmpfloat_with_epsilon (
    gtk_level_bar_get_value (GTK_LEVEL_BAR (find (content (fixture), GTK_TYPE_LEVEL_BAR))),
    0.73, 0.0001);
}

static void
test_below_20_is_in_the_warning_colour (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (GPtrArray) devices = devices_new (
    battery_new (BAT0, 73, FALSE),
    device_new (MOUSE, PW_TYPE_MOUSE, FALSE, PW_STATE_DISCHARGING, 12, "MX Master 3S"),
    NULL);
  GtkStyleContext *low, *fine;

  pw_popup_update_devices (fixture->content, devices);
  low = gtk_widget_get_style_context (find_label (content (fixture), "12%"));
  fine = gtk_widget_get_style_context (find_label (content (fixture), "73%"));
  g_assert_true (gtk_style_context_has_class (low, "caminito-power-low"));
  g_assert_false (gtk_style_context_has_class (fine, "caminito-power-low"));
}

static void
test_menu_has_health_and_the_switch_where_supported (Fixture *fixture,
                                                      gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (GPtrArray) devices = devices_new (battery_new (BAT0, 73, TRUE),
                                               battery_new (BAT1, 80, FALSE), NULL);
  g_autoptr (GPtrArray) buttons = NULL;
  g_autofree char *with = NULL;
  g_autofree char *without = NULL;

  pw_popup_update_devices (fixture->content, devices);
  buttons = menu_buttons (fixture);
  g_assert_cmpuint (buttons->len, ==, 2);
  with = texts (menu_of (g_ptr_array_index (buttons, 0)));
  without = texts (menu_of (g_ptr_array_index (buttons, 1)));
  g_assert_cmpstr (with, ==, "Health 58%|Limit charging to 80%");
  g_assert_nonnull (find (menu_of (g_ptr_array_index (buttons, 0)), GTK_TYPE_SWITCH));
  /* Where the charge limit isn't supported: health alone, and no switch. */
  g_assert_cmpstr (without, ==, "Health 58%");
  g_assert_null (find (menu_of (g_ptr_array_index (buttons, 1)), GTK_TYPE_SWITCH));
}

static void
test_limit_is_in_the_state_and_the_switch (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  PwDevice *battery = battery_new (BAT0, 80, TRUE);
  g_autoptr (GPtrArray) devices = devices_new (battery, NULL);
  g_autoptr (GPtrArray) buttons = NULL;

  battery->state = PW_STATE_PENDING_CHARGE;
  battery->threshold_enabled = TRUE;
  battery->threshold_end = 80;
  pw_popup_update_devices (fixture->content, devices);
  g_assert_nonnull (find_label (content (fixture), "Not charging · limit 80%"));
  buttons = menu_buttons (fixture);
  g_assert_true (gtk_switch_get_active (
    GTK_SWITCH (find (menu_of (g_ptr_array_index (buttons, 0)), GTK_TYPE_SWITCH))));
}

static GtkWidget *
limit_switch (Fixture *fixture)
{
  g_autoptr (GPtrArray) buttons = menu_buttons (fixture);

  return find (menu_of (g_ptr_array_index (buttons, 0)), GTK_TYPE_SWITCH);
}

static void
test_switch_calls_enable_charge_threshold (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (GPtrArray) devices = devices_new (battery_new (BAT0, 73, TRUE), NULL);

  pw_popup_update_devices (fixture->content, devices);
  gtk_switch_set_active (GTK_SWITCH (limit_switch (fixture)), TRUE);
  g_assert_cmpstr (fixture->calls->str, ==, "limit " BAT0 " 1;");
  /* Once UPower says so, it's on in the state's line too, and off is a call again. */
  g_assert_true (gtk_switch_get_active (GTK_SWITCH (limit_switch (fixture))));
  g_assert_false (gtk_widget_get_sensitive (limit_switch (fixture)));
  pw_popup_limit_done (fixture->content, BAT0, NULL);
  g_assert_true (gtk_switch_get_active (GTK_SWITCH (limit_switch (fixture))));
  g_assert_true (gtk_widget_get_sensitive (limit_switch (fixture)));

  g_string_truncate (fixture->calls, 0);
  gtk_switch_set_active (GTK_SWITCH (limit_switch (fixture)), FALSE);
  g_assert_cmpstr (fixture->calls->str, ==, "limit " BAT0 " 0;");
}

static void
test_refusal_leaves_the_switch_as_it_was (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (GPtrArray) devices = devices_new (battery_new (BAT0, 73, TRUE), NULL);
  g_autoptr (GPtrArray) buttons = NULL;

  pw_popup_update_devices (fixture->content, devices);
  gtk_switch_set_active (GTK_SWITCH (limit_switch (fixture)), TRUE);
  pw_popup_limit_done (fixture->content, BAT0, "Not authorized");

  g_assert_false (gtk_switch_get_active (GTK_SWITCH (limit_switch (fixture))));
  g_assert_true (gtk_widget_get_sensitive (limit_switch (fixture)));
  buttons = menu_buttons (fixture);
  g_assert_nonnull (find_label (menu_of (g_ptr_array_index (buttons, 0)), "Not authorized"));
  /* The state's line never said there was a limit. */
  g_assert_nonnull (find_label (content (fixture), "Discharging"));
  /* The next try starts clean. */
  gtk_switch_set_active (GTK_SWITCH (limit_switch (fixture)), TRUE);
  g_assert_null (find_label (content (fixture), "Not authorized"));
}

/* UPower's change for a limit that was turned on, after the answer, is the same as the
   switch says: nothing moves. */
static void
test_upower_catching_up_keeps_the_switch (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  PwDevice *battery = battery_new (BAT0, 73, TRUE);
  g_autoptr (GPtrArray) devices = devices_new (battery, NULL);
  g_autoptr (GPtrArray) later = NULL;
  PwDevice *changed = battery_new (BAT0, 73, TRUE);

  pw_popup_update_devices (fixture->content, devices);
  gtk_switch_set_active (GTK_SWITCH (limit_switch (fixture)), TRUE);
  pw_popup_limit_done (fixture->content, BAT0, NULL);
  changed->threshold_enabled = TRUE;
  changed->threshold_end = 80;
  later = devices_new (changed, NULL);
  pw_popup_update_devices (fixture->content, later);
  g_assert_true (gtk_switch_get_active (GTK_SWITCH (limit_switch (fixture))));
  g_assert_nonnull (find_label (content (fixture), "Discharging · limit 80%"));
}

static void
test_an_unchanged_table_is_not_drawn_again (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (GPtrArray) devices = devices_new (battery_new (BAT0, 73, TRUE), NULL);
  g_autoptr (GPtrArray) again = devices_new (battery_new (BAT0, 73, TRUE), NULL);
  GtkWidget *before;

  pw_popup_update_devices (fixture->content, devices);
  before = find (content (fixture), GTK_TYPE_LEVEL_BAR);
  /* What the table doesn't show, the time left, which UPower sends every minute, is not
     in PwDevice: the same devices again. */
  pw_popup_update_devices (fixture->content, again);
  g_assert_true (find (content (fixture), GTK_TYPE_LEVEL_BAR) == before);
}

/* The menu that is open stays open through UPower's changes, which draw the table again. */
static void
test_open_menu_survives_a_change (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (GPtrArray) devices = devices_new (battery_new (BAT0, 73, TRUE),
                                               battery_new (BAT1, 80, TRUE), NULL);
  g_autoptr (GPtrArray) later = devices_new (battery_new (BAT0, 72, TRUE),
                                             battery_new (BAT1, 80, TRUE), NULL);
  g_autoptr (GPtrArray) buttons = NULL;
  g_autoptr (GPtrArray) after = NULL;

  pw_popup_update_devices (fixture->content, devices);
  buttons = menu_buttons (fixture);
  gtk_toggle_button_set_active (g_ptr_array_index (buttons, 1), TRUE);

  pw_popup_update_devices (fixture->content, later);
  after = menu_buttons (fixture);
  g_assert_false (gtk_toggle_button_get_active (g_ptr_array_index (after, 0)));
  g_assert_true (gtk_toggle_button_get_active (g_ptr_array_index (after, 1)));
  g_assert_nonnull (find_label (content (fixture), "72%"));

  /* One menu at a time. */
  gtk_toggle_button_set_active (g_ptr_array_index (after, 0), TRUE);
  g_assert_false (gtk_toggle_button_get_active (g_ptr_array_index (after, 1)));
  /* And closed, it stays closed. */
  gtk_toggle_button_set_active (g_ptr_array_index (after, 0), FALSE);
  g_clear_pointer (&later, g_ptr_array_unref);
  later = devices_new (battery_new (BAT0, 71, TRUE), battery_new (BAT1, 80, TRUE), NULL);
  pw_popup_update_devices (fixture->content, later);
  g_clear_pointer (&after, g_ptr_array_unref);
  after = menu_buttons (fixture);
  g_assert_false (gtk_toggle_button_get_active (g_ptr_array_index (after, 0)));
  g_assert_false (gtk_toggle_button_get_active (g_ptr_array_index (after, 1)));
}

static void
test_a_mouse_turned_off_leaves_the_table (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  PwDevice *mouse = device_new (MOUSE, PW_TYPE_MOUSE, FALSE, PW_STATE_DISCHARGING, 55, "MX");
  g_autoptr (GPtrArray) devices = devices_new (battery_new (BAT0, 73, FALSE), mouse, NULL);
  g_autoptr (GPtrArray) later = devices_new (battery_new (BAT0, 73, FALSE), NULL);

  pw_popup_update_devices (fixture->content, devices);
  g_assert_nonnull (find_label (content (fixture), "MX"));
  pw_popup_update_devices (fixture->content, later);
  g_assert_null (find_label (content (fixture), "MX"));
}

static void
test_footer_runs_the_program_and_closes (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  GtkWidget *label = find_label (content (fixture), "Configure Power Management…");

  gtk_button_clicked (GTK_BUTTON (gtk_widget_get_ancestor (label, GTK_TYPE_BUTTON)));
  g_assert_cmpstr (fixture->calls->str, ==, "configure;");
  g_assert_false (caminito_applet_popup_is_shown (fixture->popup));
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  if (!gtk_init_check (&argc, &argv))
    {
      g_test_skip ("no display");
      return g_test_run ();
    }
  g_test_add ("/popup/lists-laptop-batteries-then-peripherals", Fixture, NULL, setup, test_lists_laptop_batteries_then_peripherals, teardown);
  g_test_add ("/popup/no-battery-is-no-table", Fixture, NULL, setup, test_no_batteries_is_no_table, teardown);
  g_test_add ("/popup/peripherals-alone-have-no-menu", Fixture, NULL, setup, test_only_peripherals, teardown);
  g_test_add ("/popup/the-bar-follows-the-percentage", Fixture, NULL, setup, test_bar_follows_the_percentage, teardown);
  g_test_add ("/popup/below-20-is-in-the-warning-colour", Fixture, NULL, setup, test_below_20_is_in_the_warning_colour, teardown);
  g_test_add ("/popup/the-menu-has-health-and-the-switch-where-supported", Fixture, NULL, setup, test_menu_has_health_and_the_switch_where_supported, teardown);
  g_test_add ("/popup/the-limit-is-in-the-state-and-the-switch", Fixture, NULL, setup, test_limit_is_in_the_state_and_the_switch, teardown);
  g_test_add ("/popup/the-switch-calls-enable-charge-threshold", Fixture, NULL, setup, test_switch_calls_enable_charge_threshold, teardown);
  g_test_add ("/popup/a-refusal-leaves-the-switch-as-it-was", Fixture, NULL, setup, test_refusal_leaves_the_switch_as_it_was, teardown);
  g_test_add ("/popup/upower-catching-up-keeps-the-switch", Fixture, NULL, setup, test_upower_catching_up_keeps_the_switch, teardown);
  g_test_add ("/popup/an-unchanged-table-is-not-drawn-again", Fixture, NULL, setup, test_an_unchanged_table_is_not_drawn_again, teardown);
  g_test_add ("/popup/an-open-menu-survives-a-change", Fixture, NULL, setup, test_open_menu_survives_a_change, teardown);
  g_test_add ("/popup/a-mouse-turned-off-leaves-the-table", Fixture, NULL, setup, test_a_mouse_turned_off_leaves_the_table, teardown);
  g_test_add ("/popup/the-footer-runs-the-program-and-closes", Fixture, NULL, setup, test_footer_runs_the_program_and_closes, teardown);
  return g_test_run ();
}
