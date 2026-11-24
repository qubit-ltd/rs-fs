// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::Arc;

use super::FsEffectFact;
use super::FsEffectObservationError;
use super::FsEffectSnapshot;
use super::FsEffectWait;
use super::internal::FsEffectShared;

/// Cloneable read-only port for observing a managed mutation effect.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::FsEffectFact;
/// use qubit_fs::FsEffectSlotPool;
/// use qubit_fs::FsEffectSnapshot;
/// use qubit_fs::FsEffectState;
///
/// let reservation = FsEffectSlotPool::new(1).reserve().expect("slot available");
/// let observer = reservation.observer();
/// reservation.publish(FsEffectFact::State(FsEffectState::Unchanged));
/// assert!(matches!(observer.snapshot(), FsEffectSnapshot::Final(_)));
/// ```
///
/// Observers cannot publish final evidence:
///
/// ```compile_fail
/// use qubit_fs::FsEffectSlotPool;
/// use qubit_fs::FsEffectState;
///
/// let reservation = FsEffectSlotPool::new(1).reserve().unwrap();
/// let observer = reservation.observer();
/// observer.publish(FsEffectState::Applied);
/// ```
///
/// They also cannot publish typed facts:
///
/// ```compile_fail
/// use qubit_fs::FsEffectFact;
/// use qubit_fs::FsEffectSlotPool;
/// use qubit_fs::FsEffectState;
///
/// let reservation = FsEffectSlotPool::new(1).reserve().unwrap();
/// let observer = reservation.observer();
/// observer.publish_fact(FsEffectFact::State(FsEffectState::Applied));
/// ```
#[must_use = "retain the observer to inspect or await the mutation effect"]
#[derive(Clone)]
pub struct FsEffectObserver {
    /// Shared snapshot and waiter lists observed by all clones of this port.
    pub(super) state: Arc<FsEffectShared>,
}
impl FsEffectObserver {
    /// Returns the latest effect snapshot without consuming it.
    ///
    /// # Returns
    /// A clone of the current snapshot; a poisoned snapshot lock is reported
    /// as `Pending` because this non-fallible accessor cannot expose the lock
    /// error.
    pub fn snapshot(&self) -> FsEffectSnapshot {
        self.state
            .snapshot
            .lock()
            .map(|v| v.clone())
            .unwrap_or(FsEffectSnapshot::Pending)
    }
    /// Waits synchronously until typed final evidence is available.
    ///
    /// # Returns
    /// The published fact after the snapshot reaches `Final`.
    ///
    /// # Errors
    /// Returns [`FsEffectObservationError::Poisoned`] if locking the snapshot
    /// or waiting on its condition variable encounters a poisoned lock, or
    /// [`FsEffectObservationError::PublisherClosed`] if the last publisher is
    /// dropped before final evidence arrives. In that case the effect is
    /// unknown.
    pub fn wait_final(&self) -> Result<FsEffectFact, FsEffectObservationError> {
        let mut snapshot = self
            .state
            .snapshot
            .lock()
            .map_err(|_| FsEffectObservationError::Poisoned)?;
        #[cfg(test)]
        let mut notified_wait_stage = false;
        loop {
            match &*snapshot {
                FsEffectSnapshot::Final(fact) => return Ok(fact.clone()),
                FsEffectSnapshot::Closed => {
                    return Err(FsEffectObservationError::PublisherClosed);
                }
                FsEffectSnapshot::Pending => {
                    #[cfg(test)]
                    if !notified_wait_stage {
                        notified_wait_stage = true;
                        if let Ok(signal) = self.state.sync_wait_started.lock()
                            && let Some(sender) = signal.as_ref()
                        {
                            let _ = sender.send(());
                        }
                    }
                    snapshot = self
                        .state
                        .changed
                        .wait(snapshot)
                        .map_err(|_| FsEffectObservationError::Poisoned)?;
                }
            }
        }
    }
    /// Returns a future that waits without blocking for typed final evidence.
    ///
    /// # Returns
    /// A future that resolves to the final fact or to
    /// [`FsEffectObservationError::Poisoned`] if shared observation state is
    /// poisoned, or [`FsEffectObservationError::PublisherClosed`] if the last
    /// publisher closes before publishing. A closed publisher leaves the
    /// effect unknown.
    #[inline]
    pub fn wait_final_async(&self) -> FsEffectWait<'_> {
        FsEffectWait {
            observer: self,
            registration_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use super::super::FsEffectSlotPool;
    use super::FsEffectObservationError;
    use super::FsEffectSnapshot;

    #[test]
    fn test_last_publisher_close_wakes_synchronous_waiters() {
        let pool = FsEffectSlotPool::new(1);
        let reservation = pool.reserve().expect("slot available");
        let observer = reservation.observer();
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        *observer.state.sync_wait_started.lock().unwrap() = Some(started_tx);

        let (result_tx, result_rx) = mpsc::channel();
        let mut waiters = Vec::new();
        for _ in 0..2 {
            let observer = observer.clone();
            let result_tx = result_tx.clone();
            waiters.push(thread::spawn(move || {
                result_tx.send(observer.wait_final()).unwrap();
            }));
        }
        drop(result_tx);

        let mut started = true;
        for _ in 0..2 {
            if started_rx.recv_timeout(Duration::from_secs(2)).is_err() {
                started = false;
                break;
            }
        }

        // Dropping the final publisher contends on the exact snapshot lock
        // held by each waiter up to its atomic Condvar transition.
        drop(reservation);
        assert!(matches!(observer.snapshot(), FsEffectSnapshot::Closed));

        let mut closed_waiters = 0;
        for _ in 0..2 {
            if matches!(
                result_rx.recv_timeout(Duration::from_secs(2)),
                Ok(Err(FsEffectObservationError::PublisherClosed))
            ) {
                closed_waiters += 1;
            }
        }
        assert!(started, "synchronous waiter did not reach Condvar::wait");
        assert_eq!(closed_waiters, 2, "publisher close did not wake both waiters");
        for waiter in waiters {
            waiter.join().unwrap();
        }
        assert!(pool.reserve().is_ok(), "last publisher returns capacity");
    }
}
