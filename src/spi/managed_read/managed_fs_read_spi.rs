// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider preparation contract for owned reads.

use std::sync::Arc;

use crate::execution::FsOwnedReadRequest;
use crate::execution::FsPrepareFailure;
use crate::spi::PreparedFsRead;

/// Object-safe provider preparation interface for owned read requests.
///
/// # Examples
///
/// ```
/// use qubit_fs::execution::FsOwnedReadRequest;
/// use qubit_fs::spi::ManagedFsReadSpi;
///
/// fn prepare_read<T: ManagedFsReadSpi>(provider: &T, request: FsOwnedReadRequest) {
///     match provider.prepare_read_all(request) {
///         Ok(_prepared) => {}
///         Err(_failure) => {}
///     }
/// }
/// ```
pub trait ManagedFsReadSpi: Send + Sync {
    /// Reserves provider resources and prepares a read without starting I/O.
    ///
    /// # Parameters
    /// - `request`: Owned read request to prepare.
    ///
    /// # Returns
    /// A prepared read operation on success.
    ///
    /// # Errors
    /// Returns the preparation failure together with the original request.
    fn prepare_read_all(
        &self,
        request: FsOwnedReadRequest,
    ) -> Result<Arc<dyn PreparedFsRead>, FsPrepareFailure<FsOwnedReadRequest>>;
}
