// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider-side synchronous file write sessions.

use qubit_io::Output;

use crate::{
    FsResult,
    WriteOutcome,
};

/// Provider session underlying a concrete [`crate::FileWriter`] handle.
pub trait FileWriteSession: Output<Item = u8> + Send {
    /// Publishes bytes accepted by the session.
    ///
    /// A failed call must leave the session available to the caller. The
    /// returned error should use [`crate::FsErrorKind::Indeterminate`] when the
    /// provider cannot determine whether publication occurred.
    ///
    /// # Returns
    /// Actual publication method and atomicity on success.
    ///
    /// # Errors
    /// Returns a filesystem error when publication cannot be confirmed.
    fn commit(&mut self) -> FsResult<WriteOutcome>;

    /// Cancels the session and releases provider staging resources.
    ///
    /// # Errors
    /// Returns a filesystem error when cleanup cannot be confirmed.
    fn abort(&mut self) -> FsResult<()>;
}
