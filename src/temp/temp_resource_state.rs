// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Temporary resource lifecycle states.

/// Observable lifecycle and recovery state of a temporary resource handle.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::temp::TempResourceState;
///
/// let state = TempResourceState::Owned;
/// let description = match state {
///     TempResourceState::Owned => "cleanup is owned",
///     TempResourceState::Persisted => "source was published",
///     TempResourceState::Kept => "cleanup was released",
///     TempResourceState::Cleaned => "source was cleaned",
///     TempResourceState::CleanupRequired => "cleanup remains",
///     TempResourceState::Indeterminate => "source state is unknown",
/// };
/// assert_eq!(description, "cleanup is owned");
/// ```
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TempResourceState {
    /// The handle owns cleanup responsibility for an unpublished source.
    Owned,
    /// The source was successfully published to its final target.
    Persisted,
    /// Cleanup responsibility was explicitly released to the caller.
    Kept,
    /// The temporary source was explicitly cleaned.
    Cleaned,
    /// Only residual cleanup is permitted; this does not imply publication.
    CleanupRequired,
    /// The provider cannot determine the final source or target state.
    Indeterminate,
}
