// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_operation::PreparedExternalOperation;

use crate::FsOperationFailure;
use crate::execution::FsOwnedRenameRequest;
use crate::rename::RenameOutcome;

/// Object-safe prepared rename operation with request recovery before claim.
///
/// # Examples
///
/// ```
/// use qubit_fs::spi::PreparedFsRename;
///
/// fn recover_request(operation: &dyn PreparedFsRename) {
///     let _request = operation.take_unclaimed_request();
/// }
/// ```
pub trait PreparedFsRename: PreparedExternalOperation<RenameOutcome, FsOperationFailure> {
    /// Returns the original rename request while unclaimed.
    ///
    /// # Returns
    /// The request if it has not been claimed; otherwise `None`.
    fn take_unclaimed_request(&self) -> Option<FsOwnedRenameRequest>;

    /// Installs the single bounded effect reservation before submission.
    ///
    /// Implementations synchronize this one-shot slot: exactly one concurrent
    /// installation may succeed. The accepted reservation stays with the
    /// active work owner until it publishes one final typed fact and recovery
    /// responsibility is released. A duplicate or late installation returns
    /// the supplied reservation unchanged.
    ///
    /// # Parameters
    /// - `reservation`: Bounded effect reservation to install.
    ///
    /// # Returns
    /// `Ok(())` if installed, or the unchanged reservation if the slot was
    /// already installed or the operation is no longer accepting one.
    fn install_effect_reservation(
        &self,
        reservation: crate::execution::FsEffectReservation,
    ) -> Result<(), crate::execution::FsEffectReservation>;
}
