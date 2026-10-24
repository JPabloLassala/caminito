# 50. Caminito tells the Dock applet which item to activate

Date: 2026-10-24
Status: Accepted

## Context

Super+1 to Super+9 activate the first nine Dock items (CMN-226), as in Plasma and Ubuntu Dock. The modifier is the `org.mate.Caminito` key `dock-activate-modifier`, Super by default. On Wayland a panel applet can't grab keys, so Caminito binds them, and something must tell the Dock applet which item the user asked for. The applet is in mate-panel's process (ADR 0035). Thus every Dock panel of a session is in one Wayland client. A user can have a Dock panel on each output. Only the applet knows its items, their order and what a click on one does.

## Decision

- **A small private protocol, `caminito-dock-activation-v1`, tells the applet.** Its manager has one request, `get_dock_activation`, which takes the `wl_surface` of the applet's panel and makes an object. The object has one event, `activate(index)`, with the index counted from 0 among the Dock items in the order the applet shows them. Like the other private globals, it is hidden from sandboxed clients.
- **The applet names its panel's surface.** One client can have several Dock panels, and Caminito must know which object is on which output. Caminito reads the surface's output when it chooses, so a panel that moves to another output goes with it.
- **The request takes a `wl_surface`, not the `zwlr_layer_surface_v1`** that Intellihide takes (ADR 0047). gtk-layer-shell keeps the layer surface to itself, and an applet gets only its panel window's `wl_surface` from GDK. A surface that is no layer surface is on no output.
- **Caminito tells one applet**: the applet on the output the pointer is on, else the applet on the focused window's output, else the oldest applet with a panel on an output, else the oldest applet.
- **The keys are bound only while an applet has an object.** Without the Dock applet, Super+1 to Super+9 go to the focused window.
- **What the activation does is the applet's**: the same as a click, and a focused item with several windows cycles them.

## Consequences

- The applet needs no D-Bus name and no setting, and Caminito can see at any time whether an applet has an object.
- A later feature from Caminito to the applet, such as number badges while the modifier is held, can be a new event on the same object.
- Rejected: a D-Bus signal. Without a name for each panel, Caminito can't tell which applet is on which output, or whether an applet runs.
- Rejected: a gsettings key that the applet watches. A key is a setting, not an event, and one key can't choose one of two panels.
- Rejected: an event on the manager, with the output found from the client's layer surfaces. It is wrong when one mate-panel has Dock panels on two outputs.
