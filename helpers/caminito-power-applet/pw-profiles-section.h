/*
 * The Power popup's power profile section (264): a tile for each profile the daemon lists,
 * the active one pressed, and a dimmed line under them for why Performance is degraded and
 * who holds a profile. It reads power-profiles-daemon by itself (pw-ppd.c) and is hidden
 * where there is none. The daemon's connection goes when the widget does.
 */

#pragma once

#include <gtk/gtk.h>

GtkWidget *pw_profiles_section_new (void);
