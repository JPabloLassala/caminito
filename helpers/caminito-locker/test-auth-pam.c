/*
 * The PAM conversation of gs-auth-pam.c (131) against a stand-in mate-screensaver service.
 *
 * Meson runs it under pam_wrapper, whose PAM_WRAPPER_SERVICE_DIR has a mate-screensaver
 * service on pam_matrix and pam_chatty, and passes the path of pam_matrix's password file
 * as the one argument. The test writes that file with the running user's password, so the
 * service is the only thing that differs from a real session: the code under test is the
 * locker's own, its service name and its user's lookup included.
 */

#include <locale.h>
#include <stdlib.h>

#include <glib.h>
#include <glib/gstdio.h>

#include "gs-auth.h"

#define PASSWORD "the right one"

static const char *passdb;

typedef struct
{
  /* What the dialog types at the password prompt, or NULL to cancel it. */
  const char *typed;
  /* Each message the conversation gave the dialog, as "STYLE: message". */
  GPtrArray  *messages;
} Dialog;

static const char *
style_name (GSAuthMessageStyle style)
{
  switch (style)
    {
    case GS_AUTH_MESSAGE_PROMPT_ECHO_ON:
      return "echo on";
    case GS_AUTH_MESSAGE_PROMPT_ECHO_OFF:
      return "echo off";
    case GS_AUTH_MESSAGE_ERROR_MSG:
      return "error";
    case GS_AUTH_MESSAGE_TEXT_INFO:
      return "info";
    default:
      g_assert_not_reached ();
    }
}

/* What mate-screensaver-dialog.c's auth_message_handler returns: a response to each
   prompt, and nothing to a message. */
static gboolean
answer (GSAuthMessageStyle style,
        const char        *msg,
        char             **response,
        gpointer           data)
{
  Dialog *dialog = data;

  g_ptr_array_add (dialog->messages, g_strdup_printf ("%s: %s", style_name (style), msg));
  *response = NULL;
  if (style == GS_AUTH_MESSAGE_PROMPT_ECHO_OFF || style == GS_AUTH_MESSAGE_PROMPT_ECHO_ON)
    {
      *response = g_strdup (dialog->typed);
      return *response != NULL;
    }
  return FALSE;
}

/* Runs the conversation as the dialog would, with `typed` at the prompt. */
static gboolean
verify (const char *typed,
        GPtrArray **messages,
        GError    **error)
{
  Dialog dialog = { typed, g_ptr_array_new_with_free_func (g_free) };
  gboolean res;

  res = gs_auth_verify_user (g_get_user_name (), ":0", answer, &dialog, error);
  *messages = dialog.messages;
  return res;
}

static gboolean
has_message (GPtrArray  *messages,
             const char *message)
{
  for (guint i = 0; i < messages->len; i++)
    if (g_str_equal (g_ptr_array_index (messages, i), message))
      return TRUE;
  return FALSE;
}

static void
the_right_password_unlocks (void)
{
  g_autoptr (GPtrArray) messages = NULL;
  g_autoptr (GError) error = NULL;

  g_assert_true (verify (PASSWORD, &messages, &error));
  g_assert_no_error (error);
  g_assert_true (has_message (messages, "echo off: Password: "));
}

static void
a_wrong_password_is_refused_as_incorrect (void)
{
  g_autoptr (GPtrArray) messages = NULL;
  g_autoptr (GError) error = NULL;

  g_assert_false (verify ("a wrong one", &messages, &error));
  g_assert_error (error, GS_AUTH_ERROR, GS_AUTH_ERROR_AUTH_ERROR);
  g_assert_cmpstr (error->message, ==, "Incorrect password.");
}

static void
the_password_is_asked_again_after_a_wrong_one (void)
{
  g_autoptr (GPtrArray) wrong = NULL;
  g_autoptr (GPtrArray) right = NULL;
  g_autoptr (GError) error = NULL;

  g_assert_false (verify ("a wrong one", &wrong, NULL));
  g_assert_true (verify (PASSWORD, &right, &error));
  g_assert_no_error (error);
}

static void
a_cancelled_prompt_ends_the_conversation_unauthenticated (void)
{
  g_autoptr (GPtrArray) messages = NULL;

  g_assert_false (verify (NULL, &messages, NULL));
  /* Nothing is asked once the prompt is cancelled. */
  g_assert_cmpstr (g_ptr_array_index (messages, messages->len - 1), ==, "echo off: Password: ");
}

static void
the_services_messages_reach_the_dialog (void)
{
  g_autoptr (GPtrArray) messages = NULL;

  g_assert_true (verify (PASSWORD, &messages, NULL));
  g_assert_true (has_message (messages, "info: Authentication succeeded"));
}

int
main (int    argc,
      char **argv)
{
  g_autofree char *contents = NULL;
  int status;

  setlocale (LC_ALL, "C");
  g_test_init (&argc, &argv, NULL);
  g_assert_cmpint (argc, ==, 2);
  passdb = argv[1];

  /* pam_matrix's format: user, password and service. */
  contents = g_strdup_printf ("%s:%s:mate-screensaver\n", g_get_user_name (), PASSWORD);
  g_assert_true (g_file_set_contents (passdb, contents, -1, NULL));

  g_test_add_func ("/auth/the-right-password-unlocks", the_right_password_unlocks);
  g_test_add_func ("/auth/a-wrong-password-is-refused-as-incorrect",
                   a_wrong_password_is_refused_as_incorrect);
  g_test_add_func ("/auth/the-password-is-asked-again-after-a-wrong-one",
                   the_password_is_asked_again_after_a_wrong_one);
  g_test_add_func ("/auth/a-cancelled-prompt-ends-the-conversation-unauthenticated",
                   a_cancelled_prompt_ends_the_conversation_unauthenticated);
  g_test_add_func ("/auth/the-services-messages-reach-the-dialog",
                   the_services_messages_reach_the_dialog);

  status = g_test_run ();
  g_unlink (passdb);
  return status;
}
