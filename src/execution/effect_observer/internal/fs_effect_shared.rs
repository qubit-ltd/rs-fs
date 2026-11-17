// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::task::Waker;

use super::super::FsEffectFact;
use super::super::FsEffectSnapshot;
use super::fs_effect_waiter::FsEffectWaiter;

/// Shared synchronization state behind mutation-effect observers.
pub(in crate::execution::effect_observer) struct FsEffectShared {
    /// Latest immutable pending or final evidence.
    pub(in crate::execution::effect_observer) snapshot: Mutex<FsEffectSnapshot>,
    /// Notifies blocking waiters after final evidence is published.
    pub(in crate::execution::effect_observer) changed: Condvar,
    /// Async task wakers registered while evidence is still pending.
    pub(in crate::execution::effect_observer) wakers: Mutex<Vec<FsEffectWaiter>>,
}

impl FsEffectShared {
    /// Creates a pending snapshot with empty synchronous and asynchronous
    /// waiters.
    pub(in crate::execution::effect_observer) fn new() -> Self {
        Self {
            snapshot: Mutex::new(FsEffectSnapshot::Pending),
            changed: Condvar::new(),
            wakers: Mutex::new(Vec::new()),
        }
    }

    /// Publishes final evidence once and notifies both kinds of waiter.
    ///
    /// The snapshot lock is released before notification callbacks run.
    ///
    /// # Parameters
    ///
    /// - `fact`: Final operation-effect evidence to publish.
    pub(in crate::execution::effect_observer) fn publish(&self, fact: FsEffectFact) {
        let published = if let Ok(mut snapshot) = self.snapshot.lock()
            && matches!(*snapshot, FsEffectSnapshot::Pending)
        {
            *snapshot = FsEffectSnapshot::Final(fact);
            true
        } else {
            false
        };

        if published {
            self.changed.notify_all();
            self.wake_waiters();
        }
    }

    /// Registers or updates one future's waker, keeping all callbacks outside
    /// the shared registration lock.
    ///
    /// # Parameters
    ///
    /// - `id`: Identity token of the future that owns the registration.
    /// - `waker`: Task waker to register or replace.
    pub(in crate::execution::effect_observer) fn register_waiter(&self, id: &Arc<()>, waker: &Waker) -> Result<(), ()> {
        let candidate = waker.clone();
        let old_waker = match self.wakers.lock() {
            Ok(mut waiters) => {
                if let Some(waiter) = waiters.iter_mut().find(|waiter| Arc::ptr_eq(&waiter.id, id)) {
                    if waiter.waker.will_wake(&candidate) {
                        None
                    } else {
                        Some(std::mem::replace(&mut waiter.waker, candidate))
                    }
                } else {
                    waiters.push(FsEffectWaiter {
                        id: Arc::clone(id),
                        waker: candidate,
                    });
                    return Ok(());
                }
            }
            Err(_) => return Err(()),
        };
        drop(old_waker);
        Ok(())
    }

    /// Removes one registration, recovering a poisoned lock so future drop
    /// cannot panic or leave a known stale waiter behind.
    ///
    /// # Parameters
    ///
    /// - `id`: Identity token of the future whose registration is removed.
    pub(in crate::execution::effect_observer) fn unregister_waiter(&self, id: &Arc<()>) {
        let removed = match self.wakers.lock() {
            Ok(mut waiters) => remove_waiter(&mut waiters, id),
            Err(poisoned) => {
                let mut waiters = poisoned.into_inner();
                let removed = remove_waiter(&mut waiters, id);
                drop(waiters);
                removed
            }
        };
        drop(removed);
    }

    /// Wakes all registered tasks after moving their callbacks out of the
    /// shared lock.
    fn wake_waiters(&self) {
        let waiters = match self.wakers.lock() {
            Ok(mut waiters) => std::mem::take(&mut *waiters),
            Err(_) => return,
        };
        for waiter in waiters {
            waiter.waker.wake();
        }
    }
}

/// Removes and returns the registration matching `id`, if present.
///
/// # Parameters
///
/// - `waiters`: Mutable registrations to search.
/// - `id`: Identity token of the owning future.
fn remove_waiter(waiters: &mut Vec<FsEffectWaiter>, id: &Arc<()>) -> Option<FsEffectWaiter> {
    waiters
        .iter()
        .position(|waiter| Arc::ptr_eq(&waiter.id, id))
        .map(|index| waiters.swap_remove(index))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Weak;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::task::Wake;
    use std::task::Waker;

    use super::FsEffectShared;
    use crate::error::FsEffectState;
    use crate::execution::FsEffectFact;

    struct LockCheckingWake {
        shared: Weak<FsEffectShared>,
        callbacks_under_lock: Arc<AtomicUsize>,
    }

    impl LockCheckingWake {
        fn record_if_locked(&self) {
            if let Some(shared) = self.shared.upgrade()
                && shared.wakers.try_lock().is_err()
            {
                self.callbacks_under_lock.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    impl Wake for LockCheckingWake {
        fn wake(self: Arc<Self>) {
            self.record_if_locked();
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.record_if_locked();
        }
    }

    impl Drop for LockCheckingWake {
        fn drop(&mut self) {
            self.record_if_locked();
        }
    }

    fn lock_checking_waker(shared: &Arc<FsEffectShared>, callbacks_under_lock: &Arc<AtomicUsize>) -> Waker {
        Waker::from(Arc::new(LockCheckingWake {
            shared: Arc::downgrade(shared),
            callbacks_under_lock: Arc::clone(callbacks_under_lock),
        }))
    }

    #[test]
    fn test_effect_wait_registration_callbacks_run_outside_lock() {
        let shared = Arc::new(FsEffectShared::new());
        let callbacks_under_lock = Arc::new(AtomicUsize::new(0));
        let id = Arc::new(());
        let first = lock_checking_waker(&shared, &callbacks_under_lock);
        let first_owner = first.clone();
        shared.register_waiter(&id, &first).unwrap();
        drop(first);
        drop(first_owner);

        let replacement = lock_checking_waker(&shared, &callbacks_under_lock);
        shared.register_waiter(&id, &replacement).unwrap();
        assert_eq!(callbacks_under_lock.load(Ordering::SeqCst), 0);

        drop(replacement);
        shared.unregister_waiter(&id);
        assert_eq!(callbacks_under_lock.load(Ordering::SeqCst), 0);

        let waker = lock_checking_waker(&shared, &callbacks_under_lock);
        let wake_id = Arc::new(());
        shared.register_waiter(&wake_id, &waker).unwrap();
        drop(waker);
        shared.publish(FsEffectFact::State(FsEffectState::Applied));
        assert_eq!(callbacks_under_lock.load(Ordering::SeqCst), 0);
    }
}
