//! Idle's timers as a pure type (15, 128): when the session goes idle and the outputs blank,
//! when the lock is asked for after that, when the outputs are powered off (129), and when
//! each ext-idle-notify client is told it's idle. It reads no clock: every call takes `now`, and [`IdleState::deadline`] says when it
//! next needs [`IdleState::elapse`].

use std::time::Duration;

use crate::settings::translate::IdleSettings;

/// What the timers ask for, in the order they ask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Effect<N> {
    /// The session went idle: the outputs go black.
    Blank,
    /// Activity ended the blank.
    Unblank,
    /// The blank has lasted `lock-delay`: the session is to be locked.
    Lock,
    /// An ext-idle-notify notification went idle.
    Idled(N),
    /// One that was idle no longer is.
    Resumed(N),
    /// The session has gone `sleep-display` without activity: the outputs are powered off.
    PowerOff,
    /// Activity powered them on again.
    PowerOn,
}

/// The timers, and what they've brought.
#[derive(Debug)]
pub(crate) struct IdleState<N> {
    settings: IdleSettings,
    /// When the timers that inhibitors hold off count from: the last activity, or the
    /// moment the last inhibitor went, whichever is later.
    quiet_since: Duration,
    /// The last activity, which the notifications that ignore inhibitors count from.
    input_since: Duration,
    inhibited: bool,
    /// When the blank started, while the outputs are blanked: when it was due, so the lock
    /// delay counts from there whenever the timer fired.
    blanked_at: Option<Duration>,
    /// Whether this blank has asked for the lock.
    lock_asked: bool,
    /// Whether UPower says the machine runs on its battery, which picks the `sleep-display`
    /// key (129).
    on_battery: bool,
    /// Whether the outputs are powered off.
    powered_off: bool,
    notifications: Vec<Notification<N>>,
}

/// An ext-idle-notify notification.
#[derive(Debug)]
struct Notification<N> {
    handle: N,
    timeout: Duration,
    /// Made with `get_input_idle_notification`: inhibitors don't hold it off.
    input_only: bool,
    /// When it was made, before which nothing counts for it.
    since: Duration,
    idle: bool,
}

/// Which timer is due.
#[derive(Debug, Clone, Copy)]
enum Timer {
    Blank,
    Lock,
    PowerOff,
    /// The notification at this index.
    Notification(usize),
}

impl<N: Clone + PartialEq> IdleState<N> {
    pub(crate) fn new(settings: IdleSettings, now: Duration) -> Self {
        IdleState {
            settings,
            quiet_since: now,
            input_since: now,
            inhibited: false,
            blanked_at: None,
            lock_asked: false,
            on_battery: false,
            powered_off: false,
            notifications: Vec::new(),
        }
    }

    pub(crate) fn is_blanked(&self) -> bool {
        self.blanked_at.is_some()
    }

    pub(crate) fn is_powered_off(&self) -> bool {
        self.powered_off
    }

    pub(crate) fn is_inhibited(&self) -> bool {
        self.inhibited
    }

    /// How many notifications there are.
    pub(crate) fn notification_count(&self) -> usize {
        self.notifications.len()
    }

    /// Input on any device: the blank ends, the outputs are powered on, and every count
    /// starts again. It never unlocks (12).
    pub(crate) fn activity(&mut self, now: Duration) -> Vec<Effect<N>> {
        self.quiet_since = now;
        self.input_since = now;
        let mut effects = Vec::new();
        if self.blanked_at.take().is_some() {
            self.lock_asked = false;
            effects.push(Effect::Unblank);
        }
        if self.powered_off {
            self.powered_off = false;
            effects.push(Effect::PowerOn);
        }
        for notification in &mut self.notifications {
            if notification.idle {
                notification.idle = false;
                effects.push(Effect::Resumed(notification.handle.clone()));
            }
        }
        effects
    }

    /// Blanks now, as if the session had gone idle, and counts the lock delay from now:
    /// `SetActive(true)` (12). It's asked for, so neither `idle-activation-enabled` nor an
    /// inhibitor holds it off. A blank that's on stays as it is. The outputs stay lit until
    /// their own time: the shim's call is mate-screensaver's, and the power-off isn't.
    pub(crate) fn blank_now(&mut self, now: Duration) -> Vec<Effect<N>> {
        if self.blanked_at.is_some() {
            return Vec::new();
        }
        self.blanked_at = Some(now);
        self.lock_asked = false;
        let mut effects = vec![Effect::Blank];
        effects.extend(self.elapse(now));
        effects
    }

    /// Whether an inhibitor holds idle off. One that comes resumes the notifications it
    /// holds off, as Smithay's ext-idle-notify does (`idle_notify/mod.rs:138-165` at 928d4a9), and leaves a blank as it is; once the
    /// last goes, the count starts again.
    pub(crate) fn set_inhibited(&mut self, inhibited: bool, now: Duration) -> Vec<Effect<N>> {
        if inhibited == self.inhibited {
            return Vec::new();
        }
        self.inhibited = inhibited;
        let mut effects = Vec::new();
        if inhibited {
            for notification in &mut self.notifications {
                if notification.idle && !notification.input_only {
                    notification.idle = false;
                    effects.push(Effect::Resumed(notification.handle.clone()));
                }
            }
        } else {
            self.quiet_since = now;
        }
        effects
    }

    /// New settings, which apply at once: a count that's past its new delay is due now.
    pub(crate) fn settings_changed(
        &mut self,
        settings: IdleSettings,
        now: Duration,
    ) -> Vec<Effect<N>> {
        self.settings = settings;
        self.elapse(now)
    }

    /// Whether UPower says the machine runs on its battery: the other `sleep-display` key
    /// applies at once, counted from the same activity.
    pub(crate) fn set_on_battery(&mut self, on_battery: bool, now: Duration) -> Vec<Effect<N>> {
        self.on_battery = on_battery;
        self.elapse(now)
    }

    /// A notification that goes idle after `timeout` without activity, counted from now.
    pub(crate) fn add_notification(
        &mut self,
        handle: N,
        timeout: Duration,
        input_only: bool,
        now: Duration,
    ) -> Vec<Effect<N>> {
        self.notifications.push(Notification {
            handle,
            timeout,
            input_only,
            since: now,
            idle: false,
        });
        self.elapse(now)
    }

    pub(crate) fn remove_notification(&mut self, handle: &N) {
        self.notifications
            .retain(|notification| notification.handle != *handle);
    }

    /// When the next timer is due, if one is to come.
    pub(crate) fn deadline(&self) -> Option<Duration> {
        self.next().map(|(due, _)| due)
    }

    /// Runs every timer that's due at `now`, earliest first.
    pub(crate) fn elapse(&mut self, now: Duration) -> Vec<Effect<N>> {
        let mut effects = Vec::new();
        while let Some((due, timer)) = self.next().filter(|(due, _)| *due <= now) {
            match timer {
                Timer::Blank => {
                    self.blanked_at = Some(due);
                    effects.push(Effect::Blank);
                }
                Timer::Lock => {
                    self.lock_asked = true;
                    effects.push(Effect::Lock);
                }
                Timer::PowerOff => {
                    self.powered_off = true;
                    effects.push(Effect::PowerOff);
                }
                Timer::Notification(index) => {
                    let notification = &mut self.notifications[index];
                    notification.idle = true;
                    effects.push(Effect::Idled(notification.handle.clone()));
                }
            }
        }
        effects
    }

    /// The timer due first, and when. On a tie the blank comes before its lock, then the
    /// power-off, and all of them before the notifications, in the order they were made.
    fn next(&self) -> Option<(Duration, Timer)> {
        let settings = &self.settings;
        let blank = (self.blanked_at.is_none()
            && settings.blank
            && !settings.delay.is_zero()
            && !self.inhibited)
            .then(|| (self.quiet_since + settings.delay, Timer::Blank));
        let lock = match (self.blanked_at, settings.lock) {
            (Some(blanked_at), Some(delay)) if !self.lock_asked => {
                Some((blanked_at + delay, Timer::Lock))
            }
            _ => None,
        };
        let sleep = if self.on_battery {
            settings.sleep_display.battery
        } else {
            settings.sleep_display.ac
        };
        // Counted from the activity the blank's count is, as mate-power-manager's blank
        // timer is from the idle time, which its inhibitors cancel (`gpm-idle.c:186-230` at
        // 1.28.1).
        let power_off = (!self.powered_off && !sleep.is_zero() && !self.inhibited)
            .then(|| (self.quiet_since + sleep, Timer::PowerOff));
        let notifications = self
            .notifications
            .iter()
            .enumerate()
            .filter(|(_, notification)| {
                !notification.idle && (notification.input_only || !self.inhibited)
            })
            .map(|(index, notification)| {
                let counted = if notification.input_only {
                    self.input_since
                } else {
                    self.quiet_since
                };
                let since = counted.max(notification.since);
                (since + notification.timeout, Timer::Notification(index))
            });
        blank
            .into_iter()
            .chain(lock)
            .chain(power_off)
            .chain(notifications)
            .min_by_key(|(due, _)| *due)
    }
}

#[cfg(test)]
mod tests {
    use super::Effect::{Blank, Idled, Lock, PowerOff, PowerOn, Resumed, Unblank};
    use super::*;
    use crate::settings::translate::SleepDisplay;

    const SECOND: Duration = Duration::from_secs(1);
    const MINUTE: Duration = Duration::from_secs(60);

    /// `minutes` and `seconds` on the driven clock.
    fn at(minutes: u64, seconds: u64) -> Duration {
        Duration::from_secs(minutes * 60 + seconds)
    }

    /// Five minutes to idle, as MATE's default, blanking, no lock, and the outputs never
    /// powered off.
    fn blanking() -> IdleSettings {
        IdleSettings {
            delay: 5 * MINUTE,
            blank: true,
            lock: None,
            sleep_display: SleepDisplay {
                ac: Duration::ZERO,
                battery: Duration::ZERO,
            },
        }
    }

    /// [`blanking`], with the outputs powered off after `ac` on AC and `battery` on
    /// battery.
    fn powering_off_after(ac: Duration, battery: Duration) -> IdleSettings {
        IdleSettings {
            sleep_display: SleepDisplay { ac, battery },
            ..blanking()
        }
    }

    fn locking_after(delay: Duration) -> IdleSettings {
        IdleSettings {
            lock: Some(delay),
            ..blanking()
        }
    }

    /// Idle from 0 on the clock, whose notifications are named by numbers.
    fn idle(settings: IdleSettings) -> IdleState<u32> {
        IdleState::new(settings, Duration::ZERO)
    }

    #[test]
    fn the_outputs_blank_at_the_idle_delay() {
        let mut idle = idle(blanking());
        assert_eq!(idle.deadline(), Some(at(5, 0)));
        assert_eq!(idle.elapse(at(4, 59)), []);
        assert!(!idle.is_blanked());

        assert_eq!(idle.elapse(at(5, 0)), [Blank]);
        assert!(idle.is_blanked());
        assert_eq!(idle.deadline(), None);
        assert_eq!(idle.elapse(at(60, 0)), []);
    }

    #[test]
    fn nothing_blanks_with_idle_activation_off() {
        let mut idle = idle(IdleSettings {
            blank: false,
            ..locking_after(Duration::ZERO)
        });
        assert_eq!(idle.deadline(), None);
        assert_eq!(idle.elapse(at(60, 0)), []);
        assert!(!idle.is_blanked());
    }

    #[test]
    fn an_idle_delay_of_0_never_blanks() {
        let mut idle = idle(IdleSettings {
            delay: Duration::ZERO,
            ..blanking()
        });
        assert_eq!(idle.deadline(), None);
        assert_eq!(idle.elapse(at(60, 0)), []);
    }

    #[test]
    fn activity_restarts_the_count() {
        let mut idle = idle(blanking());
        assert_eq!(idle.activity(at(3, 0)), []);
        assert_eq!(idle.elapse(at(5, 0)), []);
        assert_eq!(idle.deadline(), Some(at(8, 0)));
        assert_eq!(idle.elapse(at(8, 0)), [Blank]);
    }

    #[test]
    fn activity_unblanks_and_restarts_the_count() {
        let mut idle = idle(blanking());
        idle.elapse(at(5, 0));

        assert_eq!(idle.activity(at(6, 0)), [Unblank]);
        assert!(!idle.is_blanked());
        assert_eq!(idle.elapse(at(10, 59)), []);
        assert_eq!(idle.elapse(at(11, 0)), [Blank]);
    }

    #[test]
    fn the_lock_comes_lock_delay_after_the_blank() {
        let mut idle = idle(locking_after(2 * MINUTE));
        assert_eq!(idle.elapse(at(5, 0)), [Blank]);
        assert_eq!(idle.deadline(), Some(at(7, 0)));
        assert_eq!(idle.elapse(at(6, 59)), []);

        assert_eq!(idle.elapse(at(7, 0)), [Lock]);
        assert_eq!(idle.deadline(), None);
        assert_eq!(idle.elapse(at(60, 0)), []);
    }

    /// The lock delay counts from when the blank was due, not from a late timer.
    #[test]
    fn a_late_timer_blanks_and_locks_together() {
        let mut idle = idle(locking_after(2 * MINUTE));
        assert_eq!(idle.elapse(at(7, 30)), [Blank, Lock]);
    }

    #[test]
    fn a_lock_delay_of_0_locks_as_the_blank_starts() {
        let mut idle = idle(locking_after(Duration::ZERO));
        assert_eq!(idle.elapse(at(5, 0)), [Blank, Lock]);
    }

    #[test]
    fn nothing_locks_with_lock_enabled_off() {
        let mut idle = idle(blanking());
        assert_eq!(idle.elapse(at(5, 0)), [Blank]);
        assert_eq!(idle.elapse(at(60, 0)), []);
    }

    #[test]
    fn activity_during_the_lock_delay_asks_for_no_lock() {
        let mut idle = idle(locking_after(2 * MINUTE));
        idle.elapse(at(5, 0));
        assert_eq!(idle.activity(at(6, 0)), [Unblank]);
        assert_eq!(idle.elapse(at(7, 0)), []);
        assert_eq!(idle.deadline(), Some(at(11, 0)));
    }

    #[test]
    fn the_next_blank_locks_again() {
        let mut idle = idle(locking_after(Duration::ZERO));
        idle.elapse(at(5, 0));
        idle.activity(at(6, 0));
        assert_eq!(idle.elapse(at(11, 0)), [Blank, Lock]);
    }

    /// `SetActive(true)` (127): as if the session had gone idle now.
    #[test]
    fn blanking_now_counts_the_lock_delay_from_now() {
        let mut idle = idle(locking_after(2 * MINUTE));
        assert_eq!(idle.blank_now(at(1, 0)), [Blank]);
        assert!(idle.is_blanked());
        assert_eq!(idle.deadline(), Some(at(3, 0)));
        assert_eq!(idle.elapse(at(3, 0)), [Lock]);
    }

    #[test]
    fn blanking_now_with_a_lock_delay_of_0_locks_at_once() {
        let mut idle = idle(locking_after(Duration::ZERO));
        assert_eq!(idle.blank_now(at(1, 0)), [Blank, Lock]);
    }

    /// It's asked for, so neither the settings nor an inhibitor hold it off.
    #[test]
    fn blanking_now_blanks_with_idle_activation_off_and_an_inhibitor() {
        let mut idle = idle(IdleSettings {
            blank: false,
            ..blanking()
        });
        idle.set_inhibited(true, at(0, 30));
        assert_eq!(idle.blank_now(at(1, 0)), [Blank]);
        assert_eq!(idle.activity(at(2, 0)), [Unblank]);
    }

    #[test]
    fn blanking_now_while_blanked_changes_nothing() {
        let mut idle = idle(locking_after(2 * MINUTE));
        idle.elapse(at(5, 0));
        assert_eq!(idle.blank_now(at(6, 0)), []);
        assert_eq!(idle.elapse(at(7, 0)), [Lock]);
    }

    #[test]
    fn an_inhibitor_holds_the_blank_off() {
        let mut idle = idle(blanking());
        assert_eq!(idle.set_inhibited(true, at(1, 0)), []);
        assert!(idle.is_inhibited());
        assert_eq!(idle.deadline(), None);
        assert_eq!(idle.elapse(at(60, 0)), []);
    }

    /// As Smithay's ext-idle-notify has it, which sets a notification's timer again as the
    /// inhibitor goes (`idle_notify/mod.rs:160-162` at 928d4a9).
    #[test]
    fn the_count_starts_again_when_the_inhibitor_goes() {
        let mut idle = idle(blanking());
        idle.set_inhibited(true, at(1, 0));
        assert_eq!(idle.set_inhibited(false, at(30, 0)), []);
        assert_eq!(idle.elapse(at(34, 59)), []);
        assert_eq!(idle.elapse(at(35, 0)), [Blank]);
    }

    /// Inhibitors keep the session from going idle, as mate-screensaver's do; they don't
    /// end a blank.
    #[test]
    fn an_inhibitor_that_comes_while_blanked_leaves_the_blank() {
        let mut idle = idle(locking_after(2 * MINUTE));
        idle.elapse(at(5, 0));
        assert_eq!(idle.set_inhibited(true, at(6, 0)), []);
        assert!(idle.is_blanked());
        assert_eq!(idle.elapse(at(7, 0)), [Lock]);
    }

    #[test]
    fn a_notification_goes_idle_after_its_timeout_and_resumes_with_activity() {
        let mut idle = idle(blanking());
        assert_eq!(idle.add_notification(1, 30 * SECOND, false, at(0, 10)), []);
        assert_eq!(idle.deadline(), Some(at(0, 40)));
        assert_eq!(idle.elapse(at(0, 39)), []);

        assert_eq!(idle.elapse(at(0, 40)), [Idled(1)]);
        assert_eq!(idle.elapse(at(1, 0)), []);
        assert_eq!(idle.activity(at(1, 0)), [Resumed(1)]);
        assert_eq!(idle.elapse(at(1, 30)), [Idled(1)]);
    }

    #[test]
    fn notifications_go_idle_with_idle_activation_off() {
        let mut idle = idle(IdleSettings {
            blank: false,
            ..blanking()
        });
        idle.add_notification(1, 6 * MINUTE, false, Duration::ZERO);
        assert_eq!(idle.elapse(at(6, 0)), [Idled(1)]);
    }

    #[test]
    fn each_notification_has_its_own_timeout() {
        let mut idle = idle(blanking());
        idle.add_notification(1, 10 * MINUTE, false, Duration::ZERO);
        idle.add_notification(2, 2 * MINUTE, false, Duration::ZERO);
        assert_eq!(idle.deadline(), Some(at(2, 0)));
        assert_eq!(idle.elapse(at(2, 0)), [Idled(2)]);
        assert_eq!(idle.elapse(at(5, 0)), [Blank]);
        assert_eq!(idle.elapse(at(10, 0)), [Idled(1)]);
        assert_eq!(idle.activity(at(11, 0)), [Unblank, Resumed(1), Resumed(2)]);
    }

    #[test]
    fn a_notification_removed_is_never_told() {
        let mut idle = idle(blanking());
        idle.add_notification(1, MINUTE, false, Duration::ZERO);
        idle.remove_notification(&1);
        assert_eq!(idle.elapse(at(1, 0)), []);
    }

    #[test]
    fn an_inhibitor_holds_a_notification_off_and_resumes_one_that_was_idle() {
        let mut idle = idle(blanking());
        idle.add_notification(1, MINUTE, false, Duration::ZERO);
        idle.elapse(at(1, 0));

        assert_eq!(idle.set_inhibited(true, at(2, 0)), [Resumed(1)]);
        assert_eq!(idle.elapse(at(60, 0)), []);
        idle.set_inhibited(false, at(60, 0));
        assert_eq!(idle.elapse(at(61, 0)), [Idled(1)]);
    }

    /// `get_input_idle_notification`, which only input ends.
    #[test]
    fn an_input_notification_ignores_inhibitors() {
        let mut idle = idle(blanking());
        idle.add_notification(1, MINUTE, true, Duration::ZERO);
        assert_eq!(idle.set_inhibited(true, at(0, 30)), []);
        assert_eq!(idle.elapse(at(1, 0)), [Idled(1)]);
        assert_eq!(idle.set_inhibited(false, at(2, 0)), []);
        assert_eq!(idle.activity(at(3, 0)), [Resumed(1)]);
    }

    #[test]
    fn a_shorter_idle_delay_applies_at_once() {
        let mut idle = idle(blanking());
        let shorter = IdleSettings {
            delay: MINUTE,
            ..blanking()
        };
        assert_eq!(idle.settings_changed(shorter, at(0, 30)), []);
        assert_eq!(idle.deadline(), Some(at(1, 0)));
        assert_eq!(idle.settings_changed(blanking(), at(0, 40)), []);
        assert_eq!(idle.settings_changed(shorter, at(2, 0)), [Blank]);
    }

    #[test]
    fn idle_activation_turned_on_blanks_at_the_delay() {
        let mut idle = idle(IdleSettings {
            blank: false,
            ..blanking()
        });
        assert_eq!(idle.settings_changed(blanking(), at(1, 0)), []);
        assert_eq!(idle.elapse(at(5, 0)), [Blank]);
    }

    #[test]
    fn lock_enabled_turned_off_while_blanked_asks_for_no_lock() {
        let mut idle = idle(locking_after(2 * MINUTE));
        idle.elapse(at(5, 0));
        assert_eq!(idle.settings_changed(blanking(), at(6, 0)), []);
        assert_eq!(idle.elapse(at(60, 0)), []);
    }

    #[test]
    fn a_new_lock_delay_counts_from_the_blank() {
        let mut idle = idle(locking_after(10 * MINUTE));
        idle.elapse(at(5, 0));
        assert_eq!(idle.settings_changed(locking_after(MINUTE), at(5, 30)), []);
        assert_eq!(idle.elapse(at(6, 0)), [Lock]);
    }

    // Powering the outputs off (129).

    #[test]
    fn the_outputs_power_off_at_the_sleep_display_of_ac() {
        let mut idle = idle(powering_off_after(30 * MINUTE, 10 * MINUTE));
        assert_eq!(idle.elapse(at(5, 0)), [Blank]);
        assert_eq!(idle.deadline(), Some(at(30, 0)));
        assert_eq!(idle.elapse(at(29, 59)), []);

        assert_eq!(idle.elapse(at(30, 0)), [PowerOff]);
        assert!(idle.is_powered_off());
        assert!(idle.is_blanked());
        assert_eq!(idle.deadline(), None);
        assert_eq!(idle.elapse(at(90, 0)), []);
    }

    #[test]
    fn on_battery_the_outputs_power_off_at_the_sleep_display_of_battery() {
        let mut idle = idle(powering_off_after(30 * MINUTE, 10 * MINUTE));
        assert_eq!(idle.set_on_battery(true, at(1, 0)), []);
        assert_eq!(idle.elapse(at(5, 0)), [Blank]);
        assert_eq!(idle.deadline(), Some(at(10, 0)));
        assert_eq!(idle.elapse(at(10, 0)), [PowerOff]);
    }

    /// Unplugged after the battery's time without activity: the outputs go off then.
    #[test]
    fn going_on_battery_past_its_sleep_display_powers_off_at_once() {
        let mut idle = idle(powering_off_after(30 * MINUTE, 10 * MINUTE));
        idle.elapse(at(5, 0));
        assert_eq!(idle.set_on_battery(true, at(12, 0)), [PowerOff]);
    }

    #[test]
    fn back_on_ac_the_sleep_display_of_ac_applies_again() {
        let mut idle = idle(powering_off_after(30 * MINUTE, 10 * MINUTE));
        idle.set_on_battery(true, at(1, 0));
        assert_eq!(idle.set_on_battery(false, at(2, 0)), []);
        assert_eq!(idle.elapse(at(10, 0)), [Blank]);
        assert_eq!(idle.deadline(), Some(at(30, 0)));
    }

    #[test]
    fn a_sleep_display_of_0_never_powers_off() {
        let mut idle = idle(powering_off_after(Duration::ZERO, 10 * MINUTE));
        assert_eq!(idle.elapse(at(5, 0)), [Blank]);
        assert_eq!(idle.deadline(), None);
        assert_eq!(idle.elapse(at(600, 0)), []);
        assert!(!idle.is_powered_off());
    }

    /// The outputs power off whatever `idle-activation-enabled` says, as mate-power-manager's
    /// DPMS does on X11: it's another program's key.
    #[test]
    fn the_outputs_power_off_with_idle_activation_off() {
        let mut idle = idle(IdleSettings {
            blank: false,
            ..powering_off_after(30 * MINUTE, 10 * MINUTE)
        });
        assert_eq!(idle.elapse(at(30, 0)), [PowerOff]);
        assert!(!idle.is_blanked());
    }

    /// A power-off shorter than the blank's delay comes first, and the blank after it.
    #[test]
    fn a_sleep_display_shorter_than_the_idle_delay_powers_off_first() {
        let mut idle = idle(powering_off_after(2 * MINUTE, 2 * MINUTE));
        assert_eq!(idle.elapse(at(2, 0)), [PowerOff]);
        assert_eq!(idle.elapse(at(5, 0)), [Blank]);
    }

    #[test]
    fn activity_powers_the_outputs_on_and_restarts_the_count() {
        let mut idle = idle(powering_off_after(30 * MINUTE, 10 * MINUTE));
        idle.elapse(at(30, 0));

        assert_eq!(idle.activity(at(31, 0)), [Unblank, PowerOn]);
        assert!(!idle.is_powered_off());
        assert_eq!(idle.elapse(at(36, 0)), [Blank]);
        assert_eq!(idle.elapse(at(60, 59)), []);
        assert_eq!(idle.elapse(at(61, 0)), [PowerOff]);
    }

    #[test]
    fn an_inhibitor_holds_the_power_off_off() {
        let mut idle = idle(powering_off_after(30 * MINUTE, 10 * MINUTE));
        idle.set_inhibited(true, at(1, 0));
        assert_eq!(idle.deadline(), None);
        assert_eq!(idle.elapse(at(600, 0)), []);
        assert!(!idle.is_powered_off());

        idle.set_inhibited(false, at(600, 0));
        assert_eq!(idle.elapse(at(630, 0)), [Blank, PowerOff]);
    }

    /// As with the blank: inhibitors keep the session from going idle, and don't light it.
    #[test]
    fn an_inhibitor_that_comes_while_powered_off_leaves_it_off() {
        let mut idle = idle(powering_off_after(30 * MINUTE, 10 * MINUTE));
        idle.elapse(at(30, 0));
        assert_eq!(idle.set_inhibited(true, at(31, 0)), []);
        assert!(idle.is_powered_off());
    }

    /// `SetActive(true)` blanks, and the outputs stay lit until their own time.
    #[test]
    fn blanking_now_leaves_the_outputs_on() {
        let mut idle = idle(powering_off_after(30 * MINUTE, 10 * MINUTE));
        assert_eq!(idle.blank_now(at(1, 0)), [Blank]);
        assert!(!idle.is_powered_off());
        assert_eq!(idle.deadline(), Some(at(30, 0)));
    }

    #[test]
    fn a_shorter_sleep_display_applies_at_once() {
        let mut idle = idle(powering_off_after(30 * MINUTE, 10 * MINUTE));
        idle.elapse(at(5, 0));
        let shorter = powering_off_after(6 * MINUTE, 10 * MINUTE);
        assert_eq!(idle.settings_changed(shorter, at(7, 0)), [PowerOff]);
    }
}
