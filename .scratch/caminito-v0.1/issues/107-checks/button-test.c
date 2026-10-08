/* mate-power-manager's keys grabbed through Caminito (107): its own gpm-button.c, with the
 * carried patch, against a stand-in for org.mate.Caminito on a private session bus. The
 * stand-in answers GrabAccelerators as Caminito does, an action id for each accelerator and
 * 0 for a refused one, and signals the grabber alone. What the test reads is GpmButton's
 * button-pressed, which gpm-manager, gpm-backlight and gpm-kbd-backlight act on.
 *
 *   button-test caminito   MATE_WM_NAME=Caminito: the keys are grabbed through the name
 *   button-test x11        without MATE_WM_NAME: nothing is asked of it, and the keys
 *                          are grabbed on X11 as before
 *
 * Both run on an X display, as mate-power-manager runs on XWayland under Caminito (78).
 *
 * run.sh builds it and runs both. */

#include <string.h>
#include <gio/gio.h>
#include <gdk/gdkx.h>
#include <gtk/gtk.h>
#include <X11/Xlib.h>
#include <X11/XF86keysym.h>

#include "gpm-button.h"

#define NAME "org.mate.Caminito"
#define PATH "/org/mate/Caminito"

/* Long enough for a call or a signal to cross the private bus many times over, for the
 * checks that something doesn't happen */
#define QUIET_MS 300
#define TIMEOUT_MS 2000

/* The keys, in the order gpm-button.c grabs them on X11, and what each one presses */
static const struct {
	const gchar *accelerator;
	const gchar *type;
} expected[] = {
	{ "XF86PowerOff", "power" },
	{ "XF86Suspend", "hibernate" },
	{ "XF86Sleep", "suspend" },
	{ "XF86Hibernate", "hibernate" },
	{ "XF86MonBrightnessUp", "brightness-up" },
	{ "XF86MonBrightnessDown", "brightness-down" },
	{ "XF86ScreenSaver", "lock" },
	{ "XF86Battery", "battery" },
	{ "XF86KbdBrightnessUp", "kbd-illum-up" },
	{ "XF86KbdBrightnessDown", "kbd-illum-down" },
	{ "XF86KbdLightOnOff", "kbd-illum-toggle" },
};
#define N_KEYS G_N_ELEMENTS (expected)

/* Another caller has it, so the stand-in refuses it */
#define REFUSED "XF86Battery"

typedef struct {
	GDBusConnection *connection;
	guint            object_id;
	/* The first action id of this owner's answers: each owner has ids of its own */
	guint32          base;
	gchar           *grabber;
	GPtrArray       *grabs;     /* of GStrv, one for each GrabAccelerators call */
	GArray          *ungrabbed; /* of guint32 */
} Stand_in;

static GPtrArray *pressed;

static const gchar introspection[] =
	"<node>"
	"  <interface name='org.mate.Caminito'>"
	"    <method name='GrabAccelerators'>"
	"      <arg type='as' direction='in'/>"
	"      <arg type='au' direction='out'/>"
	"    </method>"
	"    <method name='UngrabAccelerators'>"
	"      <arg type='au' direction='in'/>"
	"    </method>"
	"    <signal name='AcceleratorActivated'><arg type='u'/><arg type='u'/></signal>"
	"    <signal name='AcceleratorRepeated'><arg type='u'/><arg type='u'/></signal>"
	"  </interface>"
	"</node>";

static void
stand_in_call (GDBusConnection       *connection G_GNUC_UNUSED,
	       const gchar           *sender,
	       const gchar           *object_path G_GNUC_UNUSED,
	       const gchar           *interface_name G_GNUC_UNUSED,
	       const gchar           *method_name,
	       GVariant              *parameters,
	       GDBusMethodInvocation *invocation,
	       gpointer               data)
{
	Stand_in *stand_in = data;

	if (g_strcmp0 (method_name, "GrabAccelerators") == 0) {
		GVariantBuilder actions;
		gchar **accelerators;
		guint i;

		g_variant_get (parameters, "(^as)", &accelerators);
		g_ptr_array_add (stand_in->grabs, accelerators);
		g_free (stand_in->grabber);
		stand_in->grabber = g_strdup (sender);

		g_variant_builder_init (&actions, G_VARIANT_TYPE ("au"));
		for (i = 0; accelerators[i] != NULL; i++)
			g_variant_builder_add (&actions, "u",
					       g_strcmp0 (accelerators[i], REFUSED) == 0 ? 0 : stand_in->base + i);
		g_dbus_method_invocation_return_value (invocation, g_variant_new ("(au)", &actions));
	} else {
		GVariantIter *iter;
		guint32 action;

		g_variant_get (parameters, "(au)", &iter);
		while (g_variant_iter_next (iter, "u", &action))
			g_array_append_val (stand_in->ungrabbed, action);
		g_variant_iter_free (iter);
		g_dbus_method_invocation_return_value (invocation, NULL);
	}
}

static const GDBusInterfaceVTable vtable = { stand_in_call, NULL, NULL, { 0 } };

static GDBusConnection *
new_connection (void)
{
	GDBusConnection *connection;
	gchar *address;

	address = g_dbus_address_get_for_bus_sync (G_BUS_TYPE_SESSION, NULL, NULL);
	connection = g_dbus_connection_new_for_address_sync (address,
							     G_DBUS_CONNECTION_FLAGS_AUTHENTICATION_CLIENT |
							     G_DBUS_CONNECTION_FLAGS_MESSAGE_BUS_CONNECTION,
							     NULL, NULL, NULL);
	g_assert_nonnull (connection);
	g_free (address);
	return connection;
}

static Stand_in *
stand_in_new (guint32 base)
{
	static GDBusNodeInfo *info;
	Stand_in *stand_in = g_new0 (Stand_in, 1);

	if (info == NULL)
		info = g_dbus_node_info_new_for_xml (introspection, NULL);

	stand_in->connection = new_connection ();
	stand_in->base = base;
	stand_in->grabs = g_ptr_array_new_with_free_func ((GDestroyNotify) g_strfreev);
	stand_in->ungrabbed = g_array_new (FALSE, FALSE, sizeof (guint32));
	stand_in->object_id = g_dbus_connection_register_object (stand_in->connection, PATH,
								 info->interfaces[0], &vtable,
								 stand_in, NULL, NULL);
	g_assert_cmpuint (stand_in->object_id, >, 0);
	return stand_in;
}

static void
stand_in_own (Stand_in *stand_in)
{
	GVariant *reply;
	guint32 result;

	reply = g_dbus_connection_call_sync (stand_in->connection,
					     "org.freedesktop.DBus", "/org/freedesktop/DBus",
					     "org.freedesktop.DBus", "RequestName",
					     g_variant_new ("(su)", NAME, 0x4 /* DO_NOT_QUEUE */),
					     G_VARIANT_TYPE ("(u)"), G_DBUS_CALL_FLAGS_NONE, -1,
					     NULL, NULL);
	g_assert_nonnull (reply);
	g_variant_get (reply, "(u)", &result);
	g_assert_cmpuint (result, ==, 1 /* PRIMARY_OWNER */);
	g_variant_unref (reply);
}

/* Leaves the bus, as Caminito does when it ends */
static void
stand_in_free (Stand_in *stand_in)
{
	g_dbus_connection_unregister_object (stand_in->connection, stand_in->object_id);
	g_dbus_connection_close_sync (stand_in->connection, NULL, NULL);
	g_object_unref (stand_in->connection);
	g_ptr_array_unref (stand_in->grabs);
	g_array_unref (stand_in->ungrabbed);
	g_free (stand_in->grabber);
	g_free (stand_in);
}

static void
stand_in_signal (Stand_in *stand_in, const gchar *signal, guint32 action)
{
	g_assert_nonnull (stand_in->grabber);
	g_dbus_connection_emit_signal (stand_in->connection, stand_in->grabber, PATH, NAME,
				       signal, g_variant_new ("(uu)", action, 0), NULL);
	g_dbus_connection_flush_sync (stand_in->connection, NULL, NULL);
}

/* The action the stand-in gave the key with this accelerator, in its last answer */
static guint32
action_of (Stand_in *stand_in, const gchar *accelerator)
{
	guint i;

	for (i = 0; i < N_KEYS; i++)
		if (g_strcmp0 (expected[i].accelerator, accelerator) == 0)
			return stand_in->base + i;
	g_assert_not_reached ();
}

static gboolean
timed_out (gpointer data)
{
	*(gboolean *) data = TRUE;
	return G_SOURCE_REMOVE;
}

/* Runs the main loop until *count is at least @want, and fails after TIMEOUT_MS */
static void
wait_for (const guint *count, guint want, const gchar *what)
{
	gboolean out = FALSE;
	guint id = g_timeout_add (TIMEOUT_MS, timed_out, &out);

	while (*count < want && !out)
		g_main_context_iteration (NULL, TRUE);
	if (out)
		g_error ("timed out waiting for %s", what);
	g_source_remove (id);
}

/* Runs the main loop for QUIET_MS, for a check that nothing happens */
static void
wait_quiet (void)
{
	gboolean out = FALSE;

	g_timeout_add (QUIET_MS, timed_out, &out);
	while (!out)
		g_main_context_iteration (NULL, TRUE);
}

/* gpm-button drops a press of the type it pressed last within 125 ms, as a duplicate */
static void
wait_past_duplicates (void)
{
	g_usleep (150 * 1000);
}

static void
pressed_cb (GpmButton *button G_GNUC_UNUSED, const gchar *type, gpointer data G_GNUC_UNUSED)
{
	g_ptr_array_add (pressed, g_strdup (type));
}

static void
assert_pressed (const gchar *type)
{
	g_assert_cmpuint (pressed->len, >, 0);
	g_assert_cmpstr (g_ptr_array_index (pressed, pressed->len - 1), ==, type);
}

static gboolean bad_access;

static int
x_error (Display *display G_GNUC_UNUSED, XErrorEvent *event)
{
	if (event->error_code == BadAccess)
		bad_access = TRUE;
	return 0;
}

/* Whether another X client can grab a key gpm-button.c grabs on X11: it can't
 * while mate-power-manager has the key */
static gboolean
x11_key_is_free (KeySym keysym)
{
	Display *display = XOpenDisplay (NULL);
	KeyCode keycode;
	gboolean free;

	g_assert_nonnull (display);
	keycode = XKeysymToKeycode (display, keysym);
	g_assert_cmpuint (keycode, !=, 0);

	bad_access = FALSE;
	XSetErrorHandler (x_error);
	XGrabKey (display, keycode, AnyModifier, DefaultRootWindow (display), True,
		  GrabModeAsync, GrabModeAsync);
	XSync (display, False);
	free = !bad_access;
	XCloseDisplay (display);
	return free;
}

static void
ok (const gchar *name)
{
	g_print ("ok - %s\n", name);
}

static int
run_caminito (void)
{
	Stand_in *first, *second;
	GpmButton *button;
	gchar **grabbed;
	guint i, before;

	g_setenv ("MATE_WM_NAME", "Caminito", TRUE);

	first = stand_in_new (100);
	button = gpm_button_new ();
	g_signal_connect (button, "button-pressed", G_CALLBACK (pressed_cb), NULL);

	/* The name gets its owner after mate-power-manager has started */
	wait_quiet ();
	g_assert_cmpuint (first->grabs->len, ==, 0);
	stand_in_own (first);
	wait_for (&first->grabs->len, 1, "the grab");
	grabbed = g_ptr_array_index (first->grabs, 0);
	g_assert_cmpuint (g_strv_length (grabbed), ==, N_KEYS);
	for (i = 0; i < N_KEYS; i++)
		g_assert_cmpstr (grabbed[i], ==, expected[i].accelerator);
	wait_quiet ();
	g_assert_cmpuint (first->grabs->len, ==, 1);
	ok ("the keys are grabbed once org_mate_caminito has an owner");

	g_assert_true (x11_key_is_free (XF86XK_PowerOff));
	g_assert_true (x11_key_is_free (XF86XK_MonBrightnessUp));
	ok ("no key is grabbed on X11");

	for (i = 0; i < N_KEYS; i++) {
		if (g_strcmp0 (expected[i].accelerator, REFUSED) == 0)
			continue;
		before = pressed->len;
		stand_in_signal (first, "AcceleratorActivated", action_of (first, expected[i].accelerator));
		wait_for (&pressed->len, before + 1, expected[i].accelerator);
		assert_pressed (expected[i].type);
	}
	ok ("each grabbed key presses its button");

	before = pressed->len;
	stand_in_signal (first, "AcceleratorActivated", action_of (first, REFUSED));
	stand_in_signal (first, "AcceleratorActivated", 999);
	/* the refused key's action is 0, so 0 must match no key */
	stand_in_signal (first, "AcceleratorActivated", 0);
	wait_quiet ();
	g_assert_cmpuint (pressed->len, ==, before);
	ok ("a refused key, an unknown action and action 0 press nothing");

	{
		/* Another caller on the bus, signalling as if it were Caminito */
		GDBusConnection *other = new_connection ();

		g_dbus_connection_emit_signal (other, first->grabber, PATH, NAME,
					       "AcceleratorActivated",
					       g_variant_new ("(uu)",
							      action_of (first, "XF86PowerOff"), 0),
					       NULL);
		g_dbus_connection_flush_sync (other, NULL, NULL);
		wait_quiet ();
		g_assert_cmpuint (pressed->len, ==, before);
		g_object_unref (other);
	}
	ok ("a signal from another sender than the name's owner presses nothing");

	{
		static const gchar *repeat[] = {
			"XF86MonBrightnessUp", "XF86MonBrightnessDown",
			"XF86KbdBrightnessUp", "XF86KbdBrightnessDown",
		};

		for (i = 0; i < G_N_ELEMENTS (repeat); i++) {
			wait_past_duplicates ();
			before = pressed->len;
			stand_in_signal (first, "AcceleratorRepeated", action_of (first, repeat[i]));
			wait_for (&pressed->len, before + 1, repeat[i]);
		}
		assert_pressed ("kbd-illum-down");
	}
	ok ("a held brightness key repeats");

	{
		static const gchar *once[] = {
			"XF86PowerOff", "XF86Suspend", "XF86Sleep", "XF86Hibernate",
			"XF86ScreenSaver", "XF86KbdLightOnOff",
		};

		wait_past_duplicates ();
		before = pressed->len;
		for (i = 0; i < G_N_ELEMENTS (once); i++)
			stand_in_signal (first, "AcceleratorRepeated", action_of (first, once[i]));
		wait_quiet ();
		g_assert_cmpuint (pressed->len, ==, before);
	}
	ok ("a held power, sleep, lock or backlight toggle key doesn't repeat");

	{
		/* Two presses of one key 125 ms apart or less, as X11's grab gets them */
		wait_past_duplicates ();
		before = pressed->len;
		stand_in_signal (first, "AcceleratorActivated", action_of (first, "XF86MonBrightnessUp"));
		stand_in_signal (first, "AcceleratorActivated", action_of (first, "XF86MonBrightnessUp"));
		wait_for (&pressed->len, before + 1, "the first press");
		wait_quiet ();
		g_assert_cmpuint (pressed->len, ==, before + 1);
	}
	ok ("a press soon after the same key's is dropped as a duplicate, as on X11");

	/* Caminito restarts: the grabs went with the old owner */
	stand_in_free (first);
	second = stand_in_new (200);
	stand_in_own (second);
	wait_for (&second->grabs->len, 1, "the grab of the new owner");
	grabbed = g_ptr_array_index (second->grabs, 0);
	g_assert_cmpuint (g_strv_length (grabbed), ==, N_KEYS);
	ok ("the keys are grabbed again when org_mate_caminito gets a new owner");

	wait_past_duplicates ();
	before = pressed->len;
	stand_in_signal (second, "AcceleratorActivated", 100 + 4 /* the old owner's id */);
	wait_quiet ();
	g_assert_cmpuint (pressed->len, ==, before);
	stand_in_signal (second, "AcceleratorActivated", action_of (second, "XF86MonBrightnessUp"));
	wait_for (&pressed->len, before + 1, "the new owner's press");
	assert_pressed ("brightness-up");
	ok ("the new owner's actions press the keys, and the old owner's don't");

	g_object_unref (button);
	wait_for (&second->ungrabbed->len, N_KEYS - 1, "the release");
	for (i = 0; i < N_KEYS; i++) {
		guint32 action = action_of (second, expected[i].accelerator);
		gboolean found = FALSE;
		guint j;

		for (j = 0; j < second->ungrabbed->len; j++)
			if (g_array_index (second->ungrabbed, guint32, j) == action)
				found = TRUE;
		g_assert_true (found == (g_strcmp0 (expected[i].accelerator, REFUSED) != 0));
	}
	ok ("the grabs are released when the button goes");

	stand_in_free (second);
	return 0;
}

static int
run_x11 (void)
{
	Stand_in *stand_in;
	GpmButton *button;

	g_unsetenv ("MATE_WM_NAME");

	stand_in = stand_in_new (100);
	stand_in_own (stand_in);
	button = gpm_button_new ();
	wait_quiet ();
	g_assert_cmpuint (stand_in->grabs->len, ==, 0);
	ok ("without MATE_WM_NAME nothing is asked of org_mate_caminito");

	g_assert_false (x11_key_is_free (XF86XK_PowerOff));
	g_assert_false (x11_key_is_free (XF86XK_MonBrightnessUp));
	ok ("without MATE_WM_NAME the keys are grabbed on X11");

	g_object_unref (button);
	stand_in_free (stand_in);
	return 0;
}

int
main (int argc, char **argv)
{
	pressed = g_ptr_array_new_with_free_func (g_free);
	g_setenv ("DBUS_SYSTEM_BUS_ADDRESS", g_getenv ("DBUS_SESSION_BUS_ADDRESS"), TRUE);
	gtk_init (&argc, &argv);
	g_assert_true (GDK_IS_X11_DISPLAY (gdk_display_get_default ()));

	if (argc > 1 && strcmp (argv[1], "x11") == 0)
		return run_x11 ();
	return run_caminito ();
}
