// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Marks interrupted cleanup attempts indeterminate without owning the session.

use crate::error::RecoveryCleanupState;

/// Borrows only cleanup state so dropping a future cannot drop its session.
#[must_use = "the cleanup guard must remain alive until the attempt finishes"]
pub(crate) struct RecoveryCleanupGuard<'a> {
    /// State owned by the persistent isolated recovery handle.
    state: &'a mut RecoveryCleanupState,
}
impl<'a> RecoveryCleanupGuard<'a> {
    /// Starts a polled cleanup attempt. Callers reject Completed before this
    /// call.
    ///
    /// # Parameters
    ///
    /// - `state`: Persistent cleanup state updated while the attempt runs.
    ///
    /// # Returns
    ///
    /// A guard that marks an unfinished attempt indeterminate when dropped.
    pub(crate) fn start(state: &'a mut RecoveryCleanupState) -> Self {
        *state = RecoveryCleanupState::Running;
        Self { state }
    }
    /// Records whether the provider confirmed cleanup of its owned resources.
    ///
    /// # Parameters
    ///
    /// - `confirmed`: Whether the provider confirmed that cleanup completed.
    pub(crate) fn finish(&mut self, confirmed: bool) {
        *self.state = if confirmed {
            RecoveryCleanupState::Completed
        } else {
            RecoveryCleanupState::Indeterminate
        };
    }
}
impl Drop for RecoveryCleanupGuard<'_> {
    /// Cancellation or panic leaves the persistent session available for
    /// recovery.
    fn drop(&mut self) {
        if *self.state == RecoveryCleanupState::Running {
            *self.state = RecoveryCleanupState::Indeterminate;
        }
    }
}
