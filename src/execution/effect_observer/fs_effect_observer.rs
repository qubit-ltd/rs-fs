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
use crate::error::FsEffectState;

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
    /// or waiting on its condition variable encounters a poisoned lock.
    pub fn wait_final(&self) -> Result<FsEffectFact, FsEffectObservationError> {
        let mut snapshot = self
            .state
            .snapshot
            .lock()
            .map_err(|_| FsEffectObservationError::Poisoned)?;
        loop {
            match &*snapshot {
                FsEffectSnapshot::Final(fact) => return Ok(fact.clone()),
                FsEffectSnapshot::Pending => {
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
    /// poisoned.
    #[inline]
    pub fn wait_final_async(&self) -> FsEffectWait<'_> {
        FsEffectWait { observer: self }
    }
    /// Publishes final effect evidence to observer clones.
    ///
    /// # Parameters
    ///
    /// - `state`: Confirmed effect state to publish to observers.
    ///
    /// The first publication wins. Lock poisoning prevents publication and is
    /// intentionally not returned by this best-effort notification method.
    #[inline]
    pub fn publish(&self, state: FsEffectState) {
        self.publish_fact(FsEffectFact::State(state));
    }
    /// Publishes typed final evidence once; later publications cannot replace
    /// it.
    ///
    /// # Parameters
    ///
    /// - `fact`: Typed provider evidence retained by the observer snapshot.
    ///
    /// A poisoned state lock prevents publication; otherwise the first fact
    /// wakes synchronous waiters and registered async tasks.
    pub fn publish_fact(&self, fact: FsEffectFact) {
        let mut published = false;
        if let Ok(mut v) = self.state.snapshot.lock()
            && matches!(*v, FsEffectSnapshot::Pending)
        {
            *v = FsEffectSnapshot::Final(fact);
            published = true;
        }
        if published {
            self.state.changed.notify_all();
            if let Ok(mut wakers) = self.state.wakers.lock() {
                for waker in wakers.drain(..) {
                    waker.wake();
                }
            }
        }
    }
}
