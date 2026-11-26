// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
#[cfg(test)]
use std::sync::mpsc::SyncSender;
use std::task::Waker;

use super::super::FsEffectFact;
use super::super::FsEffectSnapshot;
use super::fs_effect_waiter::FsEffectWaiter;

/// Shared synchronization state behind mutation-effect observers.
pub(in crate::execution::effect_observer) struct FsEffectShared {
    /// Latest immutable pending, final, or closed state.
    pub(in crate::execution::effect_observer) snapshot: Mutex<FsEffectSnapshot>,
    /// Notifies blocking waiters after final evidence is published or closed.
    pub(in crate::execution::effect_observer) changed: Condvar,
    /// Async task wakers registered while evidence is still pending.
    pub(in crate::execution::effect_observer) wakers: Mutex<Vec<FsEffectWaiter>>,
    /// Test-only signal sent while a synchronous waiter holds the snapshot
    /// lock immediately before entering the condition-variable wait.
    #[cfg(test)]
    pub(in crate::execution::effect_observer) sync_wait_started: Mutex<Option<SyncSender<()>>>,
}

impl FsEffectShared {
    /// Creates a pending snapshot with empty synchronous and asynchronous
    /// waiters.
    pub(in crate::execution::effect_observer) fn new() -> Self {
        Self {
            snapshot: Mutex::new(FsEffectSnapshot::Pending),
            changed: Condvar::new(),
            wakers: Mutex::new(Vec::new()),
            #[cfg(test)]
            sync_wait_started: Mutex::new(None),
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

    /// Closes a still-pending publisher and notifies both kinds of waiter.
    /// Published facts remain immutable. Notifications run after releasing the
    /// snapshot lock.
    pub(in crate::execution::effect_observer) fn close(&self) {
        let closed = if let Ok(mut snapshot) = self.snapshot.lock()
            && matches!(*snapshot, FsEffectSnapshot::Pending)
        {
            *snapshot = FsEffectSnapshot::Closed;
            true
        } else {
            false
        };

        if closed {
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
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| waiter.waker.wake())) {
                // A panic payload's destructor is arbitrary user code too.
                // Isolate it so one hostile waker cannot prevent later
                // registrations from being notified.
                if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
                    // Its destructor panicked as well. Dropping this second
                    // payload could unwind again, so intentionally leak it.
                    std::mem::forget(payload);
                }
            }
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

    struct PublishOnDrop {
        shared: Weak<FsEffectShared>,
        published: Arc<AtomicUsize>,
        wake_calls: Arc<AtomicUsize>,
    }

    impl Wake for PublishOnDrop {
        fn wake(self: Arc<Self>) {
            self.wake_calls.fetch_add(1, Ordering::SeqCst);
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.wake_calls.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl Drop for PublishOnDrop {
        fn drop(&mut self) {
            let Some(shared) = self.shared.upgrade() else {
                return;
            };
            if shared.wakers.try_lock().is_err() {
                return;
            }
            self.published.fetch_add(1, Ordering::SeqCst);
            shared.publish(FsEffectFact::State(FsEffectState::Applied));
        }
    }

    struct CountingWake(Arc<AtomicUsize>);

    impl Wake for CountingWake {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
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

    #[test]
    fn test_effect_wait_registration_cancellation_interleaves_with_publication() {
        let shared = Arc::new(FsEffectShared::new());
        let cancelled_id = Arc::new(());
        let active_id = Arc::new(());
        let published = Arc::new(AtomicUsize::new(0));
        let cancelled_wakes = Arc::new(AtomicUsize::new(0));
        let active_wakes = Arc::new(AtomicUsize::new(0));
        let cancelled_waker = Waker::from(Arc::new(PublishOnDrop {
            shared: Arc::downgrade(&shared),
            published: Arc::clone(&published),
            wake_calls: Arc::clone(&cancelled_wakes),
        }));
        let active_waker = Waker::from(Arc::new(CountingWake(Arc::clone(&active_wakes))));

        shared.register_waiter(&cancelled_id, &cancelled_waker).unwrap();
        shared.register_waiter(&active_id, &active_waker).unwrap();
        drop(cancelled_waker);
        drop(active_waker);

        // Removing the cancelled task drops its waker after releasing the
        // registry lock. That callback publishes final evidence immediately,
        // while cancellation is still in progress, and must wake the survivor.
        shared.unregister_waiter(&cancelled_id);

        assert_eq!(published.load(Ordering::SeqCst), 1);
        assert_eq!(active_wakes.load(Ordering::SeqCst), 1);
    }
}
