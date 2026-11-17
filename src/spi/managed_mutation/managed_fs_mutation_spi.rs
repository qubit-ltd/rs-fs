// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::Arc;

use super::PreparedFsCopy;
use super::PreparedFsRename;
use super::PreparedFsWrite;
use crate::execution::FsOwnedCopyRequest;
use crate::execution::FsOwnedRenameRequest;
use crate::execution::FsOwnedWriteRequest;
use crate::execution::FsPrepareFailure;

/// Object-safe provider preparation interface for owned mutations.
///
/// # Examples
///
/// ```
/// use qubit_fs::execution::FsOwnedWriteRequest;
/// use qubit_fs::spi::ManagedFsMutationSpi;
///
/// fn prepare_write<T: ManagedFsMutationSpi>(provider: &T, request: FsOwnedWriteRequest) {
///     let _preparation = provider.prepare_write(request);
/// }
/// ```
pub trait ManagedFsMutationSpi: Send + Sync {
    /// Prepares a write without starting I/O.
    ///
    /// # Parameters
    /// - `request`: Owned write request to prepare.
    ///
    /// # Returns
    /// A prepared write operation on success.
    ///
    /// # Errors
    /// Returns the preparation failure together with the original request.
    fn prepare_write(
        &self,
        request: FsOwnedWriteRequest,
    ) -> Result<Arc<dyn PreparedFsWrite>, FsPrepareFailure<FsOwnedWriteRequest>>;
    /// Prepares a copy without starting I/O.
    ///
    /// # Parameters
    /// - `request`: Owned copy request to prepare.
    ///
    /// # Returns
    /// A prepared copy operation on success.
    ///
    /// # Errors
    /// Returns the preparation failure together with the original request.
    fn prepare_copy(
        &self,
        request: FsOwnedCopyRequest,
    ) -> Result<Arc<dyn PreparedFsCopy>, FsPrepareFailure<FsOwnedCopyRequest>>;
    /// Prepares a rename without starting I/O.
    ///
    /// # Parameters
    /// - `request`: Owned rename request to prepare.
    ///
    /// # Returns
    /// A prepared rename operation on success.
    ///
    /// # Errors
    /// Returns the preparation failure together with the original request.
    fn prepare_rename(
        &self,
        request: FsOwnedRenameRequest,
    ) -> Result<Arc<dyn PreparedFsRename>, FsPrepareFailure<FsOwnedRenameRequest>>;
}
