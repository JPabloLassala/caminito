/*
 * What the Power popup shows of power-profiles-daemon (264). See pw-profiles.h.
 */

#include "pw-profiles.h"

static const struct
{
  const char        *id;
  const char        *label;
  /* Breeze and Adwaita name them power-profile-*-symbolic; MATE's own themes have none of
     those, so the battery profile names and a plain glyph follow. */
  const char *const  icons[4];
} profile_info[PW_PROFILE_COUNT] = {
  [PW_PROFILE_POWER_SAVER] = {
    "power-saver", "Power Saver",
    { "power-profile-power-saver-symbolic", "battery-profile-powersave",
      "weather-clear-night-symbolic", NULL },
  },
  [PW_PROFILE_BALANCED] = {
    "balanced", "Balanced",
    { "power-profile-balanced-symbolic", "battery-profile-balanced",
      "emblem-system-symbolic", NULL },
  },
  [PW_PROFILE_PERFORMANCE] = {
    "performance", "Performance",
    { "power-profile-performance-symbolic", "battery-profile-performance",
      "starred-symbolic", NULL },
  },
};

const char *
pw_profile_id (PwProfile profile)
{
  return profile_info[profile].id;
}

const char *
pw_profile_label (PwProfile profile)
{
  return profile_info[profile].label;
}

const char *const *
pw_profile_icon_names (PwProfile profile)
{
  return profile_info[profile].icons;
}

PwProfile
pw_profile_from_id (const char *id)
{
  if (id == NULL)
    return PW_PROFILE_COUNT;
  for (int i = 0; i < PW_PROFILE_COUNT; i++)
    if (g_str_equal (id, profile_info[i].id))
      return i;
  return PW_PROFILE_COUNT;
}

static void
hold_free (gpointer data)
{
  PwHold *hold = data;

  g_free (hold->application_id);
  g_free (hold);
}

/* The daemon's a{sv} for a profile or a hold, however the bus delivered it: a mock can wrap
   what the real daemon sends in a tuple or a variant. Calls found on each dictionary in
   it. */
static void
for_each_dict (GVariant *value, void (*found) (GVariant *dict, gpointer data), gpointer data)
{
  if (value == NULL)
    return;
  if (g_variant_is_of_type (value, G_VARIANT_TYPE_VARIANT))
    {
      g_autoptr (GVariant) inner = g_variant_get_variant (value);

      for_each_dict (inner, found, data);
    }
  else if (g_variant_is_of_type (value, G_VARIANT_TYPE_VARDICT))
    found (value, data);
  else if (g_variant_is_container (value))
    {
      gsize n = g_variant_n_children (value);

      for (gsize i = 0; i < n; i++)
        {
          g_autoptr (GVariant) child = g_variant_get_child_value (value, i);

          for_each_dict (child, found, data);
        }
    }
}

static void
add_listed (GVariant *dict, gpointer data)
{
  PwProfiles *profiles = data;
  g_autofree char *id = NULL;
  PwProfile profile;

  if (!g_variant_lookup (dict, "Profile", "s", &id))
    return;
  profile = pw_profile_from_id (id);
  if (profile != PW_PROFILE_COUNT)
    profiles->listed[profile] = TRUE;
}

static void
add_hold (GVariant *dict, gpointer data)
{
  PwProfiles *profiles = data;
  PwHold *hold = g_new0 (PwHold, 1);
  g_autofree char *profile = NULL;

  g_variant_lookup (dict, "ApplicationId", "s", &hold->application_id);
  g_variant_lookup (dict, "Profile", "s", &profile);
  hold->profile = pw_profile_from_id (profile);
  g_ptr_array_add (profiles->holds, hold);
}

PwProfiles *
pw_profiles_new (GVariant *listed, GVariant *active, GVariant *degraded, GVariant *holds)
{
  PwProfiles *profiles = g_new0 (PwProfiles, 1);

  profiles->holds = g_ptr_array_new_with_free_func (hold_free);
  profiles->active = PW_PROFILE_COUNT;
  profiles->degraded = g_strdup ("");
  for_each_dict (listed, add_listed, profiles);
  if (active != NULL && g_variant_is_of_type (active, G_VARIANT_TYPE_STRING))
    {
      PwProfile profile = pw_profile_from_id (g_variant_get_string (active, NULL));

      /* A profile that isn't listed can't be pressed: there's no tile for it. */
      if (profile != PW_PROFILE_COUNT && profiles->listed[profile])
        profiles->active = profile;
    }
  if (degraded != NULL && g_variant_is_of_type (degraded, G_VARIANT_TYPE_STRING))
    {
      g_free (profiles->degraded);
      profiles->degraded = g_variant_dup_string (degraded, NULL);
    }
  for_each_dict (holds, add_hold, profiles);
  return profiles;
}

void
pw_profiles_free (PwProfiles *profiles)
{
  if (profiles == NULL)
    return;
  g_free (profiles->degraded);
  g_ptr_array_unref (profiles->holds);
  g_free (profiles);
}

gboolean
pw_profiles_any (const PwProfiles *profiles)
{
  for (int i = 0; i < PW_PROFILE_COUNT; i++)
    if (profiles->listed[i])
      return TRUE;
  return FALSE;
}

char *
pw_profiles_degraded_text (const PwProfiles *profiles)
{
  const char *reason = profiles->degraded;

  if (reason[0] == '\0' || !profiles->listed[PW_PROFILE_PERFORMANCE])
    return NULL;
  if (g_str_equal (reason, "high-operating-temperature"))
    return g_strdup ("Performance is degraded: the computer is hot.");
  if (g_str_equal (reason, "lap-detected"))
    return g_strdup ("Performance is degraded: the computer is on a lap.");
  return g_strdup_printf ("Performance is degraded: %s.", reason);
}

/* The last part of "org.gnome.GameMode", which is how an application is known where it has
   no desktop entry. */
static char *
name_from_id (const char *id)
{
  g_autofree char *trimmed = NULL;
  const char *dot;

  if (id == NULL || id[0] == '\0')
    return g_strdup ("An application");
  trimmed = g_str_has_suffix (id, ".desktop") ? g_strndup (id, strlen (id) - strlen (".desktop")) : g_strdup (id);
  dot = strrchr (trimmed, '.');
  if (dot != NULL && dot[1] != '\0')
    return g_strdup (dot + 1);
  return g_steal_pointer (&trimmed);
}

char *
pw_profiles_holds_text (const PwProfiles *profiles, PwAppName name, gpointer data)
{
  GString *text;

  if (profiles->holds->len == 0)
    return NULL;
  text = g_string_new (NULL);
  for (guint i = 0; i < profiles->holds->len; i++)
    {
      const PwHold *hold = g_ptr_array_index (profiles->holds, i);
      g_autofree char *who = NULL;

      if (name != NULL && hold->application_id != NULL)
        who = name (hold->application_id, data);
      if (who == NULL)
        who = name_from_id (hold->application_id);
      if (i > 0)
        g_string_append_c (text, '\n');
      if (hold->profile == PW_PROFILE_COUNT)
        g_string_append_printf (text, "%s is holding a profile.", who);
      else
        g_string_append_printf (text, "%s is holding %s.", who, pw_profile_label (hold->profile));
    }
  return g_string_free (text, FALSE);
}
