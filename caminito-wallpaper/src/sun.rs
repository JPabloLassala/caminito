//! The moment a dynamic wallpaper is drawn for (216): the sun's elevation and azimuth
//! (217), as Caminito publishes them on `org.mate.Caminito`'s `Sun` (215), and the local
//! clock. The drawer and the dialog's sidebar read it alike, so the two show the same
//! picture.

use gio::glib;
use gio::prelude::*;

use crate::dynamic::Sky;

/// The sky now: the sun's elevation and azimuth from `caminito`, or none without it or its
/// `Sun`, and the local wall clock.
pub fn sky_now(caminito: Option<&gio::DBusProxy>) -> Sky {
    let sun = caminito
        .and_then(|proxy| proxy.cached_property("Sun"))
        .and_then(|sun| sun.get::<glib::VariantDict>());
    let lookup = |key: &str| {
        sun.as_ref()
            .and_then(|sun| sun.lookup::<f64>(key).ok().flatten())
    };
    let (elevation, azimuth) = (lookup("elevation"), lookup("azimuth"));
    let local = glib::DateTime::now_local()
        .map(|now| now.to_unix() + now.utc_offset().as_seconds())
        .unwrap_or_default();
    Sky {
        elevation,
        azimuth,
        local,
    }
}
