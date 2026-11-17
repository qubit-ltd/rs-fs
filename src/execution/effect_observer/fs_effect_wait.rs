// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::future::Future;
use std::sync::Arc;
use std::pin::Pin;
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
                    let registration_id = this
                        .registration_id
                        .get_or_insert_with(|| Arc::new(()));
                    if observer
                        .state
                        .register_waiter(registration_id, cx.waker())
                        .is_err()
                    {
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
    fn drop(&mut self) {
        unregister(self);
    }
}

fn unregister(wait: &mut FsEffectWait<'_>) {
    if let Some(registration_id) = wait.registration_id.take() {
        wait.observer
            .state
            .unregister_waiter(&registration_id);
    }
}
