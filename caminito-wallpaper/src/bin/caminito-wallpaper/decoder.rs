//! The pictures the dialog shows small, decoded off the main loop (212): the grid's
//! thumbnails and the sidebar's previews. A few of GLib's pool threads decode at once, in
//! the order asked, and each picture is decoded once per size and kept.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::gdk_pixbuf::Pixbuf;
use gtk::{gio, glib};

/// How many pictures are decoded at once.
const WORKERS: usize = 4;

/// A picture decoded to fit a box, the way the drawer shows it: turned as its EXIF says.
pub(crate) struct Decoded {
    pub(crate) pixbuf: Pixbuf,
    /// The picture's own size, turned, in pixels.
    pub(crate) original: (i32, i32),
}

/// A picture, and the box it's decoded to fit.
type Key = (PathBuf, i32, i32);

/// What's done with a picture once it's decoded, or with `None` if it can't be.
type Done = Box<dyn FnOnce(Option<Rc<Decoded>>)>;

/// The decoder, shared by the window's parts.
#[derive(Default)]
pub(crate) struct Decoder {
    decoded: RefCell<HashMap<Key, Option<Rc<Decoded>>>>,
    /// The pictures asked for and not decoded yet, and who asked.
    waiting: RefCell<HashMap<Key, Vec<Done>>>,
    queue: RefCell<VecDeque<Key>>,
    workers: Cell<usize>,
}

impl Decoder {
    /// The picture at `path` fit to `width` by `height`, if it's decoded already.
    pub(crate) fn get(&self, path: &Path, width: i32, height: i32) -> Option<Option<Rc<Decoded>>> {
        self.decoded
            .borrow()
            .get(&(path.to_owned(), width, height))
            .cloned()
    }

    /// Calls `done` with the picture at `path` fit to `width` by `height`: at once if it's
    /// decoded, or once it is.
    pub(crate) fn request(
        self: &Rc<Self>,
        path: &Path,
        width: i32,
        height: i32,
        done: impl FnOnce(Option<Rc<Decoded>>) + 'static,
    ) {
        let key = (path.to_owned(), width, height);
        if let Some(decoded) = self.decoded.borrow().get(&key).cloned() {
            done(decoded);
            return;
        }
        let mut waiting = self.waiting.borrow_mut();
        let first = !waiting.contains_key(&key);
        waiting.entry(key.clone()).or_default().push(Box::new(done));
        drop(waiting);
        if first {
            self.queue.borrow_mut().push_back(key);
            self.start();
        }
    }

    /// Starts a worker if fewer than [`WORKERS`] run.
    fn start(self: &Rc<Self>) {
        if self.workers.get() >= WORKERS {
            return;
        }
        self.workers.set(self.workers.get() + 1);
        let decoder = self.clone();
        glib::MainContext::default().spawn_local(async move {
            loop {
                let Some(key) = decoder.queue.borrow_mut().pop_front() else {
                    break;
                };
                let (path, width, height) = key.clone();
                let decoded = gio::spawn_blocking(move || decode(&path, width, height))
                    .await
                    .ok()
                    .flatten()
                    .map(|pixels| {
                        Rc::new(Decoded {
                            pixbuf: pixels.pixbuf(),
                            original: pixels.original,
                        })
                    });
                decoder
                    .decoded
                    .borrow_mut()
                    .insert(key.clone(), decoded.clone());
                let waiting = decoder.waiting.borrow_mut().remove(&key);
                for done in waiting.into_iter().flatten() {
                    done(decoded.clone());
                }
            }
            decoder.workers.set(decoder.workers.get() - 1);
        });
    }
}

/// A decoded picture's pixels, which a pool thread can hand to the main loop, as a `Pixbuf`
/// can't be.
struct Pixels {
    bytes: glib::Bytes,
    has_alpha: bool,
    width: i32,
    height: i32,
    rowstride: i32,
    original: (i32, i32),
}

impl Pixels {
    fn of(pixbuf: &Pixbuf, original: (i32, i32)) -> Self {
        Pixels {
            bytes: pixbuf.read_pixel_bytes(),
            has_alpha: pixbuf.has_alpha(),
            width: pixbuf.width(),
            height: pixbuf.height(),
            rowstride: pixbuf.rowstride(),
            original,
        }
    }

    fn pixbuf(&self) -> Pixbuf {
        Pixbuf::from_bytes(
            &self.bytes,
            gtk::gdk_pixbuf::Colorspace::Rgb,
            self.has_alpha,
            8,
            self.width,
            self.height,
            self.rowstride,
        )
    }
}

/// Decodes the picture at `path` to fit `width` by `height`, turned as its EXIF says, with
/// its own size, turned too. On a pool thread.
fn decode(path: &Path, width: i32, height: i32) -> Option<Pixels> {
    let (_, own_width, own_height) = Pixbuf::file_info(path)?;
    let pixbuf = Pixbuf::from_file_at_scale(path, width, height, true).ok()?;
    // 5 to 8 are a quarter turn, with or without a flip.
    let turned = pixbuf
        .option("orientation")
        .is_some_and(|orientation| matches!(orientation.as_str(), "5" | "6" | "7" | "8"));
    let pixbuf = pixbuf.apply_embedded_orientation().unwrap_or(pixbuf);
    let original = if turned {
        (own_height, own_width)
    } else {
        (own_width, own_height)
    };
    Some(Pixels::of(&pixbuf, original))
}
