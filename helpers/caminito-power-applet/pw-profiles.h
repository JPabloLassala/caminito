/*
 * What the Power popup shows of power-profiles-daemon (264): the profiles it lists, the
 * active one, why Performance is degraded and who holds a profile. Nothing here talks to
 * D-Bus, so that the tests can build any state by hand or from the daemon's variants.
 */

#pragma once

#include <glib.h>

/* The three profiles the daemon can have, in the order of the tiles. It lists two or three
   (power-saver and balanced always, performance when a driver provides it), and any other
   name is left out. */
typedef enum
{
  PW_PROFILE_POWER_SAVER,
  PW_PROFILE_BALANCED,
  PW_PROFILE_PERFORMANCE,
  PW_PROFILE_COUNT,
} PwProfile;

typedef struct
{
  /* The application's ID as the daemon has it ("org.gnome.GameMode"). */
  char    *application_id;
  /* The profile it holds, or PW_PROFILE_COUNT for one that isn't one of the three. */
  PwProfile profile;
} PwHold;

typedef struct
{
  gboolean  listed[PW_PROFILE_COUNT];
  /* The active profile, or PW_PROFILE_COUNT when it isn't one that's listed. */
  PwProfile active;
  /* PerformanceDegraded: "" when it isn't. */
  char     *degraded;
  /* PwHold, in the daemon's order. */
  GPtrArray *holds;
} PwProfiles;

/* From the daemon's properties Profiles (aa{sv}), ActiveProfile (s), PerformanceDegraded (s)
   and ActiveProfileHolds (aa{sv}), each NULL or of another type where the daemon doesn't
   have it. Free it with pw_profiles_free. */
PwProfiles *pw_profiles_new  (GVariant *profiles,
                              GVariant *active,
                              GVariant *degraded,
                              GVariant *holds);
void        pw_profiles_free (PwProfiles *profiles);
/* Whether any profile is listed: with none there are no tiles. */
gboolean    pw_profiles_any  (const PwProfiles *profiles);

/* The daemon's name for the profile, "power-saver". */
const char        *pw_profile_id          (PwProfile profile);
/* The tile's name: "Power Saver". */
const char        *pw_profile_label       (PwProfile profile);
/* The tile's icon: themed names, MATE's and Adwaita's first. NULL-terminated, in static
   storage. */
const char *const *pw_profile_icon_names  (PwProfile profile);
/* The profile for the daemon's name, or PW_PROFILE_COUNT. */
PwProfile          pw_profile_from_id     (const char *id);

/* Why Performance is degraded, "Performance is degraded: the computer is hot."; NULL when it
   isn't or when Performance isn't listed. */
char *pw_profiles_degraded_text (const PwProfiles *profiles);

/* A holder's name for its application ID, or NULL when there's none better than the ID. */
typedef char *(*PwAppName) (const char *application_id, gpointer data);

/* Who holds a profile, one sentence for each holder, "GameMode is holding Performance.",
   joined by newlines; NULL when none does. The name comes from name (the application's
   desktop entry), or from the ID's last part where it's NULL or has none. */
char *pw_profiles_holds_text (const PwProfiles *profiles, PwAppName name, gpointer data);
