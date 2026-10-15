/*
 * power-profiles-daemon as the popup reads it (264), against python-dbusmock's two
 * templates on a private bus that stands for the system bus: the daemon on either bus name
 * or both, a change made elsewhere, the call that sets the profile, and the daemon
 * stopping and starting.
 *
 *   test-ppd <python>
 */

#include <gio/gio.h>

#include "pw-ppd.h"

#define NEW_NAME "org.freedesktop.UPower.PowerProfiles"
#define NEW_PATH "/org/freedesktop/UPower/PowerProfiles"
#define OLD_NAME "net.hadess.PowerProfiles"
#define OLD_PATH "/net/hadess/PowerProfiles"

static const char *python;

typedef struct
{
  GTestDBus       *bus;
  GDBusConnection *connection;
  GSubprocess     *mocks[2];
  PwPpd           *ppd;
  guint            changes;
} Fixture;

typedef struct
{
  const char *template;
  const char *name;
  const char *path;
} Mock;

/* The new name's template first, as the popup prefers it. */
static const Mock mocks[] = {
  { "upower_power_profiles_daemon", NEW_NAME, NEW_PATH },
  { "power_profiles_daemon", OLD_NAME, OLD_PATH },
};

static void
on_changed (PwPpd *ppd G_GNUC_UNUSED, gpointer data)
{
  Fixture *fixture = data;

  fixture->changes++;
}

static void
start_mock (Fixture *fixture, guint which)
{
  g_autoptr (GError) error = NULL;

  fixture->mocks[which] = g_subprocess_new (G_SUBPROCESS_FLAGS_STDOUT_SILENCE, &error, python,
                                            "-m", "dbusmock", "--system", "--template",
                                            mocks[which].template, NULL);
  g_assert_no_error (error);
  for (int i = 0; i < 100; i++)
    {
      g_autoptr (GVariant) has = NULL;
      gboolean owned;

      has = g_dbus_connection_call_sync (fixture->connection, "org.freedesktop.DBus",
                                         "/org/freedesktop/DBus", "org.freedesktop.DBus",
                                         "NameHasOwner", g_variant_new ("(s)", mocks[which].name),
                                         G_VARIANT_TYPE ("(b)"), G_DBUS_CALL_FLAGS_NONE, -1,
                                         NULL, &error);
      g_assert_no_error (error);
      g_variant_get (has, "(b)", &owned);
      if (owned)
        return;
      g_usleep (100 * 1000);
    }
  g_error ("The mock never owned %s", mocks[which].name);
}

static void
stop_mock (Fixture *fixture, guint which)
{
  g_subprocess_force_exit (fixture->mocks[which]);
  g_subprocess_wait (fixture->mocks[which], NULL, NULL);
  g_clear_object (&fixture->mocks[which]);
}

/* Sets one of the mock's properties, as the daemon would on a change of its own. */
static void
set_property (Fixture *fixture, guint which, const char *property, GVariant *value)
{
  g_autoptr (GError) error = NULL;
  g_autoptr (GVariant) reply = NULL;

  reply = g_dbus_connection_call_sync (
    fixture->connection, mocks[which].name, mocks[which].path, "org.freedesktop.DBus.Properties",
    "Set", g_variant_new ("(ssv)", mocks[which].name, property, value), NULL,
    G_DBUS_CALL_FLAGS_NONE, -1, NULL, &error);
  g_assert_no_error (error);
}

static char *
get_active (Fixture *fixture, guint which)
{
  g_autoptr (GError) error = NULL;
  g_autoptr (GVariant) reply = NULL;
  g_autoptr (GVariant) value = NULL;

  reply = g_dbus_connection_call_sync (
    fixture->connection, mocks[which].name, mocks[which].path, "org.freedesktop.DBus.Properties",
    "Get", g_variant_new ("(ss)", mocks[which].name, "ActiveProfile"), G_VARIANT_TYPE ("(v)"),
    G_DBUS_CALL_FLAGS_NONE, -1, NULL, &error);
  g_assert_no_error (error);
  g_variant_get (reply, "(v)", &value);
  return g_variant_dup_string (value, NULL);
}

typedef gboolean (*Condition) (Fixture *fixture, gconstpointer expected);

/* Runs the main loop until condition holds, or fails after 5 s. */
static void
wait_until (Fixture *fixture, Condition condition, gconstpointer expected)
{
  gint64 deadline = g_get_monotonic_time () + 5 * G_USEC_PER_SEC;

  while (!condition (fixture, expected))
    {
      g_assert_cmpint (g_get_monotonic_time (), <, deadline);
      g_main_context_iteration (NULL, FALSE);
      g_usleep (10 * 1000);
    }
}

/* The profile the popup shows pressed now: its id, "none" for no tile pressed, or "no
   daemon". */
static char *
seen (Fixture *fixture)
{
  PwProfiles *profiles = pw_ppd_profiles (fixture->ppd);
  char *text;

  if (profiles == NULL)
    return g_strdup ("no daemon");
  text = g_strdup (profiles->active == PW_PROFILE_COUNT ? "none" : pw_profile_id (profiles->active));
  pw_profiles_free (profiles);
  return text;
}

static gboolean
active_is (Fixture *fixture, gconstpointer expected)
{
  g_autofree char *text = seen (fixture);

  return g_strcmp0 (text, expected) == 0;
}

static gboolean
mock_active_is (Fixture *fixture, gconstpointer expected)
{
  g_autofree char *text = get_active (fixture, 0);

  return g_strcmp0 (text, expected) == 0;
}

static gboolean
old_mock_active_is (Fixture *fixture, gconstpointer expected)
{
  g_autofree char *text = get_active (fixture, 1);

  return g_strcmp0 (text, expected) == 0;
}

static gboolean
has_changed (Fixture *fixture, gconstpointer expected G_GNUC_UNUSED)
{
  return fixture->changes > 0;
}

static gboolean
connection_gone (Fixture *fixture, gconstpointer expected G_GNUC_UNUSED)
{
  return fixture->connection == NULL;
}

static gboolean
degraded_is (Fixture *fixture, gconstpointer expected)
{
  PwProfiles *profiles = pw_ppd_profiles (fixture->ppd);
  gboolean same = profiles != NULL && g_strcmp0 (profiles->degraded, expected) == 0;

  pw_profiles_free (profiles);
  return same;
}

static gboolean
holds_are (Fixture *fixture, gconstpointer expected)
{
  PwProfiles *profiles = pw_ppd_profiles (fixture->ppd);
  g_autofree char *text = NULL;

  if (profiles != NULL)
    text = pw_profiles_holds_text (profiles, NULL, NULL);
  pw_profiles_free (profiles);
  return g_strcmp0 (text, expected) == 0;
}

static gboolean
three_are_listed (Fixture *fixture, gconstpointer expected G_GNUC_UNUSED)
{
  PwProfiles *profiles = pw_ppd_profiles (fixture->ppd);
  gboolean three = profiles != NULL && profiles->listed[PW_PROFILE_POWER_SAVER]
                   && profiles->listed[PW_PROFILE_BALANCED]
                   && profiles->listed[PW_PROFILE_PERFORMANCE];

  pw_profiles_free (profiles);
  return three;
}

static void
setup (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_autoptr (GError) error = NULL;

  fixture->bus = g_test_dbus_new (G_TEST_DBUS_NONE);
  g_test_dbus_up (fixture->bus);
  /* GDBus reads the system bus's address from here, and so does dbusmock. */
  g_setenv ("DBUS_SYSTEM_BUS_ADDRESS", g_test_dbus_get_bus_address (fixture->bus), TRUE);
  fixture->connection = g_bus_get_sync (G_BUS_TYPE_SYSTEM, NULL, &error);
  g_assert_no_error (error);
  /* The bus goes down at the end of each test, which mustn't end the test program. */
  g_dbus_connection_set_exit_on_close (fixture->connection, FALSE);
}

static void
teardown (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  g_clear_pointer (&fixture->ppd, pw_ppd_free);
  for (guint i = 0; i < G_N_ELEMENTS (mocks); i++)
    if (fixture->mocks[i] != NULL)
      stop_mock (fixture, i);
  /* GDBus keeps the system bus's connection while anything holds it, and the next test's
     bus is a new one: wait for this one to go before taking the bus down. */
  g_object_add_weak_pointer (G_OBJECT (fixture->connection), (gpointer *) &fixture->connection);
  g_object_unref (fixture->connection);
  wait_until (fixture, connection_gone, NULL);
  g_test_dbus_down (fixture->bus);
  g_clear_object (&fixture->bus);
}

static void
test_listed_and_followed (Fixture *fixture, gconstpointer data)
{
  guint which = GPOINTER_TO_UINT (data);
  GVariantBuilder builder;

  start_mock (fixture, which);
  fixture->ppd = pw_ppd_new (on_changed, fixture);
  wait_until (fixture, three_are_listed, NULL);
  wait_until (fixture, active_is, "balanced");
  wait_until (fixture, has_changed, NULL);

  /* A change made elsewhere: powerprofilesctl, a hotkey. */
  set_property (fixture, which, "ActiveProfile", g_variant_new_string ("performance"));
  wait_until (fixture, active_is, "performance");
  set_property (fixture, which, "PerformanceDegraded", g_variant_new_string ("lap-detected"));
  wait_until (fixture, degraded_is, "lap-detected");
  set_property (fixture, which, "PerformanceDegraded", g_variant_new_string (""));
  wait_until (fixture, degraded_is, "");

  g_variant_builder_init (&builder, G_VARIANT_TYPE ("aa{sv}"));
  g_variant_builder_open (&builder, G_VARIANT_TYPE ("a{sv}"));
  g_variant_builder_add (&builder, "{sv}", "ApplicationId",
                         g_variant_new_string ("org.gnome.GameMode"));
  g_variant_builder_add (&builder, "{sv}", "Profile", g_variant_new_string ("performance"));
  g_variant_builder_add (&builder, "{sv}", "Reason", g_variant_new_string ("a game"));
  g_variant_builder_close (&builder);
  set_property (fixture, which, "ActiveProfileHolds", g_variant_builder_end (&builder));
  wait_until (fixture, holds_are, "GameMode is holding Performance.");
}

static void
test_set (Fixture *fixture, gconstpointer data)
{
  guint which = GPOINTER_TO_UINT (data);
  Condition mock_is = which == 0 ? mock_active_is : old_mock_active_is;

  start_mock (fixture, which);
  fixture->ppd = pw_ppd_new (on_changed, fixture);
  wait_until (fixture, active_is, "balanced");

  /* The daemon's own ActiveProfile changes, and the popup follows it. */
  pw_ppd_set_active (fixture->ppd, PW_PROFILE_POWER_SAVER);
  wait_until (fixture, mock_is, "power-saver");
  wait_until (fixture, active_is, "power-saver");
  pw_ppd_set_active (fixture->ppd, PW_PROFILE_PERFORMANCE);
  wait_until (fixture, mock_is, "performance");
  wait_until (fixture, active_is, "performance");
}

static void
test_no_daemon (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  /* Nothing on the bus: no profiles, and nothing started. A click goes nowhere. */
  fixture->ppd = pw_ppd_new (on_changed, fixture);
  wait_until (fixture, active_is, "no daemon");
  pw_ppd_set_active (fixture->ppd, PW_PROFILE_PERFORMANCE);
  /* A call that went out would be answered, or fail, within a few milliseconds on a private
     bus: a fifth of a second of the loop lets a stray one show up as a crash or a mock. */
  for (int i = 0; i < 20; i++)
    {
      g_main_context_iteration (NULL, FALSE);
      g_usleep (10 * 1000);
    }
  wait_until (fixture, active_is, "no daemon");

  /* The daemon comes after the applet. */
  start_mock (fixture, 0);
  wait_until (fixture, active_is, "balanced");
}

static void
test_restart (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture, 0);
  fixture->ppd = pw_ppd_new (on_changed, fixture);
  wait_until (fixture, active_is, "balanced");

  stop_mock (fixture, 0);
  wait_until (fixture, active_is, "no daemon");

  start_mock (fixture, 0);
  wait_until (fixture, active_is, "balanced");
  /* It's the new one that's followed. */
  set_property (fixture, 0, "ActiveProfile", g_variant_new_string ("power-saver"));
  wait_until (fixture, active_is, "power-saver");
}

static void
test_both_names (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture, 1);
  fixture->ppd = pw_ppd_new (on_changed, fixture);
  wait_until (fixture, active_is, "balanced");
  set_property (fixture, 1, "ActiveProfile", g_variant_new_string ("power-saver"));
  wait_until (fixture, active_is, "power-saver");

  /* tuned-ppd or the new daemon comes up under the name that's preferred. */
  start_mock (fixture, 0);
  wait_until (fixture, active_is, "balanced");
  set_property (fixture, 1, "ActiveProfile", g_variant_new_string ("performance"));
  set_property (fixture, 0, "ActiveProfile", g_variant_new_string ("power-saver"));
  wait_until (fixture, active_is, "power-saver");
  pw_ppd_set_active (fixture->ppd, PW_PROFILE_PERFORMANCE);
  wait_until (fixture, mock_active_is, "performance");
  wait_until (fixture, active_is, "performance");
  /* The other is left alone. */
  set_property (fixture, 1, "ActiveProfile", g_variant_new_string ("balanced"));
  pw_ppd_set_active (fixture->ppd, PW_PROFILE_POWER_SAVER);
  wait_until (fixture, mock_active_is, "power-saver");
  g_assert_true (old_mock_active_is (fixture, "balanced"));

  /* The preferred one goes, and the other is followed again. */
  stop_mock (fixture, 0);
  wait_until (fixture, active_is, "balanced");
}

static void
test_free_early (Fixture *fixture, gconstpointer data G_GNUC_UNUSED)
{
  start_mock (fixture, 0);
  fixture->ppd = pw_ppd_new (on_changed, fixture);
  /* Freed before the proxy is up: nothing may call into it afterwards. */
  g_clear_pointer (&fixture->ppd, pw_ppd_free);
  /* The proxy's reply, if it came, would come within a few milliseconds on a private bus:
     half a second of the loop gives a stray callback ample time to crash the test. */
  for (int i = 0; i < 50; i++)
    {
      g_main_context_iteration (NULL, FALSE);
      g_usleep (10 * 1000);
    }
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  if (argc != 2)
    g_error ("usage: test-ppd <python>");
  python = argv[1];
  g_test_add ("/ppd/the-new-name-is-listed-and-followed", Fixture, GUINT_TO_POINTER (0), setup, test_listed_and_followed, teardown);
  g_test_add ("/ppd/the-old-name-is-listed-and-followed", Fixture, GUINT_TO_POINTER (1), setup, test_listed_and_followed, teardown);
  g_test_add ("/ppd/the-new-name-is-set", Fixture, GUINT_TO_POINTER (0), setup, test_set, teardown);
  g_test_add ("/ppd/the-old-name-is-set", Fixture, GUINT_TO_POINTER (1), setup, test_set, teardown);
  g_test_add ("/ppd/no-daemon-is-none-and-a-late-one-is-followed", Fixture, NULL, setup, test_no_daemon, teardown);
  g_test_add ("/ppd/the-daemon-stopping-and-starting-again-is-followed", Fixture, NULL, setup, test_restart, teardown);
  g_test_add ("/ppd/the-new-name-is-preferred-to-the-old", Fixture, NULL, setup, test_both_names, teardown);
  g_test_add ("/ppd/freeing-while-starting-calls-nothing", Fixture, NULL, setup, test_free_early, teardown);
  return g_test_run ();
}
