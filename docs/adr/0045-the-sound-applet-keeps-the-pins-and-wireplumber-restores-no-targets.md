# 45. The Sound applet keeps the pins, and WirePlumber restores no targets

Date: 2026-10-23
Status: Accepted

## Context

The Sound applet's popup can move a Sound application to another device in two ways: choosing a device moves it while it runs, and pinning it sends it there every time it plays. WirePlumber 0.5 already remembers moves. Its `node.stream.restore-target` setting, on by default, saves the device of every moved stream in `~/.local/state/wireplumber/stream-properties`, keyed by `application.id`, `application.name`, `media.name` or `node.name` (a notification sound by its role), and sends the application back there when it next plays. With that on, every move would be a pin, and the popup couldn't show which applications are pinned.

CMN-209 tried this on WirePlumber 0.5.17 and PipeWire 1.6.8, against a private PipeWire, WirePlumber and pipewire-pulse (`.scratch/checks/CMN-209/`):

- `wpctl settings --save node.stream.restore-target false` changes the running value and writes `node.stream.restore-target=false` to `~/.local/state/wireplumber/sm-settings` about a second later. The value survives a WirePlumber restart and a new login. `wpctl settings --delete node.stream.restore-target` undoes it at the next WirePlumber start; `--reset` doesn't, since it changes only the running value. `wpctl` writes the `persistent-sm-settings` metadata, and any PipeWire client can write it the same way. A write to `sm-settings` lasts only until WirePlumber restarts.
- A user drop-in, `~/.config/wireplumber/wireplumber.conf.d/*.conf` with `wireplumber.settings = { node.stream.restore-target = false }`, works from the next WirePlumber start, and a saved value overrides it.
- With the restore off, volume and mute are still restored per application. A move through libpulse (`pa_context_move_sink_input_by_index`, through pipewire-pulse) still works and is forgotten when the application closes. WirePlumber still writes each move's target to its state file, though it no longer applies it. Turning the restore back on brings all of those targets back, the old ones included.
- A libpulse client watching sink-input events gets the `new` event before WirePlumber links the stream: 3.8 ms before, in the median, and never less than 3.4 ms. A move made from that event lands before the link, so the stream goes straight to the pinned device and never plays on the default. That held for 20 runs out of 20, for a libpulse player and for a native PipeWire one. A client 5 ms late loses that race every time, and the stream then plays on the default for about as long as the client was late: 7.8, 19.5 and 99.4 ms for a client 5, 20 and 100 ms late.
- A `stream.rules` drop-in for pipewire-pulse that sets `target.object` for one application does place its stream before it starts, but only once pipewire-pulse restarts. WirePlumber's own `stream.rules` feeds only its stream-state script, and can't set a target.

## Decision

- **The applet keeps the pins** in a GSettings key of its own schema, from the application's identity to the device's name, and lists the pinned applications from there.
- **It applies them itself.** When a stream of a pinned application appears, it moves the stream to the pinned device from libpulse's sink-input (or source-output) `new` event, before anything else it does for that event.
- **It turns WirePlumber's target restore off for the user:** whenever it connects to a sound server that is PipeWire, it runs `wpctl settings --save node.stream.restore-target false`. That's WirePlumber's supported way to change a setting for good. The command is idempotent, so running it on every connect also covers a user who turned the setting back on. The applet doesn't run it on PulseAudio, where there's no WirePlumber. When `wpctl` is missing or fails, the applet logs it and works on, and a move without a pin is then remembered as WirePlumber remembers it by default.

## Consequences

- A move without a pin is forgotten when the application closes, in Caminito and in any other desktop the same user logs in to. Outside the applet, a move made with pavucontrol, GNOME's Sound settings or plasma-pa now lasts only until the application closes. Volume, mute and the default devices are still remembered.
- The setting stays off after the applet is removed from the panel. `wpctl settings --delete node.stream.restore-target` turns it back on at the next login, and with it every target WirePlumber stored meanwhile.
- A pinned application's stream doesn't play on the default first, unless the applet is busy for more than about 3 ms when the stream appears. The applet's handling of that event must stay short.
- The applet needs `wpctl`, which ships with WirePlumber. Writing the `persistent-sm-settings` metadata through libpipewire would avoid that, at the cost of a second sound library in the applet.
- On PulseAudio, `module-stream-restore` still remembers moves; the applet leaves that alone.
- Rejected:
  - WirePlumber remembering every move, without pins of the applet's own. Every move would become a pin, the popup couldn't list or unpin them, and an application that isn't running couldn't be shown.
  - A system-wide drop-in in the `caminito` package, under `/usr/share/wireplumber/wireplumber.conf.d/`. It would turn the restore off for every user and every desktop on the machine, whether or not they use the applet, and the applet doesn't depend on Caminito.
  - A user drop-in written by the applet. It takes effect only from the next WirePlumber start, and a value saved with `wpctl` overrides it.
  - Placing the stream before it starts, with a pipewire-pulse `stream.rules` drop-in per pinned application, or a WirePlumber script of Caminito's own. Every pin and unpin would restart pipewire-pulse, which cuts every libpulse client's sound, and the rules reach no native PipeWire client; a script would tie the applet to Caminito. The move from the `new` event already gets there first.
