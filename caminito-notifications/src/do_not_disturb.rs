//! Do Not Disturb (CMN-263): while it's on, notifications get no bubble and no sound but
//! still enter the Notification history. The user turns it on for one of the [`Until`]
//! durations, and it's kept in a file under `$XDG_STATE_HOME/caminito/` so that a restart of
//! the daemon keeps it. It takes the time as an argument, so tests don't wait for an hour.
//!
//! It's also on (CMN-264) during its daily [`Schedule`], and while Caminito says the output
//! under the pointer has a fullscreen window or a screencast is running, each behind its
//! setting, the [`Triggers`]. [`Sources`] holds them all, and says whether it's on and the
//! first [`Reason`] it's on for. Turning it off by hand holds each automatic reason that's on
//! off: the schedule until its next start, a trigger until Caminito says it ended.

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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum DoNotDisturb {
    #[default]
    Off,
    /// On until `end`, in seconds since the epoch, or until it's turned off.
    On { end: Option<u64> },
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
}

/// Why Do Not Disturb is on, in the order one wins over the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reason {
    /// The user turned it on.
    Manual,
    /// Its daily schedule.
    Schedule,
    /// The output under the pointer has a fullscreen window.
    Fullscreen,
    /// A screencast is running.
    Screencast,
}

impl Reason {
    /// Its name in the private API.
    fn nick(self) -> &'static str {
        match self {
            Reason::Manual => "manual",
            Reason::Schedule => "schedule",
            Reason::Fullscreen => "fullscreen",
            Reason::Screencast => "screencast",
        }
    }
}

/// The daily schedule: one window, the same every day, in local time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Schedule {
    /// When it starts, in minutes from midnight.
    pub(crate) from: u32,
    /// When it ends, in minutes from midnight, the next day's if it's before `from`. A
    /// schedule that ends as it starts is never on.
    pub(crate) to: u32,
}

impl Schedule {
    /// Whether `now` is in it, in `zone`.
    fn contains(self, now: u64, zone: &glib::TimeZone) -> bool {
        let Some(minute) = minute_of_day(now, zone) else {
            return false;
        };
        if self.from <= self.to {
            (self.from..self.to).contains(&minute)
        } else {
            minute >= self.from || minute < self.to
        }
    }
}

/// What the settings turn Do Not Disturb on for, besides the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Triggers {
    /// The schedule, while it's on.
    pub(crate) schedule: Option<Schedule>,
    /// Whether it's on while the output under the pointer has a fullscreen window.
    pub(crate) while_fullscreen: bool,
    /// Whether it's on while a screencast runs.
    pub(crate) while_screencasting: bool,
}

/// Do Not Disturb as the clients see it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Effective {
    #[default]
    Off,
    /// On for `reason`, until `end` in seconds since the epoch, or with no end known.
    On { reason: Reason, end: Option<u64> },
}

impl Effective {
    /// Whether it's on.
    pub(crate) fn is_on(self) -> bool {
        matches!(self, Effective::On { .. })
    }

    /// What the private API gives of it: whether it's on, its end or 0 for none, and why
    /// it's on, or "" while it's off.
    pub(crate) fn to_wire(self) -> (bool, u64, &'static str) {
        match self {
            Effective::Off => (false, 0, ""),
            Effective::On { reason, end } => (true, end.unwrap_or(0), reason.nick()),
        }
    }
}

/// Everything that turns Do Not Disturb on, and what the user's turning it off holds off.
#[derive(Debug, Default)]
pub(crate) struct Sources {
    /// The user's.
    pub(crate) manual: DoNotDisturb,
    /// Whether Caminito says the output under the pointer has a fullscreen window.
    fullscreen: bool,
    /// Whether Caminito says a screencast is running.
    screencasting: bool,
    /// The schedule's next start, until which the user turned the schedule off.
    schedule_held_until: Option<u64>,
    /// Whether the user turned a fullscreen window's Do Not Disturb off, until it ends.
    fullscreen_held: bool,
    /// The same for a screencast's.
    screencast_held: bool,
}

impl Sources {
    /// The user's Do Not Disturb, and nothing else yet.
    pub(crate) fn new(manual: DoNotDisturb) -> Sources {
        Sources {
            manual,
            ..Sources::default()
        }
    }

    /// Caminito says whether the output under the pointer has a fullscreen window. One
    /// that ends ends its hold.
    pub(crate) fn set_fullscreen(&mut self, fullscreen: bool) {
        self.fullscreen = fullscreen;
        self.fullscreen_held &= fullscreen;
    }

    /// Caminito says whether a screencast is running. One that ends ends its hold.
    pub(crate) fn set_screencasting(&mut self, screencasting: bool) {
        self.screencasting = screencasting;
        self.screencast_held &= screencasting;
    }

    /// Whether it's on at `now` in `zone`, under `triggers`, and why.
    pub(crate) fn effective(
        &self,
        triggers: &Triggers,
        now: u64,
        zone: &glib::TimeZone,
    ) -> Effective {
        if let DoNotDisturb::On { end } = self.manual
            && !self.manual.ended(now)
        {
            return Effective::On {
                reason: Reason::Manual,
                end,
            };
        }
        if let Some(schedule) = self.schedule_on(triggers, now, zone) {
            return Effective::On {
                reason: Reason::Schedule,
                end: Some(next_at(now, schedule.to, zone)),
            };
        }
        let (fullscreen, screencast) = self.triggered(triggers);
        if fullscreen && !self.fullscreen_held {
            return Effective::On {
                reason: Reason::Fullscreen,
                end: None,
            };
        }
        if screencast && !self.screencast_held {
            return Effective::On {
                reason: Reason::Screencast,
                end: None,
            };
        }
        Effective::Off
    }

    /// The user turns it off at `now`: their own is, and each automatic reason that's on is
    /// held off, the schedule until its next start and a trigger until it ends.
    pub(crate) fn turn_off(&mut self, triggers: &Triggers, now: u64, zone: &glib::TimeZone) {
        self.manual = DoNotDisturb::Off;
        if let Some(schedule) = self.schedule_on(triggers, now, zone) {
            self.schedule_held_until = Some(next_at(now, schedule.from, zone));
        }
        let (fullscreen, screencast) = self.triggered(triggers);
        self.fullscreen_held |= fullscreen;
        self.screencast_held |= screencast;
    }

    /// Forgets what `now` has ended: the user's Do Not Disturb, and the schedule's hold.
    pub(crate) fn settle(&mut self, now: u64) {
        if self.manual.ended(now) {
            self.manual = DoNotDisturb::Off;
        }
        if self.schedule_held_until.is_some_and(|until| now >= until) {
            self.schedule_held_until = None;
        }
    }

    /// When the clock alone may next change it, after `now`: the user's end, the schedule's
    /// next start or end, or the end of its hold. `None` if only a call or Caminito may.
    pub(crate) fn next_change(
        &self,
        triggers: &Triggers,
        now: u64,
        zone: &glib::TimeZone,
    ) -> Option<u64> {
        let manual = match self.manual {
            DoNotDisturb::On { end } => end,
            DoNotDisturb::Off => None,
        };
        let schedule = triggers.schedule.into_iter().flat_map(|schedule| {
            [
                next_at(now, schedule.from, zone),
                next_at(now, schedule.to, zone),
            ]
        });
        manual
            .into_iter()
            .chain(schedule)
            .chain(self.schedule_held_until)
            .min()
    }

    /// The schedule, if it's on at `now` and not held off.
    fn schedule_on(
        &self,
        triggers: &Triggers,
        now: u64,
        zone: &glib::TimeZone,
    ) -> Option<Schedule> {
        let held = self.schedule_held_until.is_some_and(|until| now < until);
        triggers
            .schedule
            .filter(|schedule| !held && schedule.contains(now, zone))
    }

    /// Whether the fullscreen window and the screencast are on, each with its setting.
    fn triggered(&self, triggers: &Triggers) -> (bool, bool) {
        (
            self.fullscreen && triggers.while_fullscreen,
            self.screencasting && triggers.while_screencasting,
        )
    }
}

/// `now` in `zone`.
fn local(now: u64, zone: &glib::TimeZone) -> Option<glib::DateTime> {
    glib::DateTime::from_unix_utc(i64::try_from(now).ok()?)
        .ok()?
        .to_timezone(zone)
        .ok()
}

/// The minute of the day `now` is in, in `zone`.
fn minute_of_day(now: u64, zone: &glib::TimeZone) -> Option<u32> {
    let local = local(now, zone)?;
    u32::try_from(local.hour() * 60 + local.minute()).ok()
}

/// The first time after `now` whose time of day is `minute` in `zone`.
fn next_at(now: u64, minute: u32, zone: &glib::TimeZone) -> u64 {
    let next = (|| {
        let today = local(now, zone)?;
        let minute = i32::try_from(minute).ok()?;
        (0..=2).find_map(|days| {
            let day = today.add_days(days).ok()?;
            let (year, month, day) = day.ymd();
            let at =
                glib::DateTime::new(zone, year, month, day, minute / 60, minute % 60, 0.0).ok()?;
            u64::try_from(at.to_unix()).ok().filter(|&at| at > now)
        })
    })();
    // A time GLib can't place is a day away.
    next.unwrap_or(now + 24 * 60 * 60)
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

    /// 1970-01-12 at `hour`:`minute` in UTC, or the next days' with `day` past 12.
    fn at(day: i32, hour: i32, minute: i32) -> u64 {
        utc_time(day, hour, minute)
    }

    /// 22:00 to 07:00, and both triggers on.
    fn nightly() -> Triggers {
        Triggers {
            schedule: Some(Schedule {
                from: 22 * 60,
                to: 7 * 60,
            }),
            while_fullscreen: true,
            while_screencasting: true,
        }
    }

    fn on(reason: Reason, end: Option<u64>) -> Effective {
        Effective::On { reason, end }
    }

    #[test]
    fn the_schedule_is_on_from_its_start_to_its_end_across_midnight() {
        let utc = glib::TimeZone::utc();
        let sources = Sources::default();
        let at_time =
            |day, hour, minute| sources.effective(&nightly(), at(day, hour, minute), &utc);
        assert_eq!(at_time(12, 21, 59), Effective::Off);
        assert_eq!(at_time(12, 22, 0), on(Reason::Schedule, Some(at(13, 7, 0))));
        assert_eq!(at_time(13, 0, 30), on(Reason::Schedule, Some(at(13, 7, 0))));
        assert_eq!(at_time(13, 6, 59), on(Reason::Schedule, Some(at(13, 7, 0))));
        assert_eq!(at_time(13, 7, 0), Effective::Off);
    }

    #[test]
    fn a_schedule_within_the_day_is_on_between_its_times_and_an_empty_one_never() {
        let utc = glib::TimeZone::utc();
        let sources = Sources::default();
        let mut triggers = nightly();
        triggers.schedule = Some(Schedule {
            from: 9 * 60,
            to: 17 * 60 + 30,
        });
        assert_eq!(
            sources.effective(&triggers, at(12, 8, 59), &utc),
            Effective::Off
        );
        assert_eq!(
            sources.effective(&triggers, at(12, 9, 0), &utc),
            on(Reason::Schedule, Some(at(12, 17, 30)))
        );
        assert_eq!(
            sources.effective(&triggers, at(12, 17, 30), &utc),
            Effective::Off
        );
        triggers.schedule = Some(Schedule { from: 600, to: 600 });
        assert_eq!(
            sources.effective(&triggers, at(12, 10, 0), &utc),
            Effective::Off
        );
    }

    #[test]
    fn the_schedule_is_in_the_zones_own_time() {
        let three_hours_west = glib::TimeZone::from_offset(-3 * 60 * 60);
        let sources = Sources::default();
        // 01:00 in UTC is 22:00 there.
        assert_eq!(
            sources.effective(&nightly(), at(13, 0, 59), &three_hours_west),
            Effective::Off
        );
        assert_eq!(
            sources.effective(&nightly(), at(13, 1, 0), &three_hours_west),
            on(Reason::Schedule, Some(at(13, 10, 0)))
        );
    }

    #[test]
    fn each_trigger_turns_it_on_only_while_its_setting_is_on() {
        let utc = glib::TimeZone::utc();
        let noon = at(12, 12, 0);
        let mut sources = Sources::default();
        let mut triggers = nightly();
        sources.set_fullscreen(true);
        assert_eq!(
            sources.effective(&triggers, noon, &utc),
            on(Reason::Fullscreen, None)
        );
        triggers.while_fullscreen = false;
        assert_eq!(sources.effective(&triggers, noon, &utc), Effective::Off);
        sources.set_screencasting(true);
        assert_eq!(
            sources.effective(&triggers, noon, &utc),
            on(Reason::Screencast, None)
        );
        triggers.while_screencasting = false;
        assert_eq!(sources.effective(&triggers, noon, &utc), Effective::Off);
    }

    #[test]
    fn the_reason_is_the_user_then_the_schedule_then_fullscreen_then_the_screencast() {
        let utc = glib::TimeZone::utc();
        let night = at(12, 23, 0);
        let mut sources = Sources::default();
        sources.set_fullscreen(true);
        sources.set_screencasting(true);
        assert_eq!(
            sources.effective(&nightly(), at(12, 12, 0), &utc),
            on(Reason::Fullscreen, None)
        );
        assert_eq!(
            sources.effective(&nightly(), night, &utc),
            on(Reason::Schedule, Some(at(13, 7, 0)))
        );
        sources.manual = DoNotDisturb::On {
            end: Some(night + 60),
        };
        assert_eq!(
            sources.effective(&nightly(), night, &utc),
            on(Reason::Manual, Some(night + 60))
        );
        sources.set_fullscreen(false);
        sources.manual = DoNotDisturb::Off;
        assert_eq!(
            sources.effective(&nightly(), at(12, 12, 0), &utc),
            on(Reason::Screencast, None)
        );
    }

    #[test]
    fn turning_it_off_during_the_schedule_holds_it_off_until_the_schedules_next_start() {
        let utc = glib::TimeZone::utc();
        let mut sources = Sources::default();
        sources.turn_off(&nightly(), at(12, 23, 0), &utc);
        for (day, hour) in [(12, 23), (13, 6), (13, 12), (13, 21)] {
            assert_eq!(
                sources.effective(&nightly(), at(day, hour, 0), &utc),
                Effective::Off,
                "{day} at {hour}:00"
            );
        }
        assert_eq!(
            sources.effective(&nightly(), at(13, 22, 0), &utc),
            on(Reason::Schedule, Some(at(14, 7, 0)))
        );
    }

    #[test]
    fn turning_it_off_holds_a_trigger_off_until_it_ends() {
        let utc = glib::TimeZone::utc();
        let noon = at(12, 12, 0);
        let mut sources = Sources::default();
        sources.set_fullscreen(true);
        sources.manual = DoNotDisturb::On { end: None };
        sources.turn_off(&nightly(), noon, &utc);
        assert_eq!(sources.manual, DoNotDisturb::Off);
        assert_eq!(sources.effective(&nightly(), noon, &utc), Effective::Off);
        // A trigger that wasn't on isn't held.
        sources.set_screencasting(true);
        assert_eq!(
            sources.effective(&nightly(), noon, &utc),
            on(Reason::Screencast, None)
        );
        sources.set_screencasting(false);
        sources.set_fullscreen(false);
        sources.set_fullscreen(true);
        assert_eq!(
            sources.effective(&nightly(), noon, &utc),
            on(Reason::Fullscreen, None)
        );
    }

    #[test]
    fn the_next_change_is_the_nearest_of_the_users_end_and_the_schedules_times() {
        let utc = glib::TimeZone::utc();
        let mut sources = Sources::default();
        let mut triggers = nightly();
        assert_eq!(
            sources.next_change(&triggers, at(12, 12, 0), &utc),
            Some(at(12, 22, 0))
        );
        assert_eq!(
            sources.next_change(&triggers, at(12, 23, 0), &utc),
            Some(at(13, 7, 0))
        );
        sources.manual = DoNotDisturb::On {
            end: Some(at(12, 13, 0)),
        };
        assert_eq!(
            sources.next_change(&triggers, at(12, 12, 0), &utc),
            Some(at(12, 13, 0))
        );
        triggers.schedule = None;
        sources.manual = DoNotDisturb::On { end: None };
        assert_eq!(sources.next_change(&triggers, at(12, 12, 0), &utc), None);
    }
}
