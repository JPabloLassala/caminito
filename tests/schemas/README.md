# Test schemas

The GSettings schemas Camino's tests read (14), vendored so that CI needs no MATE
packages. Tests compile them into a private directory with `glib-compile-schemas` and point
`GSETTINGS_SCHEMA_DIR` at it.

| Schema | Source |
|---|---|
| `org.mate.interface` | mate-desktop 1.28.2 (`fee79b7559965d8ae3b4b8e213717b525c703c1b`), as installed by Arch's `mate-desktop 1.28.2-2` |

These are test data only; nothing here is installed.
