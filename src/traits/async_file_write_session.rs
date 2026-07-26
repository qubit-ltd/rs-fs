// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Provider-side asynchronous file write sessions.

use std::pin::Pin;

use qubit_io::AsyncOutput;

use crate::{FsFuture, WriteOutcome};

/// Provider session underlying a concrete [`crate::AsyncFileWriter`] handle.
pub trait AsyncFileWriteSession: AsyncOutput<Item = u8> + Send {
    /// Asynchronously publishes bytes accepted by the session.
    ///
    /// # Returns
    /// A future resolving to the actual publication method and atomicity.
    fn commit_async<'a>(self: Pin<&'a mut Self>) -> FsFuture<'a, WriteOutcome>;

    /// Asynchronously cancels and cleans up this write session.
    ///
    /// # Returns
    /// A future resolving when cleanup is confirmed.
    fn abort_async<'a>(self: Pin<&'a mut Self>) -> FsFuture<'a, ()>;

    /// Performs nonblocking local cancellation during writer drop.
    ///
    /// The default does nothing. Implementations must not start or block an
    /// executor, wait for network I/O, or claim remote cleanup completed.
    ///
    /// # Parameters
    /// - `self`: Pinned provider session being abandoned.
    #[inline]
    fn cancel_on_drop(self: Pin<&mut Self>) {}
}
