// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider-side asynchronous temporary-resource sessions.

use std::pin::Pin;

use super::PersistRequest;
use super::SpiFuture;
use super::SpiPersistFailure;
use crate::error::FsResult;
use crate::temp::PersistOutcome;

/// Provider-side asynchronous temporary-resource lifecycle session.
pub trait AsyncTempResourceSpi: Send {
    /// Performs provider-local cancellation when the facade handle is dropped.
    ///
    /// This hook must be nonblocking: it must not start asynchronous work,
    /// block the current thread, or claim that remote cleanup has completed.
    /// Providers may use it to release local descriptors or enqueue cleanup
    /// through an already-running mechanism. The default implementation is a
    /// no-op for providers that do not need local cancellation.
    #[inline]
    fn cancel_on_drop(self: Pin<&mut Self>) {}

    /// Asynchronously confirms provider cleanup.
    ///
    /// The returned future performs provider I/O when polled. Dropping it
    /// before completion does not confirm cleanup.
    ///
    /// # Returns
    /// A runtime-neutral future resolving after cleanup is confirmed.
    ///
    /// # Errors
    /// Resolves to the provider cleanup failure, including an indeterminate
    /// error when the provider cannot determine whether cleanup completed.
    fn cleanup<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, FsResult<()>>;

    /// Asynchronously publishes this resource to a provider-generated target.
    ///
    /// The returned future performs provider I/O when polled. Dropping it
    /// before completion does not transfer cleanup responsibility.
    ///
    /// # Returns
    /// A runtime-neutral future resolving to the confirmed publication
    /// outcome.
    ///
    /// # Errors
    /// Resolves to the provider-confirmed failure and recovery state when
    /// publication cannot be completed.
    fn keep<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, Result<PersistOutcome, SpiPersistFailure>>;

    /// Asynchronously persists this resource to a validated target.
    ///
    /// The returned future performs provider I/O when polled. Cancellation may
    /// leave publication state indeterminate, which implementations must
    /// report through [`SpiPersistFailure`].
    ///
    /// # Parameters
    /// - `request`: Validated target and persistence requirements.
    ///
    /// # Returns
    /// A runtime-neutral future resolving to the confirmed persistence
    /// outcome.
    ///
    /// # Errors
    /// Resolves to a typed provider failure preserving confirmed publication
    /// progress.
    fn persist<'a>(
        self: Pin<&'a mut Self>,
        request: PersistRequest<'a>,
    ) -> SpiFuture<'a, Result<PersistOutcome, SpiPersistFailure>>;
}
