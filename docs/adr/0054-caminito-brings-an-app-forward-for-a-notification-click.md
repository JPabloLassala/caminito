# 54. Caminito brings an app forward for a notification's click

Date: 2026-10-29
Status: Accepted

## Context

A click on a notification, in its bubble or in the Notification center, is to bring its app forward (the notifications spec, "The daemon"). With a default action, the daemon sends `ActivationToken` before `ActionInvoked` and the app activates its own window with the token. Without one, nothing in the Desktop Notifications spec reaches the app: the spec named "the existing Dock path (ADR 0050)", but that protocol only told the Dock applet which item Super+1…9 meant, never asked Caminito to focus anything, and it was removed with the Dock's cut features (CMN-255). The Icon Task Manager activates a window through wlr-foreign-toplevel-management, as a pager does.

The daemon knows a Notifying app by its key: the `desktop-entry` hint, or `app_name` without one. Caminito knows each window's app id and, for icons, finds the desktop entry an app id resolves to.

A token is only honoured when it's tied to user input (165), which asked that its client have the keyboard. A bubble is a layer surface that never takes the keyboard, so a token GDK asked for with the click's serial was refused, and the app's window only became urgent.

## Decision

- **`org.mate.Caminito` gets `ActivateApplication(s app)`.** `app` is a Notifying app's key. Caminito activates the app's most recently focused window, minimised or on another workspace too, as a click on its Task item does: policy's `activate`, which switches the workspace, unminimises, raises and focuses. A window is the app's when its app id is `app` in any case, or when the desktop entry its app id resolves to has the ID `app`. An app with no window that ever had the focus is left as it is.
- **The daemon calls it for a click without a default action,** without auto-start and without waiting, and closes the notification unless it's resident. Outside a Session there's no Caminito, and the click only closes it.
- **A press on a layer surface ties its client's tokens to input.** Caminito keeps the client and the serial of the last press on a layer surface, and a token from that client with a serial no older than the press is tied to input while the client still has the pointer. A hover alone, or a token asked for once the pointer has gone, isn't.

## Consequences

- The daemon and Caminito share one more method, which any client on the session bus can call. It moves the focus, as a pager on wlr-foreign-toplevel-management already can; it reveals nothing.
- A notification clicked from the Notification center brings the app forward the same way, through the private API's `InvokeAction` with `default`, which every entry now takes.
- Rejected: the daemon binding wlr-foreign-toplevel-management itself and matching app ids there. It would duplicate Caminito's desktop-entry resolution in the daemon, and the bubbles' process would need the window list for this alone.
- Rejected: giving the bubbles' layer surface the keyboard on every click, so that the old rule held. A click on a bubble would take the keyboard from the window being typed in. The bubbles take it `on_demand` only while one of them has a reply field.
