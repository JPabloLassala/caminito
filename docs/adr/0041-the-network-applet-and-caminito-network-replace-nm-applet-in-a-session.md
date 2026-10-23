# 41. The Network applet and caminito-network replace nm-applet in a Session

Date: 2026-10-23
Status: Accepted

## Context

MATE has no network front end of its own: its network icon is nm-applet's tray icon, autostarted from `/etc/xdg/autostart`. The developer wants Plasma's Networks widget in its place: a panel icon whose popup lists the connections, with a Wi-Fi switch, expanding rows to connect and disconnect, a password field inline for a new network, and each connection's details.

nm-applet is two things at once. It's the icon, and it's NetworkManager's secret agent: the program NetworkManager asks for a Wi-Fi password, an 802.1X passphrase or a VPN's one-time code when a connection needs one, and the keeper of the secrets a user saved as "for this user only". Replacing the icon without the agent leaves two network front ends in one session. Replacing the agent means taking over the secrets nm-applet saved.

## Decision

- **The Network applet follows ADR 0035:** a thin C and GTK3 in-process mate-panel applet under `helpers/`, on `caminito-applet-common`. Its settings window is nm-connection-editor, which it also opens for what the popup doesn't do: an Enterprise (802.1X) network and a hidden one.
- **Unlike the other applets, it uses libnm, not raw GDBus.** NetworkManager's API is much larger than BlueZ's or UPower's (settings dicts, devices, access points, active connections), and libnm is NetworkManager's own client library, always installed with it. A new network joined inline is libnm's add-and-activate, for all users, with the secret kept by NetworkManager.
- **`caminito-network`, in Rust, is the secret agent,** in one process through the session, over gio's D-Bus as `caminito-bluetooth` is. It asks for secrets in GTK4 dialogs, runs a VPN plugin's `auth-dialog` for VPN secrets, sends the connection notifications nm-applet did, and keeps agent-owned secrets in the Secret Service under nm-applet's attributes (`connection-uuid`, `setting-name`, `setting-key`), so the secrets nm-applet saved carry over. mate-session restarts it if it crashes (`X-MATE-AutoRestart`).
- **Only in a Session.** `nm-applet.desktop` joins the skipped list of mate-session's carried patch (ADR 0014), and `caminito-network` exits when it isn't under Caminito. Under X11 MATE the applet still works, with nm-applet as the agent beside it.

## Consequences

- The applet is the first of Caminito's applets to link a client library, and the agent never runs in the panel's process, as ADR 0035 requires.
- Two programs never register as secret agents in one session: in a Session it's `caminito-network`, elsewhere nm-applet.
- A new user of X11 MATE on the same machine gets the applet from the layout and nm-applet's tray icon too, two network icons, until they remove one.
- Secrets that nm-applet saved under other attributes, or in another keyring, aren't found, and their connections ask again once.
- Mobile broadband is left out, its PIN and PUK prompts with it, along with the hotspot, a QR code of a Wi-Fi password and the speed graph.
- Rejected:
  - Keeping nm-applet as the agent with its icon hidden. That's two network front ends in one session, and nm-applet has no supported way to run without its icon.
  - The agent in the applet. A pairing prompt or a crash would take mate-panel with it, which ADR 0035 rules out.
  - A settings window of Caminito's own. nm-connection-editor already covers 802.1X, the VPN plugins and the IP settings; one of Caminito's own can come later.
  - Replacing nm-applet under X11 MATE too. It would need autostart overrides, which ADR 0014 rejected, or a patched network-manager-applet.
