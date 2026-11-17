// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::future::Future;
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
        let observer = self.observer;
        match observer.state.snapshot.lock() {
            Err(_) => Poll::Ready(Err(FsEffectObservationError::Poisoned)),
            Ok(snapshot) => match &*snapshot {
                FsEffectSnapshot::Final(fact) => Poll::Ready(Ok(fact.clone())),
                FsEffectSnapshot::Pending => {
                    drop(snapshot);
                    let mut wakers = match observer.state.wakers.lock() {
                        Ok(wakers) => wakers,
                        Err(_) => {
                            return Poll::Ready(Err(FsEffectObservationError::Poisoned));
                        }
                    };
                    if !wakers.iter().any(|waker| waker.will_wake(cx.waker())) {
                        wakers.push(cx.waker().clone());
                    }
                    drop(wakers);
                    match observer.state.snapshot.lock() {
                        Err(_) => Poll::Ready(Err(FsEffectObservationError::Poisoned)),
                        Ok(snapshot) => match &*snapshot {
                            FsEffectSnapshot::Final(fact) => Poll::Ready(Ok(fact.clone())),
                            FsEffectSnapshot::Pending => Poll::Pending,
                        },
                    }
                }
            },
        }
    }
}
