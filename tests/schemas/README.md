# Test schemas

The GSettings schemas Caminito's tests read (14), vendored so that CI needs no MATE
packages. Tests compile them into a private directory with `glib-compile-schemas`. The
palette helper's and caminito-locker's tests point `GSETTINGS_SCHEMA_DIR` at it; caminito's
headless tests read it as their only schema source, each on a memory backend of its own (the settings thread's
`Source::Test`).

| Schema | Source |
|---|---|
| `org.mate.interface` | mate-desktop 1.28.2 (`fee79b7559965d8ae3b4b8e213717b525c703c1b`), as installed by Arch's `mate-desktop 1.28.2-2` |
| `org.mate.Marco.*` (`org.mate.marco.gschema.xml`) | marco 1.29.0 (`42a17368923adf3e17cc606ad464c0b2fd19d56f`), as installed by Arch's `marco 1.29.0-1` |
| `org.mate.font-rendering` | mate-settings-daemon 1.28.0 (`e35462eeefb6d5eabebf76317640ee4fa62ecbb0`), as installed by Arch's `mate-settings-daemon 1.28.0-2` |
| `org.mate.peripherals-mouse` | mate-settings-daemon 1.28.0 (`e35462eeefb6d5eabebf76317640ee4fa62ecbb0`), as installed by Arch's `mate-settings-daemon 1.28.0-2` |
| `org.mate.peripherals-touchpad`, `org.mate.peripherals-keyboard` | mate-settings-daemon 1.28.0 (`e35462eeefb6d5eabebf76317640ee4fa62ecbb0`), as installed by Arch's `mate-settings-daemon 1.28.0-2` |
| `org.mate.peripherals-keyboard-xkb` | libmatekbd 1.28.0, as installed by Arch's `libmatekbd 1.28.0-2` |
| `org.mate.accessibility-keyboard` | mate-desktop 1.28.2 (`fee79b7559965d8ae3b4b8e213717b525c703c1b`), as installed by Arch's `mate-desktop 1.28.2-3` |
| `org.mate.lockdown` | mate-desktop 1.28.2 (`fee79b7559965d8ae3b4b8e213717b525c703c1b`) |
| `org.mate.sound` | mate-desktop 1.28.2 (`fee79b7559965d8ae3b4b8e213717b525c703c1b`), as installed by Arch's `mate-desktop 1.28.2-2` |
| `org.mate.session` | mate-session-manager 1.28.0 (`fa1bf6c2fb45ff0263465537a0730ebff2b2010d`), with `@GETTEXT_PACKAGE@` as `mate-session-manager` and `@DEFAULT_WM@` as `marco` |
| `org.mate.screensaver` | mate-screensaver `60b2db3`, the source caminito-locker is ported from (131), with `@datadir@` as `/usr/share` |
| `org.mate.Caminito` | Caminito's own, a link to [data/org.mate.Caminito.gschema.xml](../../data/org.mate.Caminito.gschema.xml), which Meson installs |

mate-settings-daemon 1.28.0's mouse schema has no `natural-scroll`, which msd's master reads, so
Caminito reads that key only where a schema has it.

An upstream schema change is a deliberate re-vendor. These are test data only; nothing
here is installed, and Caminito's own schema is installed from `data/`.
