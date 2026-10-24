# 50. Caminito tells the Dock applet which item to activate

Date: 2026-10-24
Status: Accepted

## Context

Super+1 to Super+9 activate the first nine Dock items (CMN-226), as in Plasma and Ubuntu Dock. The modifier is the `org.mate.Caminito` key `dock-activate-modifier`, Super by default. On Wayland a panel applet can't grab keys, so Caminito binds them, and something must tell the Dock applet which item the user asked for. The applet is in mate-panel's process (ADR 0035), so every Dock panel of a session is in one Wayland client, and a user can have a Dock panel on each output. Only the applet knows its items, their order and what a click on one does.

## Decision

- **A small private protocol, `caminito-dock-activation-v1`, tells the applet.** Its manager has one request, `get_dock_activation`, which takes the `wl_surface` of the applet's panel and makes an object. The object has one event, `activate(index)`, with the index counted from 0 among the Dock items in the order the applet shows them. Like the other private globals, it is hidden from sandboxed clients.
- **The applet names its panel's surface** because one client can have several Dock panels, and Caminito must know which object is on which output. Caminito reads the surface's output when it chooses, so a panel that moves to another output goes with it.
- **Caminito tells one applet**: the applet on the output the pointer is on, else the applet on the focused window's output, else the oldest one.
- **The keys are bound only while an applet has an object.** Without the Dock applet, Super+1 to Super+9 go to the focused window.
- **What the activation does is the applet's**: the same as a click, and a focused item with several windows cycles them.

## Consequences

- The applet needs no D-Bus name and no setting, and Caminito knows at once when the last applet goes.
- A later feature from Caminito to the applet, such as number badges while the modifier is held, can be a new event on the same object.
- Rejected: a D-Bus signal (Caminito can't tell which applet is on which output, nor whether any is running, without a name for each panel); a gsettings key the applet watches (a key is a setting, not an event, and one key can't choose one of two panels); the output of the client's layer surfaces with no surface named (wrong when one mate-panel has Dock panels on two outputs).
