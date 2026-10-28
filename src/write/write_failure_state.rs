// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Recovery states for failed synchronous writes.

/// Provider-confirmed recovery state when a write commit fails.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::write::WriteFailureState;
///
/// let state = WriteFailureState::RetryableNotPublished;
/// match state {
///     WriteFailureState::RetryableNotPublished => println!("retry the commit"),
///     WriteFailureState::NotPublished => println!("clean up the unpublished target"),
///     WriteFailureState::Published => println!("preserve the published target; clean up only the session or staging resources"),
///     WriteFailureState::Indeterminate => println!("resolve publication status"),
/// }
/// ```
///
/// `Published` means the destination must be preserved; do not retry
/// publication as though it had failed. Cleanup applies only to resources still
/// owned by the writer, such as a session or staging data. Successful cleanup
/// does not roll back the published target.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteFailureState {
    /// Publication can be retried using the retained session.
    RetryableNotPublished,
    /// The target was not published and only cleanup remains possible.
    NotPublished,
    /// The target was published and only cleanup remains possible.
    Published,
    /// The provider cannot determine whether publication occurred.
    Indeterminate,
}
