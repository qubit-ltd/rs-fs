// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow all -- facade integration tests exercise this API group.
//! Observable lifecycle state for an owning asynchronous copy operation.

use crate::CopyFailureState;

/// Stable lifecycle state for [`crate::AsyncCopyOperation`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AsyncCopyOperationState {
    /// The operation passed synchronous preflight and has not been polled.
    Ready,
    /// Provider I/O may be in progress.
    Running,
    /// A copy outcome was returned successfully.
    Completed,
    /// The operation failed with its confirmed publication state.
    Failed(
        /// Confirmed destination publication state at failure time.
        CopyFailureState,
    ),
}
