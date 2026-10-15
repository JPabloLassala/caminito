//! The fixtures are made here, byte by byte: a binary property list, its XMP, and a HEIF
//! container around it with no pictures in it, as much of the format as [`xmp`] reads. None
//! is Apple's.

use std::io::Cursor as Io;

use super::*;

/// A value to write as a binary property list.
#[derive(Clone)]
enum Value {
    Int(i64),
    Real(f64),
    Str(&'static str),
    Array(Vec<Value>),
    Dict(Vec<(&'static str, Value)>),
}

/// `value` as a `bplist00`, each object written once in the order met, references a byte.
fn bplist(value: &Value) -> Vec<u8> {
    fn write(value: &Value, objects: &mut Vec<Vec<u8>>) -> u8 {
        let index = objects.len();
        objects.push(Vec::new());
        let with_length = |marker: u8, length: usize| -> Vec<u8> {
            if length < 15 {
                vec![marker | length as u8]
            } else {
                vec![marker | 15, 0x10, length as u8]
            }
        };
        let bytes = match value {
            Value::Int(n) => {
                let mut bytes = vec![0x13];
                bytes.extend_from_slice(&n.to_be_bytes());
                bytes
            }
            Value::Real(n) => {
                let mut bytes = vec![0x23];
                bytes.extend_from_slice(&n.to_be_bytes());
                bytes
            }
            Value::Str(text) => {
                let mut bytes = with_length(0x50, text.len());
                bytes.extend_from_slice(text.as_bytes());
                bytes
            }
            Value::Array(values) => {
                let references: Vec<u8> = values.iter().map(|v| write(v, objects)).collect();
                let mut bytes = with_length(0xA0, values.len());
                bytes.extend(references);
                bytes
            }
            Value::Dict(entries) => {
                let keys: Vec<u8> = entries
                    .iter()
                    .map(|(key, _)| write(&Value::Str(key), objects))
                    .collect();
                let values: Vec<u8> = entries.iter().map(|(_, v)| write(v, objects)).collect();
                let mut bytes = with_length(0xD0, entries.len());
                bytes.extend(keys);
                bytes.extend(values);
                bytes
            }
        };
        objects[index] = bytes;
        index as u8
    }
    let mut objects = Vec::new();
    write(value, &mut objects);
    let mut out = b"bplist00".to_vec();
    let mut offsets = Vec::new();
    for object in &objects {
        offsets.push(out.len() as u16);
        out.extend(object);
    }
    let table = out.len() as u64;
    for offset in offsets {
        out.extend(offset.to_be_bytes());
    }
    out.extend([0; 6]);
    out.extend([2, 1]);
    out.extend((objects.len() as u64).to_be_bytes());
    out.extend(0u64.to_be_bytes());
    out.extend(table.to_be_bytes());
    out
}

fn encode64(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::new();
    for chunk in bytes.chunks(3) {
        let mut three = [0u8; 3];
        three[..chunk.len()].copy_from_slice(chunk);
        let n = u32::from_be_bytes([0, three[0], three[1], three[2]]);
        for digit in 0..=chunk.len() {
            text.push(DIGITS[(n >> (18 - 6 * digit) & 63) as usize] as char);
        }
    }
    while !text.len().is_multiple_of(4) {
        text.push('=');
    }
    text
}

/// An XMP packet with `apple_desktop:<name>` as an attribute, as Apple writes it.
fn xmp_attribute(name: &str, plist: &Value) -> String {
    format!(
        r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Caminito's tests">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" xmlns:apple_desktop="http://ns.apple.com/namespace/1.0/" apple_desktop:{name}="{}"/>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#,
        encode64(&bplist(plist))
    )
}

/// The same, as an element's text, broken over lines.
fn xmp_element(name: &str, plist: &Value) -> String {
    let encoded = encode64(&bplist(plist));
    let lines: Vec<&str> = encoded
        .as_bytes()
        .chunks(40)
        .map(|chunk| std::str::from_utf8(chunk).unwrap())
        .collect();
    format!(
        r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:apple_desktop="http://ns.apple.com/namespace/1.0/">
<apple_desktop:{name}>
{}
</apple_desktop:{name}>
</rdf:Description></rdf:RDF></x:xmpmeta>"#,
        lines.join("\n")
    )
}

fn boxed(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    out.extend(kind);
    out.extend(body);
    out
}

fn full(kind: &[u8; 4], version: u8, body: &[u8]) -> Vec<u8> {
    let mut content = vec![version, 0, 0, 0];
    content.extend(body);
    boxed(kind, &content)
}

/// An item: its ID, its type, and its content type for a `mime`.
fn infe(id: u16, kind: &[u8; 4], content_type: Option<&str>) -> Vec<u8> {
    let mut body = id.to_be_bytes().to_vec();
    body.extend([0, 0]);
    body.extend(kind);
    body.push(0);
    if let Some(content_type) = content_type {
        body.extend(content_type.as_bytes());
        body.push(0);
    }
    full(b"infe", 2, &body)
}

/// Where the XMP goes.
#[derive(Clone, Copy)]
enum Placement {
    /// In an `mdat` after `meta`, by `iloc` version 0 and 4-byte offsets.
    Mdat,
    /// In `idat`, by `iloc` version 1's construction method 1.
    Idat,
}

/// A HEIF file: an `ftyp`, a `meta` with a picture item and an XMP item, the XMP itself.
fn heif(xmp: &str, placement: Placement) -> Vec<u8> {
    let ftyp = boxed(b"ftyp", b"heic\0\0\0\0mif1heic");
    let mut items = 2u16.to_be_bytes().to_vec();
    items.extend(infe(1, b"hvc1", None));
    items.extend(infe(2, b"mime", Some("application/rdf+xml")));
    let iinf = full(b"iinf", 0, &items);
    let xmp = xmp.as_bytes();
    let meta = |iloc: Vec<u8>, idat: &[u8]| {
        let mut body = full(b"hdlr", 0, b"\0\0\0\0pict\0\0\0\0\0\0\0\0\0\0\0\0\0");
        body.extend(full(b"pitm", 0, &1u16.to_be_bytes()));
        body.extend(&iinf);
        body.extend(iloc);
        if !idat.is_empty() {
            body.extend(boxed(b"idat", idat));
        }
        full(b"meta", 0, &body)
    };
    match placement {
        Placement::Mdat => {
            // Two passes: the offset depends on meta's length, which doesn't on the offset.
            let iloc = |offset: u32| {
                let mut body = vec![0x44, 0x00];
                body.extend(1u16.to_be_bytes());
                body.extend(2u16.to_be_bytes());
                body.extend(0u16.to_be_bytes());
                body.extend(1u16.to_be_bytes());
                body.extend(offset.to_be_bytes());
                body.extend((xmp.len() as u32).to_be_bytes());
                full(b"iloc", 0, &body)
            };
            let offset = ftyp.len() + meta(iloc(0), &[]).len() + 8;
            let mut file = ftyp.clone();
            file.extend(meta(iloc(offset as u32), &[]));
            file.extend(boxed(b"mdat", xmp));
            file
        }
        Placement::Idat => {
            let mut body = vec![0x44, 0x00];
            body.extend(1u16.to_be_bytes());
            body.extend(2u16.to_be_bytes());
            body.extend(1u16.to_be_bytes()); // construction method 1
            body.extend(0u16.to_be_bytes());
            body.extend(1u16.to_be_bytes());
            body.extend(3u32.to_be_bytes());
            body.extend((xmp.len() as u32).to_be_bytes());
            let mut idat = b"pad".to_vec();
            idat.extend(xmp);
            let mut file = ftyp.clone();
            file.extend(meta(full(b"iloc", 1, &body), &idat));
            file
        }
    }
}

fn read(bytes: Vec<u8>) -> Result<Schedule, Error> {
    Schedule::from_xmp(&xmp(&mut Io::new(bytes))?)
}

/// A made-up day at a northern latitude: seven pictures round the sky.
fn solar_day() -> Value {
    let position = |a: f64, z: f64, i: i64| {
        Value::Dict(vec![
            ("a", Value::Real(a)),
            ("z", Value::Real(z)),
            ("i", Value::Int(i)),
            ("o", Value::Int(1)),
        ])
    };
    Value::Dict(vec![
        (
            "si",
            Value::Array(vec![
                position(-20.0, 60.0, 0),
                position(0.0, 90.0, 1),
                position(30.0, 120.0, 2),
                position(60.0, 180.5, 3),
                position(30.0, 240.0, 4),
                position(0.0, 270.0, 5),
                position(-20.0, 300.0, 6),
            ]),
        ),
        (
            "ap",
            Value::Dict(vec![("d", Value::Int(6)), ("l", Value::Int(3))]),
        ),
    ])
}

fn solar() -> Schedule {
    read(heif(&xmp_attribute("solar", &solar_day()), Placement::Mdat)).unwrap()
}

fn assert_pick(pick: Pick, from: usize, to: usize, blend: f64) {
    let same = |a: Pick, b: Pick| {
        // A blend of 0 is `from` alone and of 1 `to` alone, whatever the other is.
        let show = |p: Pick| {
            if p.blend < 1e-9 {
                (p.from, p.from, 0.0)
            } else if p.blend > 1.0 - 1e-9 {
                (p.to, p.to, 0.0)
            } else {
                (p.from, p.to, p.blend)
            }
        };
        let (a, b) = (show(a), show(b));
        a.0 == b.0 && a.1 == b.1 && (a.2 - b.2).abs() < 1e-6
    };
    let expected = Pick { from, to, blend };
    assert!(same(pick, expected), "{pick:?}, not {expected:?}");
}

#[test]
fn a_solar_xmp_in_an_mdat_reads_as_its_positions_and_appearance() {
    let Schedule::Solar {
        positions,
        appearance,
    } = solar()
    else {
        panic!("not solar");
    };
    assert_eq!(positions.len(), 7);
    assert_eq!(
        positions[2],
        Position {
            elevation: 30.0,
            azimuth: 120.0,
            picture: 2
        }
    );
    assert_eq!(appearance, Appearance { light: 3, dark: 6 });
    assert_eq!(solar().pictures(), vec![0, 1, 2, 3, 4, 5, 6]);
    assert!(solar().follows_sun());
}

#[test]
fn the_xmp_is_found_in_idat_and_as_an_element() {
    let schedule = read(heif(&xmp_element("solar", &solar_day()), Placement::Idat)).unwrap();
    assert_eq!(schedule, solar());
}

#[test]
fn without_ap_the_highest_position_is_light_and_the_lowest_dark() {
    let Value::Dict(mut entries) = solar_day() else {
        unreachable!()
    };
    entries.retain(|(key, _)| *key != "ap");
    let schedule = read(heif(
        &xmp_attribute("solar", &Value::Dict(entries)),
        Placement::Mdat,
    ))
    .unwrap();
    assert_eq!(schedule.appearance(), Appearance { light: 3, dark: 0 });
}

#[test]
fn the_sun_inside_a_half_of_the_sky_cross_fades_by_elevation() {
    let solar = solar();
    let at = |elevation, azimuth| solar.at(Some((elevation, azimuth)), 0, 0.0);
    // Morning, between the dawn's picture and the morning's.
    assert_pick(at(0.0, 90.0), 1, 2, 0.0);
    assert_pick(at(15.0, 100.0), 1, 2, 0.5);
    assert_pick(at(-10.0, 75.0), 0, 1, 0.5);
    // Afternoon, falling.
    assert_pick(at(15.0, 255.0), 4, 5, 0.5);
    // Above the morning's highest, the band across noon goes by east and west.
    assert_pick(at(45.0, 270.0), 4, 4, 0.0);
    assert_pick(at(30.0, 240.0), 4, 5, 0.0);
    assert_pick(at(-10.0, 285.0), 5, 6, 0.5);
}

#[test]
fn across_noon_and_midnight_it_goes_by_east_and_west() {
    let solar = solar();
    let at = |elevation, azimuth| solar.at(Some((elevation, azimuth)), 0, 0.0);
    // Above the morning's highest picture, on the way to noon's: due south is 0 east, and
    // noon's picture, at 180.5°, a hair west of it.
    let noon = at(62.0, 180.0);
    assert!(
        noon.from == 2 && noon.to == 3 && noon.blend > 0.98,
        "{noon:?}"
    );
    let late_morning = at(45.0, 150.0);
    assert!(
        late_morning.from == 2 && late_morning.to == 3 && (0.3..0.6).contains(&late_morning.blend),
        "{late_morning:?}"
    );
    // Just either side of due south the same.
    let (before, after) = (at(62.0, 179.9), at(62.0, 180.1));
    assert!(
        (before.blend - after.blend).abs() < 0.01,
        "{before:?} {after:?}"
    );
    // Below the lowest: half and half at due north, either side.
    assert_pick(at(-40.0, 0.0), 6, 0, 0.5);
    let (before, after) = (at(-40.0, 359.9), at(-40.0, 0.1));
    assert!(
        (before.blend - after.blend).abs() < 0.01,
        "{before:?} {after:?}"
    );
}

#[test]
fn in_the_southern_hemisphere_noon_is_still_noon_and_midnight_midnight() {
    let solar = solar();
    let at = |elevation, azimuth| solar.at(Some((elevation, azimuth)), 0, 0.0);
    // The sun rises in the east, passes north and sets in the west.
    assert_pick(at(15.0, 70.0), 1, 2, 0.5);
    assert_eq!(at(62.0, 0.0).nearer(), 3);
    assert_eq!(at(62.0, 359.0).nearer(), 3);
    assert_pick(at(15.0, 290.0), 4, 5, 0.5);
    // And it's lowest due south.
    assert_pick(at(-40.0, 180.0), 6, 0, 0.5);
}

#[test]
fn through_a_day_the_picture_never_jumps_more_than_the_fade_hides() {
    // A sun going round at 15° an hour, peaking at 50° due south, at −30° due north.
    let solar = solar();
    let mut previous: Option<Pick> = None;
    for minute in 0..(24 * 60) {
        let hours = f64::from(minute) / 60.0;
        let azimuth = (hours * 15.0).rem_euclid(360.0);
        let elevation = 10.0 - 40.0 * (azimuth.to_radians()).cos();
        let pick = solar.at(Some((elevation, azimuth)), 0, 0.0);
        // What shows, as a weight for each picture.
        let weights = |p: Pick| {
            let mut w = [0.0; 7];
            w[p.from] += 1.0 - p.blend;
            w[p.to] += p.blend;
            w
        };
        if let Some(previous) = previous {
            let (a, b) = (weights(previous), weights(pick));
            let change: f64 = a.iter().zip(b).map(|(a, b)| (a - b).abs()).sum();
            assert!(
                change < 0.35,
                "at {hours:.2} h, from {previous:?} to {pick:?}"
            );
        }
        previous = Some(pick);
    }
}

#[test]
fn without_the_sun_a_solar_heic_is_its_light_and_dark_by_darkness() {
    assert_pick(solar().at(None, 0, 0.0), 3, 6, 0.0);
    assert_pick(solar().at(None, 0, 1.0), 3, 6, 1.0);
    assert_pick(solar().at(None, 0, 0.25), 3, 6, 0.25);
}

#[test]
fn an_h24_follows_the_clock_cross_fading_to_each_next_time() {
    let time = |t: f64, i: i64| Value::Dict(vec![("t", Value::Real(t)), ("i", Value::Int(i))]);
    // Given out of order: 06:00, 12:00, 18:00 and 00:00.
    let plist = Value::Dict(vec![(
        "ti",
        Value::Array(vec![
            time(0.5, 1),
            time(0.25, 0),
            time(0.75, 2),
            time(0.0, 3),
        ]),
    )]);
    let schedule = read(heif(&xmp_attribute("h24", &plist), Placement::Mdat)).unwrap();
    assert!(!schedule.follows_sun());
    let at =
        |hour: i64, minute: i64| schedule.at(Some((50.0, 180.0)), hour * 3600 + minute * 60, 0.0);
    assert_pick(at(6, 0), 0, 1, 0.0);
    assert_pick(at(9, 0), 0, 1, 0.5);
    assert_pick(at(12, 0), 1, 2, 0.0);
    assert_pick(at(21, 0), 2, 3, 0.5);
    assert_pick(at(3, 0), 3, 0, 0.5);
    // Its light is noon's and its dark midnight's, without an `ap`.
    assert_eq!(schedule.appearance(), Appearance { light: 1, dark: 3 });
}

#[test]
fn an_apr_is_a_light_and_a_dark_picture() {
    let plist = Value::Dict(vec![("l", Value::Int(0)), ("d", Value::Int(1))]);
    let schedule = read(heif(&xmp_attribute("apr", &plist), Placement::Mdat)).unwrap();
    assert_eq!(
        schedule,
        Schedule::Appearance(Appearance { light: 0, dark: 1 })
    );
    assert_pick(schedule.at(Some((0.0, 90.0)), 0, 0.5), 0, 1, 0.5);
}

#[test]
fn a_heic_without_apple_desktop_isnt_dynamic() {
    let xmp = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/" dc:format="image/heic"/></rdf:RDF></x:xmpmeta>"#;
    assert_eq!(read(heif(xmp, Placement::Mdat)), Err(Error::NoSchedule));
    // And one with no XMP at all: a photo.
    let mut photo = boxed(b"ftyp", b"heic\0\0\0\0mif1heic");
    let mut items = 1u16.to_be_bytes().to_vec();
    items.extend(infe(1, b"hvc1", None));
    photo.extend(full(b"meta", 0, &full(b"iinf", 0, &items)));
    assert_eq!(read(photo), Err(Error::NoSchedule));
}

#[test]
fn broken_files_are_refused_and_never_panic() {
    let good = heif(&xmp_attribute("solar", &solar_day()), Placement::Mdat);
    assert!(matches!(
        read(b"not a heic at all".to_vec()),
        Err(Error::NotHeif(_))
    ));
    assert!(matches!(read(Vec::new()), Err(Error::NotHeif(_))));
    // Cut short anywhere, it's an error.
    for length in 0..good.len() {
        assert!(read(good[..length].to_vec()).is_err(), "cut at {length}");
    }
    // A byte changed anywhere, it reads or it's an error.
    for at in 0..good.len() {
        for byte in [0x00, 0x01, 0x7f, 0xff] {
            let mut bad = good.clone();
            bad[at] = byte;
            let _ = read(bad);
        }
    }
}

#[test]
fn broken_metadata_is_refused() {
    let with = |name: &str, value: &Value| Schedule::from_xmp(&xmp_attribute(name, value));
    let bad = |result: Result<Schedule, Error>| {
        assert!(matches!(result, Err(Error::BadSchedule(_))), "{result:?}")
    };
    bad(with("solar", &Value::Dict(vec![])));
    bad(with(
        "solar",
        &Value::Dict(vec![("si", Value::Array(vec![]))]),
    ));
    bad(with(
        "solar",
        &Value::Dict(vec![(
            "si",
            Value::Array(vec![Value::Dict(vec![("a", Value::Real(10.0))])]),
        )]),
    ));
    bad(with(
        "solar",
        &Value::Dict(vec![(
            "si",
            Value::Array(vec![Value::Dict(vec![
                ("a", Value::Real(100.0)),
                ("z", Value::Real(0.0)),
                ("i", Value::Int(0)),
            ])]),
        )]),
    ));
    bad(with(
        "h24",
        &Value::Dict(vec![(
            "ti",
            Value::Array(vec![Value::Dict(vec![
                ("t", Value::Real(0.5)),
                ("i", Value::Int(-1)),
            ])]),
        )]),
    ));
    bad(with("apr", &Value::Dict(vec![("l", Value::Str("zero"))])));
    bad(with("apr", &Value::Int(3)));
    // Not base64, and base64 of something that isn't a property list.
    let packet = |value: &str| {
        format!(r#"<rdf:Description xmlns:apple_desktop="x" apple_desktop:solar="{value}"/>"#)
    };
    bad(Schedule::from_xmp(&packet("!!!")));
    bad(Schedule::from_xmp(&packet(&encode64(
        b"bplist00 but nothing after it, not long enough at all",
    ))));
    // Each byte of a good property list cut or changed: an error or a schedule, no panic.
    let plist = bplist(&solar_day());
    for length in 0..plist.len() {
        let _ = Schedule::from_xmp(&packet(&encode64(&plist[..length])));
    }
    for at in 0..plist.len() {
        for byte in [0x00, 0x0f, 0x1f, 0xaf, 0xdf, 0xff] {
            let mut changed = plist.clone();
            changed[at] = byte;
            let _ = Schedule::from_xmp(&packet(&encode64(&changed)));
        }
    }
}

#[test]
fn a_property_list_that_contains_itself_is_refused() {
    // An array whose one element is itself.
    let mut plist = b"bplist00".to_vec();
    plist.extend([0xA1, 0x00]);
    let table = plist.len() as u64;
    plist.extend([8]);
    plist.extend([0; 6]);
    plist.extend([1, 1]);
    plist.extend(1u64.to_be_bytes());
    plist.extend(0u64.to_be_bytes());
    plist.extend(table.to_be_bytes());
    assert!(Plist::parse(&plist).is_err());
}

#[test]
fn base64_decodes_with_and_without_padding() {
    assert_eq!(base64("TWFu").unwrap(), b"Man");
    assert_eq!(base64("TWE=").unwrap(), b"Ma");
    assert_eq!(base64("TQ==").unwrap(), b"M");
    assert_eq!(base64("TW\nFu\n").unwrap(), b"Man");
    assert_eq!(base64("T"), None);
    assert_eq!(base64("T*Fu"), None);
}

#[test]
fn a_heic_is_read_with_its_pictures_in_the_cache_named_for_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Made Up.heic");
    fs::write(
        &file,
        heif(&xmp_attribute("solar", &solar_day()), Placement::Mdat),
    )
    .unwrap();
    let cache = dir.path().join("cache");
    let heic = Heic::read_in(&file, &cache).unwrap();
    assert_eq!(heic.schedule(), &solar());
    let picture = heic.picture(0);
    assert!(picture.starts_with(&cache));
    assert!(picture.ends_with("picture-1.jpg"), "{}", picture.display());
    assert!(
        picture
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("Made Up-")
    );
    assert!(!heic.decoded());
    for index in heic.schedule().pictures() {
        let picture = heic.picture(index);
        fs::create_dir_all(picture.parent().unwrap()).unwrap();
        fs::write(picture, b"").unwrap();
    }
    assert!(heic.decoded());
    // The same file read again is decoded to the same place; a changed one elsewhere.
    assert_eq!(
        Heic::read_in(&file, &cache).unwrap().picture(0),
        heic.picture(0)
    );
    let mut longer = fs::read(&file).unwrap();
    longer.extend(boxed(b"free", b""));
    fs::write(&file, longer).unwrap();
    assert_ne!(
        Heic::read_in(&file, &cache).unwrap().picture(0),
        heic.picture(0)
    );
}

#[test]
fn heic_files_are_known_by_their_extension() {
    assert!(is_heic(Path::new("/a/Mojave.heic")));
    assert!(is_heic(Path::new("/a/b.HEIF")));
    assert!(!is_heic(Path::new("/a/b.xml")));
    assert!(!is_heic(Path::new("/a/heic")));
}

/// A JPEG of `colour`, with `xmp` in it as an APP1 segment, as a camera writes it.
fn jpeg_with_xmp(path: &Path, colour: u32, xmp: &str) {
    let pixbuf = gdk_pixbuf::Pixbuf::new(gdk_pixbuf::Colorspace::Rgb, false, 8, 64, 48).unwrap();
    pixbuf.fill(colour << 8);
    let jpeg = pixbuf
        .save_to_bufferv("jpeg", &[("quality", "95")])
        .unwrap();
    let mut segment = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
    segment.extend(xmp.as_bytes());
    let mut out = jpeg[..2].to_vec();
    out.extend([0xFF, 0xE1]);
    out.extend(((segment.len() + 2) as u16).to_be_bytes());
    out.extend(segment);
    out.extend(&jpeg[2..]);
    fs::write(path, out).unwrap();
}

/// Runs a thread-default main context until `done` is set.
fn wait(context: &glib::MainContext, done: &std::cell::Cell<bool>) {
    let start = std::time::Instant::now();
    while !done.get() {
        assert!(start.elapsed().as_secs() < 60, "the decoder took too long");
        context.iteration(true);
    }
}

#[test]
fn a_heic_made_by_libheif_is_read_and_decoded_or_greyed_out_without_it() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let context = glib::MainContext::new();
    let Some(encoder) = glib::find_program_in_path("heif-enc").filter(|_| decoder().is_some())
    else {
        // Without libheif's programs, a decode says why at once.
        let heic = Heic {
            file: dir.path().join("any.heic"),
            folder: cache.join("any"),
            schedule: Schedule::Appearance(Appearance { light: 0, dark: 1 }),
        };
        if decoder().is_none() {
            let outcome = std::rc::Rc::new(std::cell::RefCell::new(None));
            let kept = outcome.clone();
            decode(&heic, move |result| *kept.borrow_mut() = Some(result));
            assert_eq!(*outcome.borrow(), Some(Err(NO_DECODER.to_owned())));
        }
        eprintln!("libheif's heif-enc or heif-dec isn't installed: no HEIC is made");
        return;
    };
    // Three pictures, red, green and blue, the first with an apr naming green light and
    // blue dark, as heif-enc carries a JPEG's XMP into the HEIC.
    let apr = Value::Dict(vec![("l", Value::Int(1)), ("d", Value::Int(2))]);
    let colours = [0xC82828, 0x28C828, 0x2828C8];
    let mut inputs = Vec::new();
    for (index, colour) in colours.into_iter().enumerate() {
        let input = dir.path().join(format!("{index}.jpg"));
        jpeg_with_xmp(&input, colour, &xmp_attribute("apr", &apr));
        inputs.push(input);
    }
    let file = dir.path().join("made.heic");
    let status = std::process::Command::new(encoder)
        .args(["-q", "90"])
        .args(&inputs)
        .arg("-o")
        .arg(&file)
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());

    let heic = Heic::read_in(&file, &cache).unwrap();
    assert_eq!(
        heic.schedule(),
        &Schedule::Appearance(Appearance { light: 1, dark: 2 })
    );
    assert!(!heic.decoded());
    let done = std::rc::Rc::new(std::cell::Cell::new(false));
    context
        .with_thread_default(|| {
            let finished = done.clone();
            decode(&heic, move |result| {
                assert_eq!(result, Ok(()));
                finished.set(true);
            });
            wait(&context, &done);
        })
        .unwrap();
    assert!(heic.decoded());
    for (index, colour) in colours.into_iter().enumerate() {
        let pixbuf = gdk_pixbuf::Pixbuf::from_file(heic.picture(index)).unwrap();
        let pixels = pixbuf.read_pixel_bytes();
        let expected = colour.to_be_bytes();
        for channel in 0..3 {
            let (got, want) = (i32::from(pixels[channel]), i32::from(expected[channel + 1]));
            assert!(
                (got - want).abs() < 16,
                "picture {index}: {:?}",
                &pixels[..3]
            );
        }
    }
    // Nothing is left beside the pictures.
    let entries: Vec<_> = fs::read_dir(&cache).unwrap().flatten().collect();
    assert_eq!(entries.len(), 1);
}

/// A HEIC that libheif 1.17's `heif-enc` made, the container as libheif writes it: three
/// pictures of 64 by 48 of one colour each, `0xc0c0a0` by day, `0x806040` at the horizon and
/// `0x202040` by night, and a made-up `solar` with the sun due north at −30°, east and
/// west at 0° and due south at 60°. Made with Python's `plistlib` and Pillow, and
/// `heif-enc -q 95 day.jpg horizon.jpg night.jpg`, the first JPEG carrying the XMP. The
/// drawer's test (`caminito/src/tests/caminito_wallpaperd.rs`) draws it.
const SOLAR_HEIC: &[u8] = include_bytes!("solar.heic");

#[test]
fn a_heic_as_libheif_writes_it_is_read_without_libheif() {
    let schedule = read(SOLAR_HEIC.to_vec()).unwrap();
    let position = |elevation: f64, azimuth: f64, picture: usize| Position {
        elevation,
        azimuth,
        picture,
    };
    assert_eq!(
        schedule,
        Schedule::Solar {
            positions: vec![
                position(-30.0, 0.0, 2),
                position(0.0, 90.0, 1),
                position(60.0, 180.0, 0),
                position(0.0, 270.0, 1),
            ],
            appearance: Appearance { light: 0, dark: 2 },
        }
    );
    // Day, dusk and night.
    assert_eq!(schedule.at(Some((50.0, 170.0)), 0, 0.0).nearer(), 0);
    assert_eq!(schedule.at(Some((0.0, 270.0)), 0, 0.0).nearer(), 1);
    assert_eq!(schedule.at(Some((-30.0, 350.0)), 0, 0.0).nearer(), 2);
}
