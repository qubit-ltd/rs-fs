// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use super::FsEffectFact;
use super::FsEffectObservationError;
use super::FsEffectObserver;
use super::FsEffectSnapshot;

/// Future returned by [`FsEffectObserver::wait_final_async`].
///
/// # Type Parameters
///
/// - `'a`: Borrow lifetime of the observer whose shared state this future
///   polls.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::FsEffectFact;
/// use qubit_fs::FsEffectObservationError;
/// use qubit_fs::FsEffectObserver;
///
/// async fn wait_for_effect(observer: &FsEffectObserver) -> Result<FsEffectFact, FsEffectObservationError> {
///     observer.wait_final_async().await
/// }
/// ```
pub struct FsEffectWait<'a> {
    /// Observer whose shared snapshot and waker list this future polls.
    pub(super) observer: &'a FsEffectObserver,
    /// Identity of this future's optional shared waiter registration.
    pub(super) registration_id: Option<Arc<()>>,
}
impl Future for FsEffectWait<'_> {
    /// Resolves with the published fact or a poisoned-lock error.
    type Output = Result<FsEffectFact, FsEffectObservationError>;
    /// Checks for final evidence, registers this task when pending, and
    /// rechecks the snapshot to avoid missing a concurrent publication.
    ///
    /// # Parameters
    ///
    /// - `self`: Pinned future that retains the observer borrow.
    /// - `cx`: Task context used to register the current waker.
    ///
    /// # Returns
    ///
    /// `Ready` with the final fact or observation error, or `Pending` while no
    /// final fact has been published.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let observer = this.observer;
        match observer.state.snapshot.lock() {
            Err(_) => {
                unregister(this);
                Poll::Ready(Err(FsEffectObservationError::Poisoned))
            }
            Ok(snapshot) => match &*snapshot {
                FsEffectSnapshot::Final(fact) => {
                    let fact = fact.clone();
                    drop(snapshot);
                    unregister(this);
                    Poll::Ready(Ok(fact))
                }
                FsEffectSnapshot::Pending => {
                    drop(snapshot);
                    let registration_id = this.registration_id.get_or_insert_with(|| Arc::new(()));
                    if observer.state.register_waiter(registration_id, cx.waker()).is_err() {
                        unregister(this);
                        return Poll::Ready(Err(FsEffectObservationError::Poisoned));
                    }
                    match observer.state.snapshot.lock() {
                        Err(_) => {
                            unregister(this);
                            Poll::Ready(Err(FsEffectObservationError::Poisoned))
                        }
                        Ok(snapshot) => match &*snapshot {
                            FsEffectSnapshot::Final(fact) => {
                                let fact = fact.clone();
                                drop(snapshot);
                                unregister(this);
                                Poll::Ready(Ok(fact))
                            }
                            FsEffectSnapshot::Pending => Poll::Pending,
                        },
                    }
                }
            },
        }
    }
}

impl Drop for FsEffectWait<'_> {
    /// Removes the future's waiter registration when it is cancelled.
    ///
    /// # Parameters
    ///
    /// - `self`: Future whose registration is removed.
    fn drop(&mut self) {
        unregister(self);
    }
}

/// Removes the async waiter registration owned by `wait`, when present.
///
/// # Parameters
///
/// - `wait`: Future whose registration should be removed.
fn unregister(wait: &mut FsEffectWait<'_>) {
    if let Some(registration_id) = wait.registration_id.take() {
        wait.observer.state.unregister_waiter(&registration_id);
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::sync::Arc;
    use std::sync::Weak;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::task::Context;
    use std::task::Poll;
    use std::task::Waker;

    use crate::execution::FsEffectObservationError;
    use crate::execution::FsEffectSlotPool;

    struct ResourceWake(AtomicUsize);

    impl std::task::Wake for ResourceWake {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn pending_future_waker(
        observer: &super::super::FsEffectObserver,
    ) -> (std::pin::Pin<Box<super::FsEffectWait<'_>>>, Waker, Weak<ResourceWake>) {
        let wake = Arc::new(ResourceWake(AtomicUsize::new(0)));
        let weak = Arc::downgrade(&wake);
        let waker = Waker::from(Arc::clone(&wake));
        let mut future = Box::pin(observer.wait_final_async());
        {
            let mut context = Context::from_waker(&waker);
            assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
        }
        (future, waker, weak)
    }

    #[test]
    fn test_effect_wait_registration_poisoned_snapshot_unregisters() {
        let reservation = FsEffectSlotPool::new(1).reserve().unwrap();
        let observer = reservation.observer();
        let (mut future, waker, weak) = pending_future_waker(&observer);
        let shared = Arc::clone(&observer.state);
        assert!(
            std::thread::spawn(move || {
                let _snapshot = shared.snapshot.lock().unwrap();
                panic!("poison effect snapshot lock");
            })
            .join()
            .is_err()
        );
        drop(waker);

        let context_waker = Waker::noop();
        let mut context = Context::from_waker(context_waker);
        assert!(matches!(
            future.as_mut().poll(&mut context),
            Poll::Ready(Err(FsEffectObservationError::Poisoned))
        ));
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn test_effect_wait_registration_drop_recovers_poisoned_registry_lock() {
        let reservation = FsEffectSlotPool::new(1).reserve().unwrap();
        let observer = reservation.observer();
        let (future, waker, weak) = pending_future_waker(&observer);
        let shared = Arc::clone(&observer.state);
        assert!(
            std::thread::spawn(move || {
                let _wakers = shared.wakers.lock().unwrap();
                panic!("poison effect waker registry lock");
            })
            .join()
            .is_err()
        );
        drop(waker);

        drop(future);
        assert!(weak.upgrade().is_none());
    }
}
