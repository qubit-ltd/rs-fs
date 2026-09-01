// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider-side synchronous temporary-resource sessions.

use super::PersistRequest;
use super::SpiPersistFailure;
use crate::error::FsResult;
use crate::temp::PersistOutcome;

/// Provider temporary-resource lifecycle session.
pub trait TempResourceSpi: Send {
    /// Persists a temporary resource.
    ///
    /// # Parameters
    /// - `request`: Validated target and persistence requirements.
    ///
    /// # Returns
    /// The confirmed persistence outcome.
    ///
    /// # Errors
    /// Returns provider-confirmed failure and recovery state when persistence
    /// does not complete successfully.
    fn persist(&mut self, request: PersistRequest<'_>) -> Result<PersistOutcome, SpiPersistFailure>;

    /// Publishes the temporary resource to a provider-generated target.
    ///
    /// # Returns
    /// The confirmed publication outcome.
    ///
    /// # Errors
    /// Returns the provider-confirmed failure and recovery state when
    /// publication cannot be completed.
    fn keep(&mut self) -> Result<PersistOutcome, SpiPersistFailure>;

    /// Cleans the temporary source.
    ///
    /// # Returns
    /// `Ok(())` after cleanup is confirmed.
    ///
    /// # Errors
    /// Returns the provider cleanup failure with filesystem context.
    fn cleanup(&mut self) -> FsResult<()>;
}
