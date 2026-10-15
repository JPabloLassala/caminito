//! BlueZ's objects as the object manager last told of them: each path's interfaces and
//! their properties, kept as the variants BlueZ sent. The adapters and devices are read
//! from them on demand, so a property nobody reads yet costs nothing to follow.

use std::collections::BTreeMap;

use gio::glib::Variant;

use super::{Adapter, Device};

pub(super) const ADAPTER: &str = "org.bluez.Adapter1";
pub(super) const DEVICE: &str = "org.bluez.Device1";
pub(super) const BATTERY: &str = "org.bluez.Battery1";

/// An interface's properties, by name.
type Properties = BTreeMap<String, Variant>;

/// Every object BlueZ manages, by path, with its interfaces by name.
#[derive(Debug, Default)]
pub(super) struct Objects {
    paths: BTreeMap<String, BTreeMap<String, Properties>>,
}

impl Objects {
    /// Replaces everything with the answer to `GetManagedObjects`, `(a{oa{sa{sv}}})`.
    pub(super) fn replace_all(&mut self, managed: &Variant) {
        self.paths.clear();
        let objects = managed.child_value(0);
        for entry in objects.iter() {
            self.add(&entry.child_value(0), &entry.child_value(1));
        }
    }

    /// Applies `InterfacesAdded`, `(oa{sa{sv}})`.
    pub(super) fn interfaces_added(&mut self, parameters: &Variant) {
        self.add(&parameters.child_value(0), &parameters.child_value(1));
    }

    /// Applies `InterfacesRemoved`, `(oas)`. A path left with no interface is gone.
    pub(super) fn interfaces_removed(&mut self, parameters: &Variant) {
        let Some(path) = parameters.child_value(0).str().map(str::to_owned) else {
            return;
        };
        let Some(interfaces) = self.paths.get_mut(&path) else {
            return;
        };
        for name in parameters.child_value(1).iter() {
            if let Some(name) = name.str() {
                interfaces.remove(name);
            }
        }
        if interfaces.is_empty() {
            self.paths.remove(&path);
        }
    }

    /// Applies `PropertiesChanged`, `(sa{sv}as)`, from the object at `path`. A change to an
    /// interface the object manager hasn't announced is ignored: BlueZ announces each one
    /// before it changes it.
    pub(super) fn properties_changed(&mut self, path: &str, parameters: &Variant) {
        let Some(interface) = parameters.child_value(0).str().map(str::to_owned) else {
            return;
        };
        let Some(properties) = self
            .paths
            .get_mut(path)
            .and_then(|interfaces| interfaces.get_mut(&interface))
        else {
            return;
        };
        properties.extend(dictionary(&parameters.child_value(1)));
        for name in parameters.child_value(2).iter() {
            if let Some(name) = name.str() {
                properties.remove(name);
            }
        }
    }

    /// Forgets everything, when BlueZ leaves the bus.
    pub(super) fn clear(&mut self) {
        self.paths.clear();
    }

    /// Every adapter, in the order of their paths.
    pub(super) fn adapters(&self) -> Vec<Adapter> {
        self.with(ADAPTER)
            .map(|(path, interfaces)| {
                let adapter = &interfaces[ADAPTER];
                Adapter {
                    path: path.to_owned(),
                    address: string(adapter, "Address").unwrap_or_default(),
                    alias: string(adapter, "Alias")
                        .or_else(|| string(adapter, "Name"))
                        .unwrap_or_default(),
                    powered: boolean(adapter, "Powered"),
                    discoverable: boolean(adapter, "Discoverable"),
                    discovering: boolean(adapter, "Discovering"),
                }
            })
            .collect()
    }

    /// Every device of every adapter, in the order of their paths.
    pub(super) fn devices(&self) -> Vec<Device> {
        self.with(DEVICE)
            .map(|(path, interfaces)| {
                let device = &interfaces[DEVICE];
                let address = string(device, "Address").unwrap_or_default();
                Device {
                    path: path.to_owned(),
                    adapter: string(device, "Adapter").unwrap_or_default(),
                    name: string(device, "Name"),
                    // BlueZ always has an alias, the address at worst, but say the same if
                    // it ever hasn't.
                    alias: string(device, "Alias").unwrap_or_else(|| address.clone()),
                    address,
                    icon: string(device, "Icon"),
                    paired: boolean(device, "Paired"),
                    trusted: boolean(device, "Trusted"),
                    connected: boolean(device, "Connected"),
                    battery: interfaces
                        .get(BATTERY)
                        .and_then(|battery| battery.get("Percentage"))
                        .and_then(Variant::get::<u8>),
                }
            })
            .collect()
    }

    fn add(&mut self, path: &Variant, interfaces: &Variant) {
        let Some(path) = path.str() else {
            return;
        };
        let known = self.paths.entry(path.to_owned()).or_default();
        for entry in interfaces.iter() {
            let Some(name) = entry.child_value(0).str().map(str::to_owned) else {
                continue;
            };
            known
                .entry(name)
                .or_default()
                .extend(dictionary(&entry.child_value(1)));
        }
    }

    fn with<'a>(
        &'a self,
        interface: &'a str,
    ) -> impl Iterator<Item = (&'a str, &'a BTreeMap<String, Properties>)> {
        self.paths
            .iter()
            .filter(move |(_, interfaces)| interfaces.contains_key(interface))
            .map(|(path, interfaces)| (path.as_str(), interfaces))
    }
}

/// An `a{sv}`'s entries, each value unboxed from its `v`.
fn dictionary(variant: &Variant) -> impl Iterator<Item = (String, Variant)> {
    variant.iter().filter_map(|entry| {
        let name = entry.child_value(0).str()?.to_owned();
        let value = entry.child_value(1).as_variant()?;
        Some((name, value))
    })
}

/// A string, object path or signature property.
fn string(properties: &Properties, name: &str) -> Option<String> {
    properties.get(name)?.str().map(str::to_owned)
}

fn boolean(properties: &Properties, name: &str) -> bool {
    properties
        .get(name)
        .and_then(Variant::get::<bool>)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use gio::glib::variant::{ObjectPath, ToVariant};

    use super::*;

    fn interfaces(entries: &[(&str, &[(&str, Variant)])]) -> Variant {
        let map: HashMap<String, HashMap<String, Variant>> = entries
            .iter()
            .map(|(interface, properties)| {
                let properties = properties
                    .iter()
                    .map(|(name, value)| (name.to_string(), value.clone()))
                    .collect();
                (interface.to_string(), properties)
            })
            .collect();
        map.to_variant()
    }

    fn added(path: &str, entries: &[(&str, &[(&str, Variant)])]) -> Variant {
        let path = ObjectPath::try_from(path.to_owned()).unwrap();
        Variant::tuple_from_iter([path.to_variant(), interfaces(entries)])
    }

    fn headset() -> Variant {
        let adapter = ObjectPath::try_from("/org/bluez/hci0".to_owned()).unwrap();
        added(
            "/org/bluez/hci0/dev_11_22_33_44_55_66",
            &[(
                DEVICE,
                &[
                    ("Address", "11:22:33:44:55:66".to_variant()),
                    ("Alias", "Headset".to_variant()),
                    ("Adapter", adapter.to_variant()),
                    ("Paired", true.to_variant()),
                ],
            )],
        )
    }

    #[test]
    fn a_device_reads_its_properties_and_its_battery() {
        let mut objects = Objects::default();
        objects.interfaces_added(&headset());
        objects.interfaces_added(&added(
            "/org/bluez/hci0/dev_11_22_33_44_55_66",
            &[(BATTERY, &[("Percentage", 42u8.to_variant())])],
        ));

        let devices = objects.devices();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].alias, "Headset");
        assert_eq!(devices[0].adapter, "/org/bluez/hci0");
        assert!(devices[0].paired);
        assert!(!devices[0].connected);
        assert_eq!(devices[0].battery, Some(42));
    }

    #[test]
    fn a_change_updates_a_property_and_an_invalidated_one_is_forgotten() {
        let mut objects = Objects::default();
        objects.interfaces_added(&headset());
        let changed: HashMap<String, Variant> =
            HashMap::from([("Connected".to_owned(), true.to_variant())]);
        objects.properties_changed(
            "/org/bluez/hci0/dev_11_22_33_44_55_66",
            &(DEVICE, changed, vec!["Alias"]).to_variant(),
        );

        let device = &objects.devices()[0];
        assert!(device.connected);
        assert_eq!(device.alias, "11:22:33:44:55:66");
    }

    #[test]
    fn removing_the_battery_keeps_the_device_and_removing_the_device_drops_it() {
        let mut objects = Objects::default();
        let path = "/org/bluez/hci0/dev_11_22_33_44_55_66";
        objects.interfaces_added(&headset());
        objects.interfaces_added(&added(
            path,
            &[(BATTERY, &[("Percentage", 42u8.to_variant())])],
        ));
        let path_variant = ObjectPath::try_from(path.to_owned()).unwrap().to_variant();

        objects.interfaces_removed(&Variant::tuple_from_iter([
            path_variant.clone(),
            vec![BATTERY].to_variant(),
        ]));
        assert_eq!(objects.devices()[0].battery, None);

        objects.interfaces_removed(&Variant::tuple_from_iter([
            path_variant,
            vec![DEVICE].to_variant(),
        ]));
        assert!(objects.devices().is_empty());
    }
}
