// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Cleanup facts for an isolated provider session.

/// Cleanup progress, independent of the failed operation's publication facts.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::error::RecoveryCleanupState;
///
/// assert!(matches!(RecoveryCleanupState::Pending, RecoveryCleanupState::Pending));
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryCleanupState {
    /// Explicit cleanup has not started.
    Pending,
    /// Explicit cleanup is in progress.
    Running,
    /// Provider cleanup completed and must not be repeated.
    Completed,
    /// Cleanup failed or was cancelled; retain ownership for reconciliation.
    Indeterminate,
}
