// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Partial-progress states for failed temporary persistence.

/// Provider-confirmed progress when a persist call does not fully complete.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
/// use qubit_fs::temp::{PersistFailure, PersistFailureState};
///
/// let failure = PersistFailure::new(
///     FsError::new(FsErrorKind::Io, FsOperation::PersistTemp, "failed"),
///     PersistFailureState::NotPublished,
/// );
/// assert_eq!(PersistFailureState::NotPublished, failure.state());
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistFailureState {
    /// The target was not published and the handle still owns the source.
    NotPublished,
    /// The target was not published, but source mutation authority is
    /// uncertain.
    NotPublishedSourceIndeterminate,
    /// The target was not published and only residual cleanup is permitted.
    NotPublishedSourceCleanupRequired,
    /// The target was not published and the handle released its source.
    NotPublishedSourceReleased,
    /// The target was published, but the handle still owns source cleanup.
    PublishedSourceRetained,
    /// The target was published, but source mutation authority is uncertain.
    PublishedSourceIndeterminate,
    /// The target was published and the handle released its source.
    PublishedSourceReleased,
    /// The provider cannot determine the final target or source state.
    Indeterminate,
}
