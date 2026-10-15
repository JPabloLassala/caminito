//! The moment a dynamic wallpaper is drawn for (216): the sun's elevation, as Caminito
//! publishes it on `org.mate.Caminito`'s `Sun` (215), and the local clock. The drawer and
//! the dialog's sidebar read it alike, so the two show the same picture.

use gio::glib;
use gio::prelude::*;

use crate::dynamic::Sky;

/// The sky now: the sun's elevation from `caminito`, or none without it or its `Sun`, and
/// the local wall clock.
pub fn sky_now(caminito: Option<&gio::DBusProxy>) -> Sky {
    let elevation = caminito
        .and_then(|proxy| proxy.cached_property("Sun"))
        .and_then(|sun| sun.get::<glib::VariantDict>())
        .and_then(|sun| sun.lookup::<f64>("elevation").ok().flatten());
    let local = glib::DateTime::now_local()
        .map(|now| now.to_unix() + now.utc_offset().as_seconds())
        .unwrap_or_default();
    Sky { elevation, local }
}
