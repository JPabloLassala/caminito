//! The GTK palette frames are drawn with (07, 28): from `camino-gtk-palette` once it
//! reports, from the cache before that, and neutral with neither.

use std::collections::BTreeSet;

use camino_theme::{
    Palette, PaletteCache, PaletteCacheError, PaletteColors, PaletteReader, ReportedPalette,
    Variant, names_request,
};
use smithay::reexports::calloop::LoopHandle;

use crate::helpers::{Helper, HelperCommand, HelperEvent, RestartPolicy, StderrTarget};

/// The loop data that holds a [`PaletteSource`].
pub trait PaletteState: Sized + 'static {
    fn palette_source(&mut self) -> &mut PaletteSource<Self>;
    /// The palette in use changed.
    fn palette_changed(&mut self);
}

/// The palette in use, and the helper that reports new ones.
pub struct PaletteSource<D: 'static> {
    helper: Helper<D>,
    reader: PaletteReader,
    /// The `gtk:custom` names the helper is asked for.
    names: BTreeSet<String>,
    cache: Option<PaletteCache>,
    gtk_theme: Option<String>,
    palette: Palette,
}

impl<D: PaletteState> PaletteSource<D> {
    /// The cached palette for `gtk_theme`, or the neutral one. The helper isn't started.
    pub fn new(
        handle: LoopHandle<'static, D>,
        command: HelperCommand,
        policy: RestartPolicy,
        stderr: StderrTarget,
        cache: Option<PaletteCache>,
        gtk_theme: Option<&str>,
    ) -> Self {
        let helper = Helper::new(
            handle,
            command,
            policy,
            stderr,
            |data: &mut D| &mut data.palette_source().helper,
            Self::helper_event,
        );
        let palette = match (&cache, gtk_theme) {
            (Some(cache), Some(gtk_theme)) => cached(cache, gtk_theme),
            _ => Palette::neutral(),
        };
        PaletteSource {
            helper,
            reader: PaletteReader::new(),
            names: BTreeSet::new(),
            cache,
            gtk_theme: gtk_theme.map(str::to_owned),
            palette,
        }
    }

    pub fn start(&mut self) {
        self.helper.start();
    }

    /// Stops the helper for good, with SIGTERM. The last palette stays in use.
    pub fn stop(&mut self) {
        self.helper.stop();
    }

    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    pub fn cache(&self) -> Option<&PaletteCache> {
        self.cache.as_ref()
    }

    /// The GTK theme the palette is for, if it's known.
    pub fn gtk_theme(&self) -> Option<&str> {
        self.gtk_theme.as_deref()
    }

    /// Uses the cached palette for `gtk_theme`, if there is one, until the helper reports.
    /// Returns whether it did.
    pub fn use_cached(&mut self, gtk_theme: &str) -> bool {
        let Some(cache) = &self.cache else {
            return false;
        };
        let loaded = [Variant::Default, Variant::Dark].map(|variant| {
            cache
                .load(gtk_theme, variant)
                .inspect_err(|err| log_cache_error(cache, gtk_theme, variant, err))
                .ok()
                .flatten()
        });
        let [Some(default), Some(dark)] = loaded else {
            return false;
        };
        self.gtk_theme = Some(gtk_theme.to_owned());
        self.palette = Palette { default, dark };
        true
    }

    /// Asks for the `gtk:custom` names the Marco theme uses, now and after every restart.
    pub fn set_names(&mut self, names: BTreeSet<String>) {
        if names != self.names {
            self.names = names;
            self.ask();
        }
    }

    fn ask(&mut self) {
        let request = names_request(self.names.iter().map(String::as_str));
        match self.helper.write_stdin(request.as_bytes()) {
            Ok(()) => {}
            // It asks again when it's started.
            Err(err) if err.kind() == std::io::ErrorKind::NotConnected => {}
            Err(err) => tracing::warn!("can't ask the palette helper for colours: {err}"),
        }
    }

    fn helper_event(data: &mut D, event: HelperEvent<'_>) {
        let source = data.palette_source();
        match event {
            HelperEvent::Started => {
                source.reader = PaletteReader::new();
                source.ask();
            }
            HelperEvent::Stdout(bytes) => {
                for result in source.reader.feed(bytes) {
                    match result {
                        Ok(reported) => {
                            data.palette_source().put_in_use(reported);
                            data.palette_changed();
                        }
                        Err(err) => tracing::warn!("palette helper: {err}"),
                    }
                }
            }
            HelperEvent::Exited => {
                if let Err(err) = source.reader.finish() {
                    tracing::warn!("palette helper: {err}");
                }
            }
            // The supervisor logged it, and the last palette stays in use.
            HelperEvent::GaveUp => {}
        }
    }

    fn put_in_use(&mut self, reported: ReportedPalette) {
        if let Some(cache) = &self.cache {
            let variants = [
                (Variant::Default, &reported.palette.default),
                (Variant::Dark, &reported.palette.dark),
            ];
            for (variant, colors) in variants {
                if let Err(err) = cache.store(&reported.gtk_theme, variant, colors) {
                    tracing::warn!("can't cache the palette for {}: {err}", reported.gtk_theme);
                }
            }
        }
        self.gtk_theme = Some(reported.gtk_theme);
        self.palette = reported.palette;
    }
}

/// Each variant from the cache, or neutral.
fn cached(cache: &PaletteCache, gtk_theme: &str) -> Palette {
    let neutral = Palette::neutral();
    let load = |variant: Variant, neutral: PaletteColors| match cache.load(gtk_theme, variant) {
        Ok(Some(colors)) => colors,
        Ok(None) => neutral,
        Err(err) => {
            log_cache_error(cache, gtk_theme, variant, &err);
            neutral
        }
    };
    Palette {
        default: load(Variant::Default, neutral.default),
        dark: load(Variant::Dark, neutral.dark),
    }
}

fn log_cache_error(
    cache: &PaletteCache,
    gtk_theme: &str,
    variant: Variant,
    err: &PaletteCacheError,
) {
    let path = cache.path(gtk_theme, variant).unwrap_or_default();
    tracing::warn!("ignoring the cached palette {}: {err}", path.display());
}
