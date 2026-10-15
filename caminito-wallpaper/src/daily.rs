//! Picture of the day (214): today's picture from one of three sources, none of which needs
//! a key, behind one interface, [`Feed`], so that more can follow. Each source's latest
//! picture is kept in the cache with its title and credit, and stays there when a fetch
//! fails or the day's item isn't a picture, so an output never goes blank for want of the
//! network.
//!
//! A fetch is two requests: the source's page or feed for the day, which [`Feed::parse`]
//! reads, and the picture it names. [`update`] runs them through a [`Fetch`], which is the
//! drawer's HTTP client ([`crate::http`]) and the tests' fake, so no test reaches the
//! network. Nothing here names a toolkit or reads a clock: the day is given.

use std::fmt;
use std::fs;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::http;

/// How long after a fetch that didn't bring today's picture another is tried, in seconds:
/// a source that publishes after local midnight, or a session that started offline, gets
/// today's within the hour rather than tomorrow.
pub(crate) const RETRY: i64 = 3600;

/// The widest picture asked of a source that scales its own, as Wikimedia Commons does: a
/// 4K output's.
const WIDEST: u64 = 3840;

/// One of the sources of a picture of the day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Source {
    /// NASA's Astronomy Picture of the Day.
    Apod,
    /// Bing's picture of the day, the one on its home page.
    #[default]
    Bing,
    /// Wikimedia Commons' Picture of the Day.
    Commons,
}

impl Source {
    /// Each source, in the dialog's order.
    pub const ALL: [Self; 3] = [Self::Apod, Self::Bing, Self::Commons];

    /// Its nick in the schema's `org.mate.Caminito.PictureOfTheDaySource`, and in the cache.
    pub fn nick(self) -> &'static str {
        match self {
            Self::Apod => "apod",
            Self::Bing => "bing",
            Self::Commons => "commons",
        }
    }

    /// The source of a nick, or the default for one the schema doesn't have.
    pub fn from_nick(nick: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|source| source.nick() == nick)
            .unwrap_or_default()
    }

    /// What the user reads for it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Apod => "NASA Astronomy Picture of the Day",
            Self::Bing => "Bing",
            Self::Commons => "Wikimedia Commons",
        }
    }

    /// The dialog's line on it.
    pub fn line(self) -> &'static str {
        match self {
            Self::Apod => {
                "A picture of the universe, chosen and explained by astronomers. Some days it's a video, and yesterday's picture stays."
            }
            Self::Bing => "A landscape or an animal from around the world, as on Bing's home page.",
            Self::Commons => {
                "A photograph or an illustration chosen by Wikimedia Commons' contributors, under a free licence."
            }
        }
    }

    /// The source's parser.
    pub(crate) fn feed(self) -> &'static dyn Feed {
        match self {
            Self::Apod => &Apod,
            Self::Bing => &Bing,
            Self::Commons => &Commons,
        }
    }
}

/// A day of the calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day {
    /// The year, as 2026.
    pub year: i32,
    /// The month, 1 to 12.
    pub month: u8,
    /// The day of the month, from 1.
    pub day: u8,
}

impl Day {
    /// The day of `local`, seconds since 1970-01-01 00:00 of the local wall clock, as a
    /// dynamic wallpaper's [`Sky`](crate::dynamic::Sky) has it.
    pub fn of_local(local: i64) -> Self {
        // Howard Hinnant's `civil_from_days`.
        let days = local.div_euclid(86_400) + 719_468;
        let era = days.div_euclid(146_097);
        let of_era = days.rem_euclid(146_097);
        let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
        let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let shifted_month = (5 * of_year + 2) / 153;
        let day = of_year - (153 * shifted_month + 2) / 5 + 1;
        let month = if shifted_month < 10 {
            shifted_month + 3
        } else {
            shifted_month - 9
        };
        let year = year_of_era + era * 400 + i64::from(month <= 2);
        Day {
            year: year as i32,
            month: month as u8,
            day: day as u8,
        }
    }

    /// The day of `YYYY-MM-DD`, or `None` for other text.
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.splitn(3, '-');
        let year = parts.next()?.parse().ok()?;
        let month = parts.next()?.parse().ok()?;
        let day = parts.next()?.parse().ok()?;
        Self::new(year, month, day)
    }

    fn new(year: i32, month: u8, day: u8) -> Option<Self> {
        ((1..=12).contains(&month) && (1..=31).contains(&day)).then_some(Day { year, month, day })
    }
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// A source's item for a day, as its page or feed gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Item {
    /// The picture's URL.
    pub(crate) picture: String,
    /// What it shows, as the source names it.
    pub(crate) title: String,
    /// Whose it is, and its licence where the source gives one.
    pub(crate) credit: String,
    /// The day the source says it is for.
    pub(crate) day: Day,
}

/// Why a source has no picture for the day, or it couldn't be fetched or kept.
#[derive(Debug)]
pub(crate) enum Error {
    /// The request failed: no network, or the server said no.
    Fetch(http::Error),
    /// The page or feed isn't as the source writes it.
    Malformed(&'static str),
    /// The day's item isn't a usable picture, as NASA's video days: the reason.
    NotAPicture(&'static str),
    /// What came back for the picture isn't one.
    NotAnImage,
    /// The cache can't be written.
    Cache(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Fetch(err) => write!(f, "{err}"),
            Error::Malformed(what) => write!(f, "the source's answer has no {what}"),
            Error::NotAPicture(why) => write!(f, "today's isn't a picture: {why}"),
            Error::NotAnImage => write!(f, "what came back isn't a picture"),
            Error::Cache(err) => write!(f, "can't keep the picture: {err}"),
        }
    }
}

impl std::error::Error for Error {}

/// A source of a picture of the day: where its item for a day is, and how to read it.
pub(crate) trait Feed {
    /// The URL of the page or feed with the item for `day`.
    fn request(&self, day: Day) -> String;
    /// The item in `body`, what came back from [`Feed::request`]'s URL for `day`.
    fn parse(&self, body: &[u8], day: Day) -> Result<Item, Error>;
}

/// NASA's Astronomy Picture of the Day, read from its page of today, since its API needs a
/// key. Today is NASA's, which starts at midnight in the US's east: a dated page asked for
/// by a local day ahead of it isn't there yet. The page's picture is the link around its
/// image; a video day has an `<iframe>` instead.
struct Apod;

const APOD: &str = "https://apod.nasa.gov/apod/";

impl Feed for Apod {
    fn request(&self, _day: Day) -> String {
        format!("{APOD}astropix.html")
    }

    fn parse(&self, body: &[u8], day: Day) -> Result<Item, Error> {
        let page = String::from_utf8_lossy(body);
        let lower = page.to_ascii_lowercase();
        let head =
            between(&page, &lower, "<title>", "</title>").ok_or(Error::Malformed("title"))?;
        let head = plain(head);
        let head = head.strip_prefix("APOD:").unwrap_or(&head).trim();
        let (date, title) = head.split_once(" - ").ok_or(Error::Malformed("title"))?;
        let day = apod_day(date).unwrap_or(day);
        let Some(link) = lower.find("href=\"image/") else {
            return Err(Error::NotAPicture(
                if lower.contains("<iframe") || lower.contains("<video") {
                    "it's a video"
                } else {
                    "it has no picture to show"
                },
            ));
        };
        let start = link + "href=\"".len();
        let end = start + page[start..].find('"').ok_or(Error::Malformed("picture"))?;
        // The credit is under the picture: its label, as `<b> Image Credit & Copyright: </b>`,
        // then the names, to the end of the block. A title with "credit" in it is no label.
        let credit = lower[end..]
            .match_indices("credit")
            .map(|(at, _)| end + at)
            .find_map(|at| {
                let label = lower[..at].rfind("<b>")?;
                let label_end = at + lower[at..].find("</b>")?;
                plain(&page[label..label_end])
                    .ends_with(':')
                    .then(|| Some(plain(&page[label..at + lower[at..].find("</center>")?])))
                    .flatten()
            })
            .unwrap_or_default();
        Ok(Item {
            picture: format!("{APOD}{}", &page[start..end]),
            title: title.trim().to_owned(),
            credit,
            day,
        })
    }
}

/// The day of APOD's `2026 October 15`.
fn apod_day(text: &str) -> Option<Day> {
    const MONTHS: [&str; 12] = [
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ];
    let mut words = text.split_whitespace();
    let year = words.next()?.parse().ok()?;
    let month = words.next()?.to_ascii_lowercase();
    let month = MONTHS.iter().position(|name| *name == month)? as u8 + 1;
    let day = words.next()?.parse().ok()?;
    Day::new(year, month, day)
}

/// Bing's picture of the day, from the feed its home page reads. `wp` is false on a day
/// whose picture may not be used as a wallpaper.
struct Bing;

const BING: &str = "https://www.bing.com";

impl Feed for Bing {
    fn request(&self, _day: Day) -> String {
        // Today's, which Bing changes at its own midnight.
        format!("{BING}/HPImageArchive.aspx?format=js&idx=0&n=1&mkt=en-US")
    }

    fn parse(&self, body: &[u8], day: Day) -> Result<Item, Error> {
        let json: Value = serde_json::from_slice(body).map_err(|_| Error::Malformed("JSON"))?;
        let image = json
            .get("images")
            .and_then(|images| images.get(0))
            .ok_or(Error::Malformed("image"))?;
        if image.get("wp").and_then(Value::as_bool) == Some(false) {
            return Err(Error::NotAPicture("Bing doesn't offer it as a wallpaper"));
        }
        let text = |key: &str| image.get(key).and_then(Value::as_str).unwrap_or_default();
        let picture = match (text("urlbase"), text("url")) {
            // The largest Bing has, 3840×2160.
            (base, _) if base.starts_with('/') => format!("{BING}{base}_UHD.jpg"),
            (_, url) if url.starts_with('/') => format!("{BING}{url}"),
            _ => return Err(Error::Malformed("picture")),
        };
        // "What it shows, where (© Who/Agency)".
        let copyright = text("copyright");
        let (about, credit) = match copyright.rsplit_once(" (") {
            Some((about, credit)) => (about, credit.trim_end_matches(')')),
            None => (copyright, ""),
        };
        let title = match text("title") {
            "" | "Info" => about,
            title => title,
        };
        // `20261015`.
        let start = text("startdate");
        let day = (start.len() == 8 && start.is_ascii())
            .then(|| {
                Day::new(
                    start[..4].parse().ok()?,
                    start[4..6].parse().ok()?,
                    start[6..].parse().ok()?,
                )
            })
            .flatten()
            .unwrap_or(day);
        Ok(Item {
            picture,
            title: title.trim().to_owned(),
            credit: credit.trim().to_owned(),
            day,
        })
    }
}

/// Wikimedia Commons' Picture of the Day, as Wikipedia's feed of the day's featured content
/// gives it, with its author and licence. A picture wider than [`WIDEST`] is asked of
/// Commons' scaler at that width, and a file of another kind, as a TIFF or an SVG, as a
/// scaled JPEG or PNG; a video is no picture.
struct Commons;

impl Feed for Commons {
    fn request(&self, day: Day) -> String {
        format!(
            "https://en.wikipedia.org/api/rest_v1/feed/featured/{:04}/{:02}/{:02}",
            day.year, day.month, day.day
        )
    }

    fn parse(&self, body: &[u8], day: Day) -> Result<Item, Error> {
        let json: Value = serde_json::from_slice(body).map_err(|_| Error::Malformed("JSON"))?;
        let Some(image) = json.get("image").filter(|image| image.is_object()) else {
            return Err(Error::NotAPicture("Commons hasn't chosen one yet"));
        };
        let text =
            |value: Option<&Value>| value.and_then(Value::as_str).map(plain).unwrap_or_default();
        let original = text(image.pointer("/image/source"));
        let width = image
            .pointer("/image/width")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX);
        let extension = original
            .rsplit_once('.')
            .map(|(_, extension)| extension.to_ascii_lowercase())
            .unwrap_or_default();
        let picture = match extension.as_str() {
            "jpg" | "jpeg" | "png" | "webp" if width <= WIDEST => original,
            "jpg" | "jpeg" | "png" | "webp" | "gif" | "tif" | "tiff" | "svg" => {
                scaled(&text(image.pointer("/thumbnail/source")), width.min(WIDEST))
                    .ok_or(Error::Malformed("picture"))?
            }
            _ => return Err(Error::NotAPicture("it's a video or a sound")),
        };
        let file = text(image.get("title"));
        let file = file.strip_prefix("File:").unwrap_or(&file);
        let file = file.rsplit_once('.').map_or(file, |(name, _)| name);
        // The caption is a title's length; the description, a paragraph, stands in for it.
        let title = [
            text(image.pointer("/structured/captions/en")),
            text(image.pointer("/description/text")),
            file.to_owned(),
        ]
        .into_iter()
        .find(|title| !title.is_empty())
        .unwrap_or_default();
        let artist = text(image.pointer("/artist/text"));
        let licence = text(image.pointer("/license/type"));
        let credit = [artist, licence]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
        Ok(Item {
            picture,
            title,
            credit,
            day,
        })
    }
}

/// Commons' thumbnail URL `thumbnail`, as `…/thumb/a/ab/Name.jpg/640px-Name.jpg`, at
/// `width` pixels wide.
fn scaled(thumbnail: &str, width: u64) -> Option<String> {
    let (folder, name) = thumbnail.rsplit_once('/')?;
    let (pixels, rest) = name.split_once("px-")?;
    pixels
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| format!("{folder}/{width}px-{rest}"))
}

/// The text in `page` between the first `open` and the `close` after it, found in `lower`,
/// `page` in lower case.
fn between<'a>(page: &'a str, lower: &str, open: &str, close: &str) -> Option<&'a str> {
    let start = lower.find(open)? + open.len();
    let end = start + lower[start..].find(close)?;
    Some(&page[start..end])
}

/// HTML's text: the tags left out, the entities a page or a feed uses read, and the
/// whitespace one space.
fn plain(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    for character in html.chars() {
        match character {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                text.push(' ');
            }
            _ if in_tag => {}
            _ => text.push(character),
        }
    }
    let mut decoded = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(at) = rest.find('&') {
        decoded.push_str(&rest[..at]);
        rest = &rest[at..];
        let entity = rest[1..]
            .find(';')
            .filter(|&end| end <= 8)
            .and_then(|end| Some((entity(&rest[1..=end])?, end + 2)));
        match entity {
            Some((character, length)) => {
                decoded.push(character);
                rest = &rest[length..];
            }
            None => {
                decoded.push('&');
                rest = &rest[1..];
            }
        }
    }
    decoded.push_str(rest);
    let words: Vec<&str> = decoded.split_whitespace().collect();
    // A tag's space before punctuation, as `<a>Name</a>,`, is the tag's, not the text's.
    words
        .join(" ")
        .replace(" ,", ",")
        .replace(" .", ".")
        .replace(" :", ":")
        .replace("( ", "(")
        .replace(" )", ")")
}

/// The character of an entity's name, between `&` and `;`.
fn entity(name: &str) -> Option<char> {
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" | "#39" => '\'',
        "nbsp" => ' ',
        "aacute" => 'á',
        "eacute" => 'é',
        "iacute" => 'í',
        "oacute" => 'ó',
        "uacute" => 'ú',
        "egrave" => 'è',
        "ntilde" => 'ñ',
        "ccedil" => 'ç',
        "auml" => 'ä',
        "ouml" => 'ö',
        "uuml" => 'ü',
        "ndash" => '–',
        "mdash" => '—',
        _ => {
            let number = name.strip_prefix('#')?;
            let code = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse().ok()?,
            };
            char::from_u32(code)?
        }
    })
}

/// The extension of a picture by its first bytes, or `None` for what isn't one of the
/// formats the sources serve, such as an HTML page of an error.
pub(crate) fn picture_extension(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("jpg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("gif")
    } else {
        None
    }
}

/// A source's picture as the cache keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    /// The picture's file.
    pub picture: PathBuf,
    /// Where it came from, so the same picture isn't fetched again.
    pub url: String,
    /// What it shows, as the source names it.
    pub title: String,
    /// Whose it is, and its licence where the source gives one.
    pub credit: String,
    /// The day the source says it is for.
    pub day: Day,
}

/// Where each source's latest picture is kept: `<nick>.about`, its title, credit, day and
/// file, and the file, `<nick>-<day>-<hash>.<extension>`, named anew for each picture so
/// the drawer, which knows a picture by its file, draws a new one.
#[derive(Debug, Clone)]
pub struct Cache {
    root: PathBuf,
}

impl Cache {
    /// The cache in `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Cache { root: root.into() }
    }

    /// The user's: `~/.cache/caminito-wallpaper/picture-of-the-day`.
    pub fn user() -> Self {
        Self::new(
            gio::glib::user_cache_dir()
                .join("caminito-wallpaper")
                .join("picture-of-the-day"),
        )
    }

    /// Its folder.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn about(&self, source: Source) -> PathBuf {
        self.root.join(format!("{}.about", source.nick()))
    }

    /// The latest picture of `source`, or `None` before the first is kept.
    pub fn latest(&self, source: Source) -> Option<Kept> {
        let about = fs::read_to_string(self.about(source)).ok()?;
        let field = |key: &str| {
            about.lines().find_map(|line| {
                let (name, value) = line.split_once('=')?;
                (name == key).then(|| value.to_owned())
            })
        };
        let picture = self.root.join(field("picture")?);
        picture.is_file().then_some(())?;
        Some(Kept {
            picture,
            url: field("url")?,
            title: field("title").unwrap_or_default(),
            credit: field("credit").unwrap_or_default(),
            day: Day::parse(&field("day")?)?,
        })
    }

    /// Keeps `bytes`, a picture of extension `extension`, as `source`'s latest, `item`,
    /// and removes the one it replaces.
    pub(crate) fn keep(
        &self,
        source: Source,
        item: &Item,
        bytes: &[u8],
        extension: &str,
    ) -> io::Result<Kept> {
        fs::create_dir_all(&self.root)?;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        item.picture.hash(&mut hasher);
        let name = format!(
            "{}-{}-{:08x}.{extension}",
            source.nick(),
            item.day,
            hasher.finish() as u32
        );
        let picture = self.root.join(&name);
        write_whole(&picture, bytes)?;
        let line = |text: &str| text.replace(['\n', '\r'], " ");
        let about = format!(
            "day={}\npicture={name}\nurl={}\ntitle={}\ncredit={}\n",
            item.day,
            line(&item.picture),
            line(&item.title),
            line(&item.credit)
        );
        write_whole(&self.about(source), about.as_bytes())?;
        let prefix = format!("{}-", source.nick());
        for entry in fs::read_dir(&self.root)?.flatten() {
            let file = entry.file_name();
            let file = file.to_string_lossy();
            if file.starts_with(&prefix) && file != name {
                let _ = fs::remove_file(entry.path());
            }
        }
        Ok(Kept {
            picture,
            url: item.picture.clone(),
            title: item.title.clone(),
            credit: item.credit.clone(),
            day: item.day,
        })
    }
}

/// Writes `bytes` to `path` whole or not at all, so that a reader never finds half a
/// picture.
fn write_whole(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut part = path.as_os_str().to_owned();
    part.push(".part");
    fs::write(&part, bytes)?;
    fs::rename(&part, path)
}

/// Copies `kept` into `folder`, as "Keep this picture" does, named by its day and title,
/// and returns the copy. A copy already there is left as it is.
pub fn keep_copy(kept: &Kept, folder: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(folder)?;
    let extension = kept
        .picture
        .extension()
        .map(|extension| extension.to_string_lossy().into_owned())
        .unwrap_or_else(|| "jpg".into());
    let title: String = kept
        .title
        .chars()
        .map(|character| match character {
            '/' | '\\' | '\0' => '-',
            character => character,
        })
        .take(80)
        .collect();
    let name = match title.trim() {
        "" => format!("{}.{extension}", kept.day),
        title => format!("{} {title}.{extension}", kept.day),
    };
    let copy = folder.join(name);
    if !copy.exists() {
        write_whole(&copy, &fs::read(&kept.picture)?)?;
    }
    Ok(copy)
}

/// What fetches a URL's body: the drawer's HTTP client, or the tests' fake.
pub(crate) trait Fetch {
    /// The body at `url`, once the server answers it.
    fn get(&self, url: &str) -> impl Future<Output = Result<Vec<u8>, http::Error>>;
}

/// Fetches `source`'s item for `today` and its picture, and keeps it in `cache`, unless
/// it's the one kept already. Returns what the cache keeps for the source now; on an error
/// the cache is as it was.
pub(crate) async fn update(
    fetch: &impl Fetch,
    cache: &Cache,
    source: Source,
    today: Day,
) -> Result<Kept, Error> {
    let feed = source.feed();
    let body = fetch
        .get(&feed.request(today))
        .await
        .map_err(Error::Fetch)?;
    let item = feed.parse(&body, today)?;
    if let Some(kept) = cache.latest(source)
        && kept.url == item.picture
    {
        return Ok(kept);
    }
    let bytes = fetch.get(&item.picture).await.map_err(Error::Fetch)?;
    let extension = picture_extension(&bytes).ok_or(Error::NotAnImage)?;
    cache
        .keep(source, &item, &bytes, extension)
        .map_err(Error::Cache)
}

/// The last fetch of a source: started on `day`, at `at` seconds of the local clock, and
/// whether it brought that day's picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Attempt {
    pub(crate) day: Day,
    pub(crate) at: i64,
    pub(crate) got_today: bool,
}

impl Attempt {
    /// Whether a source last tried as `last` is fetched again at `now`, seconds of the local
    /// clock: once a day, shortly after local midnight, and again after [`RETRY`] while
    /// the day's picture isn't in. A wake from suspend and a session's start forget the
    /// last attempt, so they fetch too.
    pub(crate) fn due(last: Option<&Attempt>, now: i64) -> bool {
        match last {
            None => true,
            Some(last) => {
                last.day != Day::of_local(now) || (!last.got_today && now - last.at >= RETRY)
            }
        }
    }
}

#[cfg(test)]
mod tests;
