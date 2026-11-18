// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Async resource cleanup adapter for staged directories.

use std::sync::Arc;

use qubit_operation::AsyncCleanup;
use qubit_operation::CleanupContext;
use qubit_operation::CleanupFuture;
use qubit_operation::CleanupOutcome;

use super::super::StagedDirectory;

/// Async cleanup adapter for a staged-directory resource.
pub(in crate::spi::managed_tree) struct StagedDirectoryCleanup;

impl AsyncCleanup<StagedDirectory> for StagedDirectoryCleanup {
    /// Releases the staged directory and returns its owner if cleanup fails.
    ///
    /// # Parameters
    /// - `self`: Shared cleanup adapter retained by the resource scope.
    /// - `resource`: Staged-directory owner whose cleanup callback must run.
    /// - `_context`: Operation cleanup context supplied by the runtime.
    ///
    /// # Returns
    /// A future resolving to release success or a failure that retains the
    /// staged-directory owner for recovery.
    fn cleanup(self: Arc<Self>, resource: StagedDirectory, _context: CleanupContext) -> CleanupFuture<StagedDirectory> {
        Box::pin(async move {
            let resource_id = resource.cleanup_identity().wait().await;
            match resource.cleanup().await {
                Ok(()) => CleanupOutcome::Released,
                Err(error) => CleanupOutcome::Failed {
                    failure: resource.cleanup_failure(error, resource_id),
                    resource,
                },
            }
        })
    }
}
