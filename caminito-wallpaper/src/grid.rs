//! The Picture grid's groups (212): the pictures the dialog offers, one heading per source,
//! in the spec's order. The current picture first, then the pictures and each folder the
//! user added, then MATE's (`/usr/share/backgrounds`) and Plasma's (`/usr/share/wallpapers`,
//! one picture per package). `~/Pictures` isn't scanned.
//!
//! The scan reads directories and no picture, so it's quick enough for the main loop; the
//! thumbnails are decoded elsewhere.

use std::fs;
use std::path::{Path, PathBuf};

/// The file extensions of the pictures offered, lower case. HEIC is a dynamic wallpaper's
/// (217), and XML a timed one's (216).
const EXTENSIONS: [&str; 9] = [
    "jpg", "jpeg", "png", "webp", "svg", "bmp", "tif", "tiff", "jxl",
];

/// How deep MATE's directory is looked into: `/usr/share/backgrounds/mate/nature/` is two.
const DEPTH: usize = 3;

/// Where the pictures already on the disk are.
#[derive(Debug, Clone)]
pub struct Roots {
    /// MATE's, and whatever else installs into it.
    pub mate: PathBuf,
    /// Plasma's packages.
    pub plasma: PathBuf,
}

impl Default for Roots {
    fn default() -> Self {
        Roots {
            mate: "/usr/share/backgrounds".into(),
            plasma: "/usr/share/wallpapers".into(),
        }
    }
}

/// One heading of the grid and its pictures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// What the heading says: the source, or the added folder's name.
    pub heading: String,
    /// The source's pictures, in the grid's order.
    pub pictures: Vec<PathBuf>,
}

/// The pictures already on the disk, MATE's and Plasma's, scanned once when the dialog
/// opens: they change with a package, not while the dialog is open.
#[derive(Debug, Clone, Default)]
pub struct Installed {
    mate: Vec<PathBuf>,
    plasma: Vec<PathBuf>,
}

impl Installed {
    /// Scans `roots`.
    pub fn scan(roots: &Roots) -> Self {
        Installed {
            mate: pictures_in(&roots.mate, DEPTH),
            plasma: plasma(&roots.plasma),
        }
    }
}

/// The grid's groups, in order, with none empty. `current` is the selected output's picture,
/// `pictures` and `folders` the user's. A folder the user added is read each time, at its
/// own level alone, so that a picture saved into it shows at the next click.
pub fn groups(
    current: Option<&str>,
    pictures: &[String],
    folders: &[String],
    installed: &Installed,
) -> Vec<Group> {
    let mut groups = Vec::new();
    let mut push = |heading: String, pictures: Vec<PathBuf>| {
        if !pictures.is_empty() {
            groups.push(Group { heading, pictures });
        }
    };
    push(
        "Current picture".into(),
        current
            .filter(|current| !current.is_empty())
            .map(PathBuf::from)
            .into_iter()
            .collect(),
    );
    push(
        "Your pictures".into(),
        pictures.iter().map(PathBuf::from).collect(),
    );
    for folder in folders {
        let folder = Path::new(folder);
        let heading = folder.file_name().map_or_else(
            || folder.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        push(heading, pictures_in(folder, 1));
    }
    push("MATE".into(), installed.mate.clone());
    push("Plasma".into(), installed.plasma.clone());
    groups
}

/// Whether `path` names a picture the grid offers, by its extension.
fn is_picture(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()))
}

/// The entries of `directory`, sorted by name, or none if it can't be read.
fn entries(directory: &Path) -> Vec<PathBuf> {
    let Ok(read) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut entries: Vec<PathBuf> = read.flatten().map(|entry| entry.path()).collect();
    entries.sort();
    entries
}

/// The pictures in `folder` and its folders `depth` deep, 1 being the folder's own, in the
/// order of their paths.
fn pictures_in(folder: &Path, depth: usize) -> Vec<PathBuf> {
    let mut pictures = Vec::new();
    if depth == 0 {
        return pictures;
    }
    for entry in entries(folder) {
        if entry.is_dir() {
            pictures.extend(pictures_in(&entry, depth - 1));
        } else if is_picture(&entry) {
            pictures.push(entry);
        }
    }
    pictures
}

/// One picture per package of Plasma's, and any picture beside the packages.
fn plasma(root: &Path) -> Vec<PathBuf> {
    let mut pictures = Vec::new();
    for entry in entries(root) {
        if entry.is_dir() {
            let images = entry.join("contents").join("images");
            let names: Vec<String> = entries(&images)
                .iter()
                .filter(|path| is_picture(path))
                .filter_map(|path| path.file_name()?.to_str().map(str::to_owned))
                .collect();
            if let Some(name) = package_picture(&names) {
                pictures.push(images.join(name));
            }
        } else if is_picture(&entry) {
            pictures.push(entry);
        }
    }
    pictures
}

/// The picture a Plasma package is shown by, of its `contents/images/`'s file names, which
/// are its pictures' sizes, as `1920x1080.png`: the largest landscape one, or the largest of
/// any shape if none is landscape, or the first by name if none is named by its size.
fn package_picture(names: &[String]) -> Option<&str> {
    let size = |name: &str| {
        let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
        let (width, height) = stem.split_once('x')?;
        Some((width.parse::<u32>().ok()?, height.parse::<u32>().ok()?))
    };
    let largest = |landscape_only: bool| {
        names
            .iter()
            .filter_map(|name| size(name).map(|size| (name, size)))
            .filter(|&(_, (width, height))| !landscape_only || width > height)
            .max_by_key(|&(_, (width, height))| u64::from(width) * u64::from(height))
            .map(|(name, _)| name.as_str())
    };
    largest(true)
        .or_else(|| largest(false))
        .or_else(|| names.iter().min().map(String::as_str))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(names: &[&str]) -> Vec<String> {
        names.iter().map(|&name| name.to_owned()).collect()
    }

    #[test]
    fn a_plasma_package_is_shown_by_its_largest_landscape_picture() {
        let package = names(&[
            "1080x1920.png",
            "1920x1080.png",
            "5120x2880.png",
            "2880x5120.png",
            "3840x2160.png",
        ]);
        assert_eq!(package_picture(&package), Some("5120x2880.png"));
    }

    #[test]
    fn a_plasma_package_of_portrait_pictures_is_shown_by_its_largest() {
        let package = names(&["720x1440.jpg", "1080x2160.jpg"]);
        assert_eq!(package_picture(&package), Some("1080x2160.jpg"));
    }

    #[test]
    fn a_plasma_package_not_named_by_size_is_shown_by_its_first_picture() {
        assert_eq!(
            package_picture(&names(&["screenshot.png", "image.jpg"])),
            Some("image.jpg")
        );
        assert_eq!(package_picture(&[]), None);
    }

    /// Makes an empty file at `path`, and its folders.
    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"").unwrap();
    }

    #[test]
    fn the_groups_are_current_added_folders_mate_and_plasma_in_that_order() {
        let disk = tempfile::tempdir().unwrap();
        let root = disk.path();
        let roots = Roots {
            mate: root.join("backgrounds"),
            plasma: root.join("wallpapers"),
        };
        touch(&roots.mate.join("mate/nature/Wood.jpg"));
        touch(&roots.mate.join("mate/desktop/Stripes.png"));
        touch(&roots.mate.join("mate/nature.xml"));
        touch(&roots.mate.join("ubuntu.jpg"));
        touch(&roots.plasma.join("Next/contents/images/1920x1080.png"));
        touch(&roots.plasma.join("Next/contents/images/5120x2880.png"));
        touch(&roots.plasma.join("Next/contents/images/1080x1920.png"));
        touch(&roots.plasma.join("Next/contents/screenshot.png"));
        touch(&roots.plasma.join("Flow/contents/images/3840x2160.jpg"));
        touch(&roots.plasma.join("Empty/metadata.json"));
        touch(&roots.plasma.join("loose.jpg"));
        let walls = root.join("Walls");
        touch(&walls.join("b.JPG"));
        touch(&walls.join("a.png"));
        touch(&walls.join("notes.txt"));
        touch(&walls.join("deeper/c.png"));
        let walls = walls.to_str().unwrap().to_owned();

        let groups = groups(
            Some("/home/me/sea.jpg"),
            &["/home/me/sea.jpg".into(), "/home/me/sky.png".into()],
            &[walls.clone(), root.join("Missing").to_str().unwrap().into()],
            &Installed::scan(&roots),
        );

        let headings: Vec<&str> = groups.iter().map(|g| g.heading.as_str()).collect();
        assert_eq!(
            headings,
            [
                "Current picture",
                "Your pictures",
                "Walls",
                "MATE",
                "Plasma"
            ]
        );
        assert_eq!(groups[0].pictures, [PathBuf::from("/home/me/sea.jpg")]);
        assert_eq!(groups[1].pictures.len(), 2);
        let walls = Path::new(&walls);
        assert_eq!(
            groups[2].pictures,
            [walls.join("a.png"), walls.join("b.JPG")]
        );
        assert_eq!(
            groups[3].pictures,
            [
                roots.mate.join("mate/desktop/Stripes.png"),
                roots.mate.join("mate/nature/Wood.jpg"),
                roots.mate.join("ubuntu.jpg"),
            ]
        );
        assert_eq!(
            groups[4].pictures,
            [
                roots.plasma.join("Flow/contents/images/3840x2160.jpg"),
                roots.plasma.join("Next/contents/images/5120x2880.png"),
                roots.plasma.join("loose.jpg"),
            ]
        );
    }

    #[test]
    fn with_nothing_chosen_added_or_installed_the_grid_is_empty() {
        let roots = Roots {
            mate: "/nonexistent/backgrounds".into(),
            plasma: "/nonexistent/wallpapers".into(),
        };
        let installed = Installed::scan(&roots);
        assert_eq!(groups(Some(""), &[], &[], &installed), []);
        assert_eq!(groups(None, &[], &[], &installed), []);
    }
}
