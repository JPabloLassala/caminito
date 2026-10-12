/* What patch 0008 adds to the keybindings capplet's eggaccelerators.c (186): a mouse button
 * read from a setting, written back, and shown, and which presses are a System shortcut.
 * run.sh builds it against the patched source. */
#include <string.h>
#include <gtk/gtk.h>
#include "eggaccelerators.h"

static int failures;

static void
expect_string (const char *what, gchar *got, const char *want)
{
  if (g_strcmp0 (got, want) != 0)
    {
      g_printerr ("FAIL %s: got \"%s\", want \"%s\"\n", what, got, want);
      failures++;
    }
  g_free (got);
}

static void
expect_parse (const char *accelerator, gboolean ok, guint button, EggVirtualModifierType mods)
{
  guint got_button = 99;
  EggVirtualModifierType got_mods = 99;
  gboolean got = egg_accelerator_parse_button (accelerator, &got_button, &got_mods);

  if (got != ok || (ok && (got_button != button || got_mods != mods)))
    {
      g_printerr ("FAIL parse \"%s\": got %d, Button%u, mods 0x%x\n",
                  accelerator, got, got_button, got_mods);
      failures++;
    }
}

static void
expect_shortcut (guint button, EggVirtualModifierType mods, gboolean want)
{
  if (egg_button_is_shortcut (button, mods) != want)
    {
      g_printerr ("FAIL Button%u with mods 0x%x is %sa shortcut\n", button, mods, want ? "not " : "");
      failures++;
    }
}

int
main (int argc, char **argv)
{
  const EggVirtualModifierType super = EGG_VIRTUAL_SUPER_MASK;
  const EggVirtualModifierType alt = EGG_VIRTUAL_ALT_MASK;
  const EggVirtualModifierType ctrl = EGG_VIRTUAL_CONTROL_MASK;
  const EggVirtualModifierType shift = EGG_VIRTUAL_SHIFT_MASK;
  guint b;

  g_setenv ("LANGUAGE", "en", TRUE);
  gtk_init (&argc, &argv);

  /* ADR 0028's syntax, as Caminito's parser reads it (184). */
  expect_parse ("<Super>Button8", TRUE, 8, super);
  expect_parse ("<super>button8", TRUE, 8, super);
  expect_parse ("<Mod4>BUTTON8", TRUE, 8, super);
  expect_parse ("<Hyper>Button9", TRUE, 9, super);
  expect_parse ("<Control><Alt>Button9", TRUE, 9, ctrl | alt);
  expect_parse ("<Primary><Shift>Button9", TRUE, 9, ctrl | shift);
  expect_parse ("<Meta>Button2", TRUE, 2, alt);
  expect_parse ("Button2", TRUE, 2, 0);
  expect_parse ("Button8", TRUE, 8, 0);
  expect_parse ("Button20", TRUE, 20, 0);
  expect_parse ("<Alt>Button1", TRUE, 1, alt);
  expect_parse ("<Alt>Button3", TRUE, 3, alt);
  expect_parse ("<Shift>Button1", TRUE, 1, shift);
  expect_parse ("Button1", FALSE, 0, 0);
  expect_parse ("Button3", FALSE, 0, 0);
  for (b = 4; b <= 7; b++)
    {
      gchar *bare = g_strdup_printf ("Button%u", b);
      gchar *held = g_strdup_printf ("<Super>Button%u", b);
      expect_parse (bare, FALSE, 0, 0);
      expect_parse (held, FALSE, 0, 0);
      g_free (bare);
      g_free (held);
    }
  expect_parse ("Button0", FALSE, 0, 0);
  expect_parse ("Button", FALSE, 0, 0);
  expect_parse ("<Super>Button", FALSE, 0, 0);
  expect_parse ("Button21", FALSE, 0, 0);
  expect_parse ("Button99999999999", FALSE, 0, 0);
  expect_parse ("Button-1", FALSE, 0, 0);
  expect_parse ("Button+8", FALSE, 0, 0);
  expect_parse ("Button08", FALSE, 0, 0);
  expect_parse ("Button8a", FALSE, 0, 0);
  expect_parse ("<Release>Button8", FALSE, 0, 0);
  expect_parse ("<Mod5>Button8", FALSE, 0, 0);
  expect_parse ("<Super>e", FALSE, 0, 0);
  expect_parse ("<Super", FALSE, 0, 0);
  expect_parse ("disabled", FALSE, 0, 0);
  expect_parse ("", FALSE, 0, 0);

  /* What the dialog writes: the ADR's value, which the parser reads back. */
  expect_string ("name of Super+8", egg_virtual_button_name (8, super), "<Super>Button8");
  expect_string ("name of 2", egg_virtual_button_name (2, 0), "Button2");
  expect_string ("name of Ctrl+Alt+9", egg_virtual_button_name (9, ctrl | alt), "<Primary><Alt>Button9");
  expect_parse ("<Primary><Alt>Button9", TRUE, 9, ctrl | alt);

  /* A press's state names Super as Mod4 and Super both, and Alt as Mod1. */
  if (egg_button_modifiers (EGG_VIRTUAL_MOD4_MASK | super | EGG_VIRTUAL_HYPER_MASK) != super ||
      egg_button_modifiers ((EggVirtualModifierType) GDK_MOD1_MASK) != alt ||
      egg_button_modifiers (EGG_VIRTUAL_META_MASK) != alt ||
      egg_button_modifiers (ctrl | shift | EGG_VIRTUAL_MOD2_MASK | EGG_VIRTUAL_LOCK_MASK) != (ctrl | shift))
    {
      g_printerr ("FAIL a press's modifiers aren't named as Caminito reads them\n");
      failures++;
    }

  /* The Window menu's labels (184), joined to the modifiers as a key's are. */
  expect_string ("label of 1", egg_virtual_button_label (1, alt), "Alt+Left Button");
  expect_string ("label of 2", egg_virtual_button_label (2, 0), "Middle Button");
  expect_string ("label of 3", egg_virtual_button_label (3, ctrl), "Ctrl+Right Button");
  expect_string ("label of 8", egg_virtual_button_label (8, super), "Super+Back Button");
  expect_string ("label of 9", egg_virtual_button_label (9, 0), "Forward Button");
  expect_string ("label of 10", egg_virtual_button_label (10, 0), "Button 10");
  expect_string ("label of 20", egg_virtual_button_label (20, shift | super), "Shift+Super+Button 20");

  /* Which presses become the shortcut. */
  expect_shortcut (1, 0, FALSE);
  expect_shortcut (3, 0, FALSE);
  expect_shortcut (1, alt, TRUE);
  expect_shortcut (3, super, TRUE);
  expect_shortcut (2, 0, TRUE);
  for (b = 4; b <= 7; b++)
    expect_shortcut (b, super, FALSE);
  expect_shortcut (8, 0, TRUE);
  expect_shortcut (9, ctrl, TRUE);
  expect_shortcut (20, 0, TRUE);
  expect_shortcut (21, 0, FALSE);
  expect_shortcut (0, 0, FALSE);

  /* Spanish, and only Spanish, has the labels in its own words. */
  g_setenv ("LANGUAGE", "es_AR:en", TRUE);
  expect_string ("Spanish label of 2", egg_virtual_button_label (2, 0), "Botón central");
  expect_string ("Spanish label of 8", egg_virtual_button_label (8, 0), "Botón atrás");
  expect_string ("Spanish label of 9", egg_virtual_button_label (9, 0), "Botón adelante");
  expect_string ("Spanish label of 1", egg_virtual_button_label (1, alt), "Alt+Botón izquierdo");
  expect_string ("Spanish label of 3", egg_virtual_button_label (3, alt), "Alt+Botón derecho");
  expect_string ("Spanish label of 12", egg_virtual_button_label (12, 0), "Botón 12");
  expect_string ("Spanish text", g_strdup (egg_caminito_text ("Yes", "Sí")), "Sí");
  g_setenv ("LANGUAGE", "fr:es", TRUE);
  expect_string ("French label of 8", egg_virtual_button_label (8, 0), "Back Button");
  g_setenv ("LANGUAGE", "est", TRUE);
  expect_string ("Estonian label of 8", egg_virtual_button_label (8, 0), "Back Button");

  if (failures)
    {
      g_printerr ("%d failed\n", failures);
      return 1;
    }
  g_print ("buttons: all passed\n");
  return 0;
}
