// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider-side synchronous writer sessions.

use qubit_io::Output;

use super::SpiWriteFailure;
use crate::FsResult;
use crate::WriteAbortOutcome;
use crate::WriteOutcome;

/// Provider writer session.
pub trait FileWriterSpi: Output<Item = u8> + Send {
    /// Publishes accepted bytes.
    ///
    /// # Returns
    /// The confirmed publication outcome.
    ///
    /// # Errors
    /// Returns provider-confirmed failure and recovery state when publication
    /// does not complete successfully.
    fn commit(&mut self) -> Result<WriteOutcome, SpiWriteFailure>;

    /// Releases provider staging resources.
    ///
    /// # Returns
    /// The provider-confirmed destination state after cleanup.
    ///
    /// # Errors
    /// Returns the provider cleanup failure with filesystem context.
    fn abort(&mut self) -> FsResult<WriteAbortOutcome>;
}
