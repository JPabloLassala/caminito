//! The lock state machine, as a pure type (12, 15): which lock requests are accepted, and
//! what the session is left in when the locker unlocks or goes away.

/// Whether the session is locked, and who holds the lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum LockState {
    #[default]
    Unlocked,
    /// Locked at Caminito's own request, before the locker has taken the lock.
    Pending,
    /// Locked, with a locker that holds the lock.
    Locked,
    /// Locked, with the locker that held the lock gone.
    LockerLost,
}

/// Who a client that asks for the lock is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Asker {
    /// A locker Caminito spawned.
    SpawnedLocker,
    /// Any other client.
    Other,
}

/// Why a client's `lock` was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refused {
    /// A locker holds the lock.
    Held,
    /// The session is locked with no locker, and only one Caminito spawned takes that lock.
    NotSpawned,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::Held => write!(f, "a locker holds the lock"),
            Refused::NotSpawned => write!(
                f,
                "the session is locked with no locker, and only one caminito spawned takes its lock"
            ),
        }
    }
}

impl std::error::Error for Refused {}

impl LockState {
    pub(crate) fn is_locked(self) -> bool {
        self != LockState::Unlocked
    }

    /// The state's name in the debug dump.
    pub(crate) fn name(self) -> &'static str {
        match self {
            LockState::Unlocked => "unlocked",
            LockState::Pending => "pending",
            LockState::Locked => "locked",
            LockState::LockerLost => "locker-lost",
        }
    }

    /// Caminito asks for the lock itself. A session that's locked stays as it is.
    pub(crate) fn requested(self) -> Self {
        match self {
            LockState::Unlocked => LockState::Pending,
            locked => locked,
        }
    }

    /// A client asks for the lock. Any client may lock an unlocked session. A lock that no
    /// locker holds goes only to a locker Caminito spawned, so that killing the locker
    /// doesn't let another process take the lock and unlock it (12).
    pub(crate) fn client_locks(self, asker: Asker) -> Result<Self, Refused> {
        match (self, asker) {
            (LockState::Unlocked, _) => Ok(LockState::Locked),
            (LockState::Locked, _) => Err(Refused::Held),
            (LockState::Pending | LockState::LockerLost, Asker::SpawnedLocker) => {
                Ok(LockState::Locked)
            }
            (LockState::Pending | LockState::LockerLost, Asker::Other) => Err(Refused::NotSpawned),
        }
    }

    /// The locker that holds the lock unlocks.
    pub(crate) fn unlocked(self) -> Self {
        match self {
            LockState::Locked => LockState::Unlocked,
            other => other,
        }
    }

    /// The locker that holds the lock went away without unlocking.
    pub(crate) fn locker_gone(self) -> Self {
        match self {
            LockState::Locked => LockState::LockerLost,
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Asker::{Other, SpawnedLocker};
    use super::LockState::{Locked, LockerLost, Pending, Unlocked};
    use super::*;

    #[test]
    fn only_the_unlocked_state_is_unlocked() {
        assert!(!Unlocked.is_locked());
        for state in [Pending, Locked, LockerLost] {
            assert!(state.is_locked(), "{state:?}");
        }
    }

    #[test]
    fn caminitos_own_request_locks_at_once_and_waits_for_the_locker() {
        assert_eq!(Unlocked.requested(), Pending);
    }

    #[test]
    fn a_request_while_locked_changes_nothing() {
        for state in [Pending, Locked, LockerLost] {
            assert_eq!(state.requested(), state);
        }
    }

    #[test]
    fn any_clients_lock_locks_an_unlocked_session() {
        assert_eq!(Unlocked.client_locks(Other), Ok(Locked));
        assert_eq!(Unlocked.client_locks(SpawnedLocker), Ok(Locked));
    }

    #[test]
    fn the_spawned_locker_takes_a_pending_lock() {
        assert_eq!(Pending.client_locks(SpawnedLocker), Ok(Locked));
    }

    #[test]
    fn another_client_is_refused_a_pending_lock() {
        assert_eq!(Pending.client_locks(Other), Err(Refused::NotSpawned));
    }

    #[test]
    fn a_second_lock_is_refused_while_a_locker_holds_it() {
        assert_eq!(Locked.client_locks(Other), Err(Refused::Held));
        assert_eq!(Locked.client_locks(SpawnedLocker), Err(Refused::Held));
    }

    #[test]
    fn the_lockers_unlock_unlocks() {
        assert_eq!(Locked.unlocked(), Unlocked);
    }

    #[test]
    fn a_locker_that_goes_away_leaves_the_session_locked() {
        assert_eq!(Locked.locker_gone(), LockerLost);
        assert!(Locked.locker_gone().is_locked());
    }

    #[test]
    fn the_spawned_locker_takes_a_lost_lock_over() {
        assert_eq!(LockerLost.client_locks(SpawnedLocker), Ok(Locked));
    }

    #[test]
    fn another_client_is_refused_a_lost_lock() {
        assert_eq!(LockerLost.client_locks(Other), Err(Refused::NotSpawned));
    }

    #[test]
    fn an_unlock_or_a_lost_locker_without_a_held_lock_changes_nothing() {
        for state in [Unlocked, Pending, LockerLost] {
            assert_eq!(state.unlocked(), state);
            assert_eq!(state.locker_gone(), state);
        }
    }

    #[test]
    fn the_dump_names_each_state() {
        let names = [Unlocked, Pending, Locked, LockerLost].map(LockState::name);
        assert_eq!(names, ["unlocked", "pending", "locked", "locker-lost"]);
    }
}
