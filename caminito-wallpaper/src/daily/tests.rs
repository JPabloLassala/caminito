use std::cell::RefCell;
use std::collections::HashMap;

use super::*;

/// The saved answers, written as each source serves its page or feed.
const APOD_PAGE: &[u8] = include_bytes!("apod.html");
const APOD_VIDEO: &[u8] = include_bytes!("apod-video.html");
const BING_FEED: &[u8] = include_bytes!("bing.json");
const COMMONS_FEED: &[u8] = include_bytes!("commons.json");

/// The bytes a JPEG starts with, which is all the cache looks at.
const JPEG: &[u8] = b"\xff\xd8\xff\xe0 a JPEG";

fn day(year: i32, month: u8, day: u8) -> Day {
    Day::new(year, month, day).unwrap()
}

fn today() -> Day {
    day(2026, 10, 15)
}

/// A fake of the network: each URL's answer, and the URLs asked for, in order.
#[derive(Default)]
struct Fake {
    answers: HashMap<String, Vec<u8>>,
    asked: RefCell<Vec<String>>,
}

impl Fake {
    fn answering(mut self, url: &str, body: &[u8]) -> Self {
        self.answers.insert(url.to_owned(), body.to_vec());
        self
    }
}

impl Fetch for Fake {
    async fn get(&self, url: &str) -> Result<Vec<u8>, http::Error> {
        self.asked.borrow_mut().push(url.to_owned());
        self.answers
            .get(url)
            .cloned()
            .ok_or(http::Error::Status(404))
    }
}

/// A fake source's feed: Bing's, whose request is the same every day, with `picture` its
/// picture's URL.
fn bing_feed(picture_id: &str, startdate: &str) -> Vec<u8> {
    String::from_utf8_lossy(BING_FEED)
        .replace("LarchesAutumn_EN-US1234567890", picture_id)
        .replace(
            "\"startdate\":\"20261015\"",
            &format!("\"startdate\":\"{startdate}\""),
        )
        .into_bytes()
}

fn bing_picture(picture_id: &str) -> String {
    format!("https://www.bing.com/th?id=OHR.{picture_id}_UHD.jpg")
}

fn run<T>(future: impl Future<Output = T>) -> T {
    gio::glib::MainContext::new().block_on(future)
}

/// The files in the cache, sorted.
fn files(cache: &Cache) -> Vec<String> {
    let mut files: Vec<String> = fs::read_dir(cache.root())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    files
}

#[test]
fn apod_is_read_from_its_page() {
    let item = Source::Apod.feed().parse(APOD_PAGE, today()).unwrap();
    assert_eq!(
        item,
        Item {
            picture: "https://apod.nasa.gov/apod/image/2610/HeartSoul_Smith_4096.jpg".into(),
            title: "The Heart and Soul Nebulae".into(),
            credit: "Image Credit & Copyright: Jane Smith, José Doe".into(),
            day: today(),
        }
    );
    assert_eq!(
        Source::Apod.feed().request(today()),
        "https://apod.nasa.gov/apod/astropix.html"
    );
}

#[test]
fn an_apod_title_with_credit_in_it_isnt_its_credit() {
    let page = String::from_utf8_lossy(APOD_PAGE)
        .replace("The Heart and Soul Nebulae", "Credit to the Comet");
    let item = Source::Apod.feed().parse(page.as_bytes(), today()).unwrap();
    assert_eq!(item.title, "Credit to the Comet");
    assert_eq!(
        item.credit,
        "Image Credit & Copyright: Jane Smith, José Doe"
    );
}

#[test]
fn an_apod_video_is_no_picture() {
    let err = Source::Apod.feed().parse(APOD_VIDEO, today()).unwrap_err();
    assert!(matches!(err, Error::NotAPicture("it's a video")), "{err:?}");
}

#[test]
fn an_apod_page_without_its_title_is_malformed() {
    let err = Source::Apod
        .feed()
        .parse(b"<html><body>Service unavailable</body></html>", today())
        .unwrap_err();
    assert!(matches!(err, Error::Malformed("title")), "{err:?}");
}

#[test]
fn bing_is_read_from_its_feed() {
    let item = Source::Bing
        .feed()
        .parse(BING_FEED, day(2026, 10, 14))
        .unwrap();
    assert_eq!(
        item,
        Item {
            picture: bing_picture("LarchesAutumn_EN-US1234567890"),
            title: "Larches turn to gold".into(),
            credit: "© Jane Photographer/Getty Images".into(),
            day: today(),
        }
    );
}

#[test]
fn a_bing_picture_not_for_wallpaper_is_no_picture() {
    let feed = String::from_utf8_lossy(BING_FEED).replace("\"wp\":true", "\"wp\":false");
    let err = Source::Bing
        .feed()
        .parse(feed.as_bytes(), today())
        .unwrap_err();
    assert!(matches!(err, Error::NotAPicture(_)), "{err:?}");
}

#[test]
fn a_bing_title_of_info_is_its_description() {
    let feed = String::from_utf8_lossy(BING_FEED).replace("Larches turn to gold", "Info");
    let item = Source::Bing.feed().parse(feed.as_bytes(), today()).unwrap();
    assert_eq!(
        item.title,
        "Golden larches above Lake Moraine, Banff National Park, Alberta, Canada"
    );
}

#[test]
fn commons_is_read_from_wikipedias_feed_scaled_to_4k() {
    let item = Source::Commons.feed().parse(COMMONS_FEED, today()).unwrap();
    assert_eq!(
        item,
        Item {
            picture: "https://upload.wikimedia.org/wikipedia/commons/thumb/4/4f/Heron_at_dawn%2C_Lake_Neusiedl%2C_Austria.jpg/3840px-Heron_at_dawn%2C_Lake_Neusiedl%2C_Austria.jpg".into(),
            title: "Grey heron at dawn".into(),
            credit: "Ann Example, CC BY-SA 4.0".into(),
            day: today(),
        }
    );
    assert_eq!(
        Source::Commons.feed().request(day(2026, 3, 7)),
        "https://en.wikipedia.org/api/rest_v1/feed/featured/2026/03/07"
    );
}

#[test]
fn a_commons_picture_without_a_caption_is_titled_by_its_description() {
    let feed =
        String::from_utf8_lossy(COMMONS_FEED).replace(r#""en":"Grey heron at dawn""#, r#""en":"""#);
    let item = Source::Commons
        .feed()
        .parse(feed.as_bytes(), today())
        .unwrap();
    assert_eq!(
        item.title,
        "A grey heron (Ardea cinerea) at dawn on Lake Neusiedl, Austria"
    );
}

#[test]
fn a_small_commons_picture_is_its_original() {
    let feed = String::from_utf8_lossy(COMMONS_FEED).replace("\"width\":6000", "\"width\":2000");
    let item = Source::Commons
        .feed()
        .parse(feed.as_bytes(), today())
        .unwrap();
    assert_eq!(
        item.picture,
        "https://upload.wikimedia.org/wikipedia/commons/4/4f/Heron_at_dawn%2C_Lake_Neusiedl%2C_Austria.jpg"
    );
}

#[test]
fn a_commons_video_or_a_day_without_a_picture_is_no_picture() {
    let video = String::from_utf8_lossy(COMMONS_FEED).replace(
        "Austria.jpg\",\"width\":6000",
        "Austria.webm\",\"width\":6000",
    );
    let err = Source::Commons
        .feed()
        .parse(video.as_bytes(), today())
        .unwrap_err();
    assert!(matches!(err, Error::NotAPicture(_)), "{err:?}");

    let err = Source::Commons
        .feed()
        .parse(br#"{"tfa": {}}"#, today())
        .unwrap_err();
    assert!(matches!(err, Error::NotAPicture(_)), "{err:?}");
}

#[test]
fn a_feed_that_isnt_json_is_malformed() {
    for source in [Source::Bing, Source::Commons] {
        let err = source
            .feed()
            .parse(b"<html>502 Bad Gateway</html>", today())
            .unwrap_err();
        assert!(matches!(err, Error::Malformed("JSON")), "{err:?}");
    }
}

#[test]
fn a_fetch_keeps_the_picture_with_its_title_and_credit() {
    let folder = tempfile::tempdir().unwrap();
    let cache = Cache::new(folder.path());
    let fake = Fake::default()
        .answering(&Source::Apod.feed().request(today()), APOD_PAGE)
        .answering(
            "https://apod.nasa.gov/apod/image/2610/HeartSoul_Smith_4096.jpg",
            JPEG,
        );

    let kept = run(update(&fake, &cache, Source::Apod, today())).unwrap();

    assert_eq!(fs::read(&kept.picture).unwrap(), JPEG);
    assert_eq!(kept.title, "The Heart and Soul Nebulae");
    assert_eq!(
        kept.credit,
        "Image Credit & Copyright: Jane Smith, José Doe"
    );
    assert_eq!(cache.latest(Source::Apod), Some(kept));
    assert_eq!(cache.latest(Source::Bing), None);
}

#[test]
fn a_failed_fetch_keeps_the_last_picture() {
    let folder = tempfile::tempdir().unwrap();
    let cache = Cache::new(folder.path());
    let yesterday = Fake::default()
        .answering(
            &Source::Bing.feed().request(today()),
            &bing_feed("Old", "20261014"),
        )
        .answering(&bing_picture("Old"), JPEG);
    let kept = run(update(&yesterday, &cache, Source::Bing, day(2026, 10, 14))).unwrap();
    let before = files(&cache);

    // No network: nothing answers.
    let err = run(update(&Fake::default(), &cache, Source::Bing, today())).unwrap_err();
    assert!(matches!(err, Error::Fetch(_)), "{err:?}");
    assert_eq!(cache.latest(Source::Bing), Some(kept.clone()));

    // The feed answers, and the picture doesn't.
    let no_picture = Fake::default().answering(
        &Source::Bing.feed().request(today()),
        &bing_feed("New", "20261015"),
    );
    let err = run(update(&no_picture, &cache, Source::Bing, today())).unwrap_err();
    assert!(matches!(err, Error::Fetch(_)), "{err:?}");
    assert_eq!(cache.latest(Source::Bing), Some(kept.clone()));

    // What comes back for the picture is a page of an error.
    let error_page = no_picture.answering(&bing_picture("New"), b"<html>Forbidden</html>");
    let err = run(update(&error_page, &cache, Source::Bing, today())).unwrap_err();
    assert!(matches!(err, Error::NotAnImage), "{err:?}");
    assert_eq!(cache.latest(Source::Bing), Some(kept));
    assert_eq!(files(&cache), before);
}

#[test]
fn a_day_that_isnt_a_picture_keeps_the_last_one() {
    let folder = tempfile::tempdir().unwrap();
    let cache = Cache::new(folder.path());
    let fake = Fake::default()
        .answering(&Source::Apod.feed().request(today()), APOD_PAGE)
        .answering(
            "https://apod.nasa.gov/apod/image/2610/HeartSoul_Smith_4096.jpg",
            JPEG,
        );
    let kept = run(update(&fake, &cache, Source::Apod, today())).unwrap();

    let tomorrow = day(2026, 10, 16);
    let video = Fake::default().answering(&Source::Apod.feed().request(tomorrow), APOD_VIDEO);
    let err = run(update(&video, &cache, Source::Apod, tomorrow)).unwrap_err();

    assert!(matches!(err, Error::NotAPicture(_)), "{err:?}");
    assert_eq!(cache.latest(Source::Apod), Some(kept));
    // The picture wasn't asked for.
    assert_eq!(video.asked.borrow().len(), 1);
}

#[test]
fn the_cache_holds_one_picture_per_source() {
    let folder = tempfile::tempdir().unwrap();
    let cache = Cache::new(folder.path());
    for (id, startdate, on) in [
        ("First", "20261013", day(2026, 10, 13)),
        ("Second", "20261014", day(2026, 10, 14)),
        ("Third", "20261015", today()),
    ] {
        let fake = Fake::default()
            .answering(&Source::Bing.feed().request(on), &bing_feed(id, startdate))
            .answering(&bing_picture(id), JPEG);
        run(update(&fake, &cache, Source::Bing, on)).unwrap();
    }
    let commons = Fake::default()
        .answering(&Source::Commons.feed().request(today()), COMMONS_FEED)
        .answering(
            &Source::Commons
                .feed()
                .parse(COMMONS_FEED, today())
                .unwrap()
                .picture,
            b"\x89PNG\r\n\x1a\n a PNG",
        );
    let kept_commons = run(update(&commons, &cache, Source::Commons, today())).unwrap();

    let files = files(&cache);
    let pictures: Vec<&String> = files
        .iter()
        .filter(|file| !file.ends_with(".about"))
        .collect();
    assert_eq!(pictures.len(), 2, "{files:?}");
    assert!(pictures[0].starts_with("bing-2026-10-15-") && pictures[0].ends_with(".jpg"));
    assert!(pictures[1].starts_with("commons-2026-10-15-") && pictures[1].ends_with(".png"));
    assert_eq!(
        files.iter().filter(|file| file.ends_with(".about")).count(),
        2
    );
    assert_eq!(
        cache.latest(Source::Bing).unwrap().url,
        bing_picture("Third")
    );
    assert_eq!(cache.latest(Source::Commons), Some(kept_commons));
}

#[test]
fn the_picture_kept_already_isnt_fetched_again() {
    let folder = tempfile::tempdir().unwrap();
    let cache = Cache::new(folder.path());
    let fake = Fake::default()
        .answering(&Source::Bing.feed().request(today()), BING_FEED)
        .answering(&bing_picture("LarchesAutumn_EN-US1234567890"), JPEG);
    let kept = run(update(&fake, &cache, Source::Bing, today())).unwrap();
    fake.asked.borrow_mut().clear();

    assert_eq!(
        run(update(&fake, &cache, Source::Bing, today())).unwrap(),
        kept
    );
    assert_eq!(
        *fake.asked.borrow(),
        vec![Source::Bing.feed().request(today())]
    );
}

#[test]
fn keep_this_picture_copies_it_by_its_day_and_title() {
    let folder = tempfile::tempdir().unwrap();
    let cache = Cache::new(folder.path().join("cache"));
    let fake = Fake::default()
        .answering(&Source::Bing.feed().request(today()), BING_FEED)
        .answering(&bing_picture("LarchesAutumn_EN-US1234567890"), JPEG);
    let mut kept = run(update(&fake, &cache, Source::Bing, today())).unwrap();
    let wallpapers = folder.path().join("Pictures").join("Wallpapers");

    let copy = keep_copy(&kept, &wallpapers).unwrap();
    assert_eq!(copy, wallpapers.join("2026-10-15 Larches turn to gold.jpg"));
    assert_eq!(fs::read(&copy).unwrap(), JPEG);
    // Again, it's the same file.
    assert_eq!(keep_copy(&kept, &wallpapers).unwrap(), copy);

    kept.title = "Up/down".into();
    assert_eq!(
        keep_copy(&kept, &wallpapers).unwrap(),
        wallpapers.join("2026-10-15 Up-down.jpg")
    );
}

#[test]
fn a_source_is_fetched_daily_and_again_within_the_hour_until_todays_is_in() {
    let noon = 20_741 * 86_400 + 12 * 3600;
    assert_eq!(Day::of_local(noon), today());
    assert!(Attempt::due(None, noon));

    let got = Attempt {
        day: today(),
        at: noon,
        got_today: true,
    };
    assert!(!Attempt::due(Some(&got), noon + 6 * 3600));
    assert!(!Attempt::due(Some(&got), noon + 12 * 3600 - 1));
    // Shortly after midnight.
    assert!(Attempt::due(Some(&got), noon + 12 * 3600));

    let missed = Attempt {
        got_today: false,
        ..got
    };
    assert!(!Attempt::due(Some(&missed), noon + RETRY - 1));
    assert!(Attempt::due(Some(&missed), noon + RETRY));
}

#[test]
fn a_day_is_of_the_local_clock() {
    assert_eq!(Day::of_local(0), day(1970, 1, 1));
    assert_eq!(Day::of_local(-1), day(1969, 12, 31));
    assert_eq!(Day::of_local(951_782_400), day(2000, 2, 29));
    assert_eq!(Day::parse("2026-10-15"), Some(today()));
    assert_eq!(today().to_string(), "2026-10-15");
    assert_eq!(Day::parse("2026-13-01"), None);
}

#[test]
fn a_picture_is_known_by_its_first_bytes() {
    assert_eq!(picture_extension(JPEG), Some("jpg"));
    assert_eq!(picture_extension(b"\x89PNG\r\n\x1a\n"), Some("png"));
    assert_eq!(picture_extension(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
    assert_eq!(picture_extension(b"<!doctype html>"), None);
    assert_eq!(picture_extension(b""), None);
}

#[test]
fn a_source_is_known_by_its_nick() {
    for source in Source::ALL {
        assert_eq!(Source::from_nick(source.nick()), source);
    }
    assert_eq!(Source::from_nick("flickr"), Source::default());
}
