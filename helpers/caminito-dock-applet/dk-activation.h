/*
 * An xdg-activation token for a launch from the dock (CMN-221). GDK 3 makes none on Wayland,
 * so the dock asks the compositor for one itself, on GDK's own connection, and puts it in the
 * launched program's environment: the program maps its first window with it.
 */

#pragma once

#include <gtk/gtk.h>

/* A token for a program about to start, as the desktop entry app_id (its id without
   ".desktop", or NULL) launched from the dock's window, or NULL where the display isn't
   Wayland's or the compositor has no xdg-activation. It asks for no input serial: GDK 3 has
   no accessor for the click's, and Caminito ties a token to input only for a client that has
   the keyboard focus, which a panel with no keyboard interactivity never has. The caller
   frees the token. */
char *dk_activation_token_new (GdkWindow *window, const char *app_id);
