// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Provider-side synchronous file write sessions.

use qubit_io::Output;

use crate::{
    FsResult,
    WriteFailure,
    WriteOutcome,
};

/// Provider session underlying a concrete [`crate::FileWriter`] handle.
pub trait FileWriteSession: Output<Item = u8> + Send {
    /// Publishes bytes accepted by the session.
    ///
    /// A failed call must retain the session for retry or explicit cleanup.
    /// The returned [`WriteFailure`] identifies whether retry is safe and
    /// whether publication definitely occurred.
    ///
    /// # Returns
    /// Actual publication method and atomicity on success.
    ///
    /// # Errors
    /// Returns a filesystem error when publication cannot be confirmed.
    fn commit(&mut self) -> Result<WriteOutcome, WriteFailure>;

    /// Cancels the session and releases provider staging resources.
    ///
    /// # Errors
    /// Returns a filesystem error when cleanup cannot be confirmed.
    fn abort(&mut self) -> FsResult<()>;
}
