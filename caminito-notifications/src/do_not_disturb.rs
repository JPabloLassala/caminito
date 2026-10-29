//! Do Not Disturb (CMN-263): while it's on, notifications get no bubble and no sound but
//! still enter the Notification history. The user turns it on for one of the [`Until`]
//! durations, and it's kept in a file under `$XDG_STATE_HOME/caminito/` so that a restart of
//! the daemon keeps it. It takes the time as an argument, so tests don't wait for an hour.
//! The schedule and the fullscreen and screencast triggers are CMN-264's, as more reasons.

use std::path::Path;

use gio::glib;

use crate::DOMAIN;
use crate::history::write;

/// When the user's Do Not Disturb ends, as the private API's `SetDoNotDisturb` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Until {
    OneHour,
    FourHours,
    /// [`MORNING`] of the next day, the day turning at [`DAY_TURNS`].
    TomorrowMorning,
    TurnedOff,
}

/// The hour "tomorrow morning" ends at, in local time.
const MORNING: i32 = 8;
/// The hour a day turns into the next for "tomorrow morning": at 01:00 the morning is that
/// same day's, which is tomorrow's to someone who hasn't slept yet.
const DAY_TURNS: i32 = 4;

impl Until {
    /// The duration whose nick is `nick`: `one-hour`, `four-hours`, `tomorrow-morning` or
    /// `until-turned-off`.
    fn of_nick(nick: &str) -> Option<Until> {
        match nick {
            "one-hour" => Some(Until::OneHour),
            "four-hours" => Some(Until::FourHours),
            "tomorrow-morning" => Some(Until::TomorrowMorning),
            "until-turned-off" => Some(Until::TurnedOff),
            _ => None,
        }
    }

    /// When it ends, set at `now` in `zone`, in seconds since the epoch, or `None` for never.
    pub(crate) fn end(self, now: u64, zone: &glib::TimeZone) -> Option<u64> {
        match self {
            Until::OneHour => Some(now + 60 * 60),
            Until::FourHours => Some(now + 4 * 60 * 60),
            Until::TomorrowMorning => Some(morning_after(now, zone)),
            Until::TurnedOff => None,
        }
    }
}

/// [`MORNING`] of the day after the one `now` is in, in `zone`, a day turning at
/// [`DAY_TURNS`].
fn morning_after(now: u64, zone: &glib::TimeZone) -> u64 {
    let morning = (|| {
        let local = glib::DateTime::from_unix_local(i64::try_from(now).ok()?)
            .ok()?
            .to_timezone(zone)
            .ok()?;
        let day = local.add_hours(-DAY_TURNS).ok()?.add_days(1).ok()?;
        let morning = glib::DateTime::new(
            zone,
            day.year(),
            day.month(),
            day.day_of_month(),
            MORNING,
            0,
            0.0,
        )
        .ok()?;
        u64::try_from(morning.to_unix()).ok()
    })();
    // A time GLib can't place is a day away.
    morning.unwrap_or(now + 24 * 60 * 60)
}

/// The user's Do Not Disturb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DoNotDisturb {
    Off,
    /// On until `end`, in seconds since the epoch, or until it's turned off.
    On {
        end: Option<u64>,
    },
}

impl DoNotDisturb {
    /// What `SetDoNotDisturb`'s `nick` sets at `now` in `zone`: `off`, or on for one of the
    /// [`Until`] durations. `None` for a nick that's neither.
    pub(crate) fn of_nick(nick: &str, now: u64, zone: &glib::TimeZone) -> Option<DoNotDisturb> {
        if nick == "off" {
            return Some(DoNotDisturb::Off);
        }
        let until = Until::of_nick(nick)?;
        Some(DoNotDisturb::On {
            end: until.end(now, zone),
        })
    }

    /// What `path` holds at `now`: off if it's missing, can't be read, or has ended.
    pub(crate) fn load(path: &Path, now: u64) -> DoNotDisturb {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return DoNotDisturb::Off,
            Err(err) => {
                glib::g_warning!(DOMAIN, "can't read {}: {err}", path.display());
                return DoNotDisturb::Off;
            }
        };
        let Some(read) = decode(&text) else {
            glib::g_warning!(DOMAIN, "can't read {}", path.display());
            return DoNotDisturb::Off;
        };
        if read.ended(now) {
            DoNotDisturb::Off
        } else {
            read
        }
    }

    /// Keeps it in `path`, which "off" is the absence of.
    pub(crate) fn save(self, path: &Path) {
        let written = match self {
            DoNotDisturb::Off => match std::fs::remove_file(path) {
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                removed => removed,
            },
            DoNotDisturb::On { end } => write(path, encode(end).as_bytes()),
        };
        if let Err(err) = written {
            glib::g_warning!(DOMAIN, "can't write {}: {err}", path.display());
        }
    }

    /// Whether it's on with an end that `now` has reached.
    pub(crate) fn ended(self, now: u64) -> bool {
        matches!(self, DoNotDisturb::On { end: Some(end) } if now >= end)
    }

    /// Whether it's on.
    pub(crate) fn is_on(self) -> bool {
        matches!(self, DoNotDisturb::On { .. })
    }

    /// What the private API gives of it: whether it's on, its end or 0 for none, and why
    /// it's on, or "" while it's off.
    pub(crate) fn to_wire(self) -> (bool, u64, &'static str) {
        match self {
            DoNotDisturb::Off => (false, 0, ""),
            DoNotDisturb::On { end } => (true, end.unwrap_or(0), "manual"),
        }
    }
}

/// The file's text: `on`, then a space and the end for one that has one.
fn encode(end: Option<u64>) -> String {
    match end {
        Some(end) => format!("on {end}\n"),
        None => "on\n".to_owned(),
    }
}

fn decode(text: &str) -> Option<DoNotDisturb> {
    let mut words = text.split_whitespace();
    if words.next()? != "on" {
        return None;
    }
    let end = match words.next() {
        Some(end) => Some(end.parse().ok()?),
        None => None,
    };
    words.next().is_none().then_some(DoNotDisturb::On { end })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 13:46:40 on 12 January 1970, in UTC.
    const NOW: u64 = 1_000_000;

    fn utc_time(day: i32, hour: i32, minute: i32) -> u64 {
        let time = glib::DateTime::from_utc(1970, 1, day, hour, minute, 0.0).unwrap();
        u64::try_from(time.to_unix()).unwrap()
    }

    #[test]
    fn tomorrow_morning_is_eight_of_the_next_day_once_the_night_is_over() {
        let utc = glib::TimeZone::utc();
        let end = |day, hour, minute| {
            Until::TomorrowMorning
                .end(utc_time(day, hour, minute), &utc)
                .unwrap()
        };
        assert_eq!(
            Until::TomorrowMorning.end(NOW, &utc),
            Some(utc_time(13, 8, 0))
        );
        assert_eq!(end(12, 23, 30), utc_time(13, 8, 0));
        // Before the day turns, the morning is that day's.
        assert_eq!(end(13, 1, 0), utc_time(13, 8, 0));
        assert_eq!(end(13, 4, 0), utc_time(14, 8, 0));
        assert_eq!(end(13, 7, 59), utc_time(14, 8, 0));
    }

    #[test]
    fn tomorrow_morning_is_the_zones_own() {
        let three_hours_west = glib::TimeZone::from_offset(-3 * 60 * 60);
        // 13:46:40 in UTC is 10:46:40 there, so the morning is 08:00 there on the 13th.
        assert_eq!(
            Until::TomorrowMorning.end(NOW, &three_hours_west),
            Some(utc_time(13, 11, 0))
        );
    }

    #[test]
    fn the_file_holds_whether_it_is_on_and_its_end() {
        assert_eq!(
            decode(&encode(Some(42))),
            Some(DoNotDisturb::On { end: Some(42) })
        );
        assert_eq!(decode(&encode(None)), Some(DoNotDisturb::On { end: None }));
        assert_eq!(decode("on soon"), None);
        assert_eq!(decode("off"), None);
        assert_eq!(decode("on 1 2"), None);
    }
}
