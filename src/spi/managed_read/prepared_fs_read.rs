// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Prepared owned read operation contract.

use qubit_operation::PreparedExternalOperation;

use crate::FsOperationFailure;
use crate::execution::FsOwnedReadRequest;

/// Prepared read accepted by the managed operation framework.
///
/// # Examples
///
/// ```
/// use qubit_fs::spi::PreparedFsRead;
///
/// fn recover_request(operation: &dyn PreparedFsRead) {
///     if let Some(request) = operation.take_unclaimed_request() {
///         drop(request);
///     }
/// }
/// ```
pub trait PreparedFsRead: PreparedExternalOperation<Vec<u8>, FsOperationFailure> {
    /// Returns the original request while the prepared operation remains
    /// unclaimed.
    ///
    /// # Returns
    /// The request if it has not been claimed; otherwise `None`.
    fn take_unclaimed_request(&self) -> Option<FsOwnedReadRequest>;
}
