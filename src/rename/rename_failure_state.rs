// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// facade tests.
//! Rename publication state at the point of provider failure.

/// Stable rename state for recovery decisions; error text is not a state
/// protocol.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::rename::RenameFailureState;
///
/// let state = RenameFailureState::Indeterminate;
/// let rename_may_have_completed = matches!(
///     state,
///     RenameFailureState::Renamed | RenameFailureState::Indeterminate
/// );
/// assert!(rename_may_have_completed);
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenameFailureState {
    /// Source and target were not changed.
    Unchanged,
    /// The rename completed before a later failure.
    Renamed,
    /// The provider cannot determine whether rename completed.
    Indeterminate,
}
