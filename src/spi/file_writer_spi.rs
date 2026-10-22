// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider-side synchronous writer sessions.

use qubit_io::Output;

use super::SpiWriteFailure;
use crate::error::FsResult;
use crate::metadata::WriteOutcome;
use crate::write::WriteAbortOutcome;

/// Provider writer session.
///
/// # Examples
///
/// ```
/// use qubit_fs::spi::FileWriterSpi;
/// use qubit_fs::metadata::WriteOutcome;
/// use qubit_fs::spi::SpiWriteFailure;
///
/// fn publish<T: FileWriterSpi>(writer: &mut T) -> Result<WriteOutcome, SpiWriteFailure> {
///     writer.commit()
/// }
/// ```
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
