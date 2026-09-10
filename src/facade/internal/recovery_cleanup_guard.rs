// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Marks interrupted cleanup attempts indeterminate without owning the session.

use crate::error::RecoveryCleanupState;

/// Borrows only cleanup state so dropping a future cannot drop its session.
pub(crate) struct RecoveryCleanupGuard<'a> {
    /// State owned by the persistent isolated recovery handle.
    state: &'a mut RecoveryCleanupState,
}
impl<'a> RecoveryCleanupGuard<'a> {
    /// Starts a polled cleanup attempt. Callers reject Completed before this
    /// call.
    pub(crate) fn start(state: &'a mut RecoveryCleanupState) -> Self {
        *state = RecoveryCleanupState::Running;
        Self { state }
    }
    /// Records whether the provider confirmed cleanup of its owned resources.
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
