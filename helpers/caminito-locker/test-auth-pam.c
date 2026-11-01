/*
 * The PAM conversation of gs-auth-pam.c (131) against a stand-in mate-screensaver service.
 *
 * Meson runs it under pam_wrapper, whose PAM_WRAPPER_SERVICE_DIR has a mate-screensaver
 * service on pam_matrix, pam_chatty and pam_faillock, and passes the path of pam_matrix's
 * password file and the directory of pam_faillock's tally as its two arguments. The test
 * writes that file with the running user's password, so the service is the only thing that
 * differs from a real session: the code under test is the locker's own, its service name
 * and its user's lookup included.
 */

#include <locale.h>
#include <stdlib.h>

#include <glib.h>
#include <glib/gstdio.h>

#include "gs-auth.h"

#define PASSWORD "the right one"

static const char *passdb;
static const char *faillock_dir;

typedef struct
{
  /* What the dialog types at the password prompt, or NULL to give the prompt up. */
  const char *typed;
  /* How long the dialog is cancelled or timed out before it types: it has no answer for
     PAM then, and the conversation waits. */
  guint       wait_msec;
  /* The failures pam_faillock had counted when the wait ended. */
  guint       failures_waiting;
  gboolean    waited;
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

/* The failures pam_faillock has counted for the user: the records of its tally file, whose
   lines starting with "#" are the header. */
static guint
failures (void)
{
  g_autofree char *path = g_build_filename (faillock_dir, g_get_user_name (), NULL);
  g_autofree char *contents = NULL;
  g_auto (GStrv) lines = NULL;
  guint count = 0;

  if (!g_file_get_contents (path, &contents, NULL, NULL))
    return 0;
  lines = g_strsplit (contents, "\n", -1);
  for (guint i = 0; lines[i] != NULL; i++)
    if (lines[i][0] != '\0' && lines[i][0] != '#')
      count++;
  return count;
}

static gboolean
stop_waiting (gpointer data)
{
  g_main_loop_quit (data);
  return G_SOURCE_REMOVE;
}

/* What mate-screensaver-dialog.c's auth_message_handler returns: a response to each
   prompt, and nothing to a message. The dialog's timeout and Cancel leave a prompt open
   (CMN-256): its handler waits, as the dialog does until a key or a click, and answers
   then. */
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
      if (dialog->wait_msec > 0)
        {
          GMainLoop *loop = g_main_loop_new (NULL, FALSE);

          g_timeout_add (dialog->wait_msec, stop_waiting, loop);
          g_main_loop_run (loop);
          g_main_loop_unref (loop);
          dialog->waited = TRUE;
          dialog->failures_waiting = failures ();
        }
      *response = g_strdup (dialog->typed);
      return *response != NULL;
    }
  return FALSE;
}

/* Runs the conversation as the dialog would, with `typed` at the prompt. */
static gboolean
verify_after (const char *typed,
              guint       wait_msec,
              Dialog     *dialog,
              GError    **error)
{
  g_autofree char *tally = g_build_filename (faillock_dir, g_get_user_name (), NULL);

  /* Each conversation starts from no failures. */
  g_unlink (tally);
  *dialog = (Dialog) { typed, wait_msec, 0, FALSE, g_ptr_array_new_with_free_func (g_free) };

  return gs_auth_verify_user (g_get_user_name (), ":0", answer, dialog, error);
}

static gboolean
verify (const char *typed,
        GPtrArray **messages,
        GError    **error)
{
  Dialog dialog;
  gboolean res;

  res = verify_after (typed, 0, &dialog, error);
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

static guint
count_message (GPtrArray  *messages,
               const char *message)
{
  guint count = 0;

  for (guint i = 0; i < messages->len; i++)
    if (g_str_equal (g_ptr_array_index (messages, i), message))
      count++;
  return count;
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
  /* A submitted wrong password is a failure that PAM counts. */
  g_assert_cmpuint (failures (), ==, 1);
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

/* Giving the prompt up, which is what the dialog's cancel was before CMN-256, is a failure
   to pam_faillock: the reason the dialog doesn't. */
static void
giving_a_prompt_up_is_a_failure_to_pam (void)
{
  g_autoptr (GPtrArray) messages = NULL;

  g_assert_false (verify (NULL, &messages, NULL));
  g_assert_cmpuint (failures (), ==, 1);
}

/* The rule: only a password that the user submits ends a PAM conversation. A prompt the
   dialog cancels or lets time out has no pam_authenticate result: the conversation is
   open, nothing is counted, and the password typed at the prompt asked again unlocks. */
static void
only_a_submitted_password_ends_the_conversation (void)
{
  Dialog dialog;
  g_autoptr (GError) error = NULL;

  g_assert_true (verify_after (PASSWORD, 300, &dialog, &error));
  g_assert_no_error (error);
  g_assert_true (dialog.waited);
  g_assert_cmpuint (dialog.failures_waiting, ==, 0);
  g_assert_cmpuint (failures (), ==, 0);
  /* The prompt was asked once: the conversation wasn't restarted for it. */
  g_assert_cmpuint (count_message (dialog.messages, "echo off: Password: "), ==, 1);
  g_ptr_array_unref (dialog.messages);
}

/* And the wrong one typed after the wait is the one failure. */
static void
a_wrong_password_after_the_wait_is_one_failure (void)
{
  Dialog dialog;
  g_autoptr (GError) error = NULL;

  g_assert_false (verify_after ("a wrong one", 300, &dialog, &error));
  g_assert_error (error, GS_AUTH_ERROR, GS_AUTH_ERROR_AUTH_ERROR);
  g_assert_cmpuint (dialog.failures_waiting, ==, 0);
  g_assert_cmpuint (failures (), ==, 1);
  g_ptr_array_unref (dialog.messages);
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
  g_assert_cmpint (argc, ==, 3);
  passdb = argv[1];
  faillock_dir = argv[2];
  g_assert_cmpint (g_mkdir_with_parents (faillock_dir, 0700), ==, 0);

  /* pam_matrix's format: user, password and service. */
  contents = g_strdup_printf ("%s:%s:mate-screensaver\n", g_get_user_name (), PASSWORD);
  g_assert_true (g_file_set_contents (passdb, contents, -1, NULL));

  g_test_add_func ("/auth/the-right-password-unlocks", the_right_password_unlocks);
  g_test_add_func ("/auth/a-wrong-password-is-refused-as-incorrect",
                   a_wrong_password_is_refused_as_incorrect);
  g_test_add_func ("/auth/the-password-is-asked-again-after-a-wrong-one",
                   the_password_is_asked_again_after_a_wrong_one);
  g_test_add_func ("/auth/giving-a-prompt-up-is-a-failure-to-pam",
                   giving_a_prompt_up_is_a_failure_to_pam);
  g_test_add_func ("/auth/only-a-submitted-password-ends-the-conversation",
                   only_a_submitted_password_ends_the_conversation);
  g_test_add_func ("/auth/a-wrong-password-after-the-wait-is-one-failure",
                   a_wrong_password_after_the_wait_is_one_failure);
  g_test_add_func ("/auth/the-services-messages-reach-the-dialog",
                   the_services_messages_reach_the_dialog);

  status = g_test_run ();
  g_unlink (passdb);
  return status;
}
