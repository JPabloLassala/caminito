# One refresh signals `org.mate.Caminito`'s properties, and the two rules of it have one place each

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## Why

`org.mate.Caminito` has five properties that Caminito signals itself when they change without a caller writing them: `PrimaryOutput`, `ShowingDesktop`, `GlobalMenu`, `GlobalMenuOffered` and `NightLight`. Each has the same three parts:

- a field on `Caminito` for what the callers last heard (`primary_output_signalled`, `showing_desktop_signalled`, `global_menu_signalled`, `global_menu_offered_signalled`, `night_light_signalled`, in `state.rs`);
- a `refresh_*_property` function in `caminito/src/dbus/caminito.rs`, with the same body as the other four;
- a line in `properties_read_from_now_on`.

Three of the functions are called from `refresh_views` (`state.rs`) and two from `refresh_global_menu` (`dbus/global_menu.rs`).

Two rules are in comments only, and each new property has to follow both:

- **A setter records what it wrote.** zbus signals a property after its own setter, so a setter that doesn't write the field has its property signalled twice. The two setters do it in two ways: `ChoosePrimaryOutput` reads the value again, and `SetShowingDesktop` records the value the caller wrote, not the one Policy has after.
- **Nothing is signalled before the interface is served.** Each function checks `self.dbus.is_none()`, and `properties_read_from_now_on` fills the five fields at the end of `start_dbus`.

The gain here is locality, not fewer edits: zbus needs a typed method for each property, and the event loop a `Request` and an arm to answer it, and those stay. This was decided in an architecture review on 2026-10-11.

## What to build

All in `caminito/src/dbus/caminito.rs`, but for the fields and calls that go.

- **One value type.** An `enum Value` with a variant for each property, and `Property::read(self, &Caminito) -> Value`, which reads the property as it is now. `Property::name()` goes, with `Debug` in its place for the warning.
- **What the callers last heard is in `Dbus`,** a `Value` for each `Property`, filled when `Dbus` is made. It can't exist before the interface is served, so the five `dbus.is_none()` checks and `properties_read_from_now_on` go, and so do the five fields on `Caminito`.
- **One refresh.** One function reads each property, compares it with what the callers last heard, signals the ones that differ and records them. It's called once from `refresh_views`, right after `refresh_global_menu`, whose two values are right only then. The signals keep today's order: `PrimaryOutput`, `ShowingDesktop`, `NightLight`, `GlobalMenuOffered`, `GlobalMenu`. `refresh_global_menu` keeps the rest of its work (the apps, the appmenu global, the registrar) and loses its two property calls. The five `refresh_*_property` functions go.
- **One function for the setter rule,** `property_written(Property)`: it reads the property as it is now and records it. Its doc comment has the zbus rule. Both setter arms of `caminito_request` call it. `SetShowingDesktop` so records what Policy has after the write.

Left alone:

- The interface's methods, the `Request` variants and their arms, and the match in `signal`.
- The global menu's offer: the `Switch`, `global_menu_offered`, the `Registrar` and the grace period.

## Acceptance criteria

- [ ] Two headless tests on the fixture's bus, written before the code: a write of `PrimaryOutput` over D-Bus is signalled once, and so is a write of `ShowingDesktop`. If a test that proves it is there already, the Done section names it in place of a new one.
- [ ] `grep -rn "_signalled:" caminito/src/state.rs` finds nothing, and `grep -rn "refresh_.*_property\|properties_read_from_now_on" caminito/src` finds nothing.
- [ ] `refresh_views` has one call for the properties, after `refresh_global_menu`, with a comment that says why it's after.
- [ ] `tests::primary_output`, `tests::show_desktop`, `tests::global_menu`, `tests::menu_claim`, `tests::app_menu_registrar` and `tests::night_light` pass with no test changed but the new ones.
- [ ] `tools/check` passes, or the Done section lists what was run and what was not.

## For the implementing agent to settle

Recorded in the Done section:

- **How `Dbus` holds a `Value` for each `Property`:** an array by the enum's order, a map, or other. A property with no entry mustn't be possible.
- **`Value` beside the types the getters answer with.** `GlobalMenu` and `night_light::Published` are compared as they are today, before they're made into what goes on the bus; say if that stays.
- **The first refresh after `start_dbus`.** Today nothing is signalled until a property changes after the interface is served. Say how that holds with the values filled as `Dbus` is made.
