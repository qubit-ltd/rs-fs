// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Prepared staged-tree publication contract.

use qubit_operation::PreparedExternalOperation;

use crate::FsOperationFailure;
use crate::directory::TreePublishOutcome;
use crate::execution::FsOwnedTreePublishRequest;

/// Object-safe prepared staged-tree publication operation.
///
/// # Examples
///
/// ```
/// use qubit_fs::spi::PreparedFsTreePublish;
///
/// fn recover_request(operation: &dyn PreparedFsTreePublish) {
///     if let Some(request) = operation.take_unclaimed_request() {
///         drop(request);
///     }
/// }
/// ```
pub trait PreparedFsTreePublish: PreparedExternalOperation<TreePublishOutcome, FsOperationFailure> {
    /// Returns the original tree request while unclaimed.
    ///
    /// # Returns
    /// The request if it has not been claimed; otherwise `None`.
    fn take_unclaimed_request(&self) -> Option<FsOwnedTreePublishRequest>;

    /// Installs the bounded mutation fact slot before provider work is
    /// activated. The provider retains it until it publishes the final
    /// typed tree fact and releases any publication or recovery owner.
    ///
    /// # Parameters
    /// - `reservation`: Bounded mutation fact reservation to install.
    ///
    /// # Returns
    /// `Ok(())` if installed, or the unchanged reservation when the slot is
    /// already installed or the operation no longer accepts one.
    fn install_effect_reservation(
        &self,
        reservation: crate::execution::FsEffectReservation,
    ) -> Result<(), crate::execution::FsEffectReservation>;
}
