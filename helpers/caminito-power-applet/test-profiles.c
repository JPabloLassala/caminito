/*
 * The popup's power profiles (264): which tiles the daemon's variants give, and the text
 * under them, from variants built by hand.
 */

#include "pw-profiles.h"

static GVariant *
listed (const char *const *ids)
{
  GVariantBuilder builder;

  g_variant_builder_init (&builder, G_VARIANT_TYPE ("aa{sv}"));
  for (; *ids != NULL; ids++)
    {
      g_variant_builder_open (&builder, G_VARIANT_TYPE ("a{sv}"));
      g_variant_builder_add (&builder, "{sv}", "Profile", g_variant_new_string (*ids));
      g_variant_builder_add (&builder, "{sv}", "Driver", g_variant_new_string ("test"));
      g_variant_builder_close (&builder);
    }
  return g_variant_ref_sink (g_variant_builder_end (&builder));
}

static GVariant *
holds (const char *application_id, const char *profile)
{
  GVariantBuilder builder;

  g_variant_builder_init (&builder, G_VARIANT_TYPE ("aa{sv}"));
  if (application_id != NULL)
    {
      g_variant_builder_open (&builder, G_VARIANT_TYPE ("a{sv}"));
      g_variant_builder_add (&builder, "{sv}", "ApplicationId", g_variant_new_string (application_id));
      g_variant_builder_add (&builder, "{sv}", "Profile", g_variant_new_string (profile));
      g_variant_builder_add (&builder, "{sv}", "Reason", g_variant_new_string ("a game"));
      g_variant_builder_close (&builder);
    }
  return g_variant_ref_sink (g_variant_builder_end (&builder));
}

static PwProfiles *
make (const char *const *ids, const char *active, const char *degraded, GVariant *hold_list)
{
  g_autoptr (GVariant) list = listed (ids);
  g_autoptr (GVariant) active_value = g_variant_ref_sink (g_variant_new_string (active));
  g_autoptr (GVariant) degraded_value = g_variant_ref_sink (g_variant_new_string (degraded));

  return pw_profiles_new (list, active_value, degraded_value, hold_list);
}

static void
test_three (void)
{
  const char *const ids[] = { "power-saver", "balanced", "performance", NULL };
  g_autoptr (GVariant) none = holds (NULL, NULL);
  PwProfiles *profiles = make (ids, "balanced", "", none);

  g_assert_true (profiles->listed[PW_PROFILE_POWER_SAVER]);
  g_assert_true (profiles->listed[PW_PROFILE_BALANCED]);
  g_assert_true (profiles->listed[PW_PROFILE_PERFORMANCE]);
  g_assert_cmpint (profiles->active, ==, PW_PROFILE_BALANCED);
  g_assert_true (pw_profiles_any (profiles));
  g_assert_null (pw_profiles_degraded_text (profiles));
  g_assert_null (pw_profiles_holds_text (profiles, NULL, NULL));
  pw_profiles_free (profiles);
}

static void
test_two (void)
{
  /* The daemon has no driver for Performance; a name that isn't one of the three is no
     tile, and a profile that isn't listed isn't active. */
  const char *const ids[] = { "power-saver", "balanced", "turbo", NULL };
  g_autoptr (GVariant) none = holds (NULL, NULL);
  PwProfiles *profiles = make (ids, "performance", "", none);

  g_assert_true (profiles->listed[PW_PROFILE_POWER_SAVER]);
  g_assert_true (profiles->listed[PW_PROFILE_BALANCED]);
  g_assert_false (profiles->listed[PW_PROFILE_PERFORMANCE]);
  g_assert_cmpint (profiles->active, ==, PW_PROFILE_COUNT);
  pw_profiles_free (profiles);
}

static void
test_none (void)
{
  const char *const ids[] = { NULL };
  PwProfiles *profiles = pw_profiles_new (NULL, NULL, NULL, NULL);
  PwProfiles *empty = make (ids, "balanced", "", NULL);

  g_assert_false (pw_profiles_any (profiles));
  g_assert_false (pw_profiles_any (empty));
  g_assert_cmpint (profiles->active, ==, PW_PROFILE_COUNT);
  pw_profiles_free (profiles);
  pw_profiles_free (empty);
}

static void
test_degraded (void)
{
  const char *const three[] = { "power-saver", "balanced", "performance", NULL };
  const char *const two[] = { "power-saver", "balanced", NULL };
  struct { const char *reason; const char *text; } cases[] = {
    { "high-operating-temperature", "Performance is degraded: the computer is hot." },
    { "lap-detected", "Performance is degraded: the computer is on a lap." },
    { "something-new", "Performance is degraded: something-new." },
    { "", NULL },
  };

  for (guint i = 0; i < G_N_ELEMENTS (cases); i++)
    {
      PwProfiles *profiles = make (three, "performance", cases[i].reason, NULL);
      g_autofree char *text = pw_profiles_degraded_text (profiles);

      g_assert_cmpstr (text, ==, cases[i].text);
      pw_profiles_free (profiles);
    }
  /* With no Performance there's nothing to be degraded. */
  {
    PwProfiles *profiles = make (two, "balanced", "lap-detected", NULL);

    g_assert_null (pw_profiles_degraded_text (profiles));
    pw_profiles_free (profiles);
  }
}

static char *
lookup (const char *application_id, gpointer data G_GNUC_UNUSED)
{
  return g_str_equal (application_id, "org.gnome.GameMode") ? g_strdup ("Feral GameMode") : NULL;
}

static void
test_holds (void)
{
  const char *const ids[] = { "power-saver", "balanced", "performance", NULL };
  g_autoptr (GVariant) game = holds ("org.gnome.GameMode", "performance");
  g_autoptr (GVariant) other = holds ("foo.desktop", "power-saver");
  g_autoptr (GVariant) odd = holds ("", "turbo");
  PwProfiles *profiles = make (ids, "performance", "", game);
  g_autofree char *text = NULL;

  /* Named from the ID's last part where nothing names it better. */
  text = pw_profiles_holds_text (profiles, NULL, NULL);
  g_assert_cmpstr (text, ==, "GameMode is holding Performance.");
  g_free (text);
  text = pw_profiles_holds_text (profiles, lookup, NULL);
  g_assert_cmpstr (text, ==, "Feral GameMode is holding Performance.");
  pw_profiles_free (profiles);

  profiles = make (ids, "power-saver", "", other);
  g_free (text);
  text = pw_profiles_holds_text (profiles, lookup, NULL);
  g_assert_cmpstr (text, ==, "foo is holding Power Saver.");
  pw_profiles_free (profiles);

  profiles = make (ids, "balanced", "", odd);
  g_free (text);
  text = pw_profiles_holds_text (profiles, NULL, NULL);
  g_assert_cmpstr (text, ==, "An application is holding a profile.");
  pw_profiles_free (profiles);
}

static void
test_holds_in_a_wrapper (void)
{
  /* python-dbusmock's template has the holds as (aa{sv}), a tuple in an array; the real
     daemon's are aa{sv}. Both are read. */
  const char *const ids[] = { "power-saver", "balanced", "performance", NULL };
  g_autoptr (GVariant) inner = holds ("org.gnome.GameMode", "performance");
  g_autoptr (GVariant) tuple = g_variant_ref_sink (g_variant_new_tuple (&inner, 1));
  g_autoptr (GVariant) array = g_variant_ref_sink (g_variant_new_array (NULL, &tuple, 1));
  PwProfiles *profiles = make (ids, "performance", "", array);
  g_autofree char *text = pw_profiles_holds_text (profiles, NULL, NULL);

  g_assert_cmpstr (text, ==, "GameMode is holding Performance.");
  pw_profiles_free (profiles);
}

static void
test_names (void)
{
  g_assert_cmpstr (pw_profile_id (PW_PROFILE_POWER_SAVER), ==, "power-saver");
  g_assert_cmpstr (pw_profile_label (PW_PROFILE_PERFORMANCE), ==, "Performance");
  g_assert_cmpint (pw_profile_from_id ("balanced"), ==, PW_PROFILE_BALANCED);
  g_assert_cmpint (pw_profile_from_id ("turbo"), ==, PW_PROFILE_COUNT);
  g_assert_cmpint (pw_profile_from_id (NULL), ==, PW_PROFILE_COUNT);
  for (int i = 0; i < PW_PROFILE_COUNT; i++)
    {
      const char *const *names = pw_profile_icon_names (i);

      /* Adwaita's name, then the battery profile's and a glyph every theme has. */
      g_assert_nonnull (names[0]);
      g_assert_nonnull (names[2]);
      g_assert_null (names[3]);
    }
}

int
main (int argc, char **argv)
{
  g_test_init (&argc, &argv, NULL);
  g_test_add_func ("/profiles/three-are-listed", test_three);
  g_test_add_func ("/profiles/two-have-no-performance", test_two);
  g_test_add_func ("/profiles/none-listed", test_none);
  g_test_add_func ("/profiles/why-performance-is-degraded", test_degraded);
  g_test_add_func ("/profiles/who-holds-a-profile", test_holds);
  g_test_add_func ("/profiles/holds-in-a-tuple-are-read", test_holds_in_a_wrapper);
  g_test_add_func ("/profiles/names-and-icons", test_names);
  return g_test_run ();
}
