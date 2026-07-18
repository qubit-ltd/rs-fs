// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Temporary resource lifecycle states.

/// Observable lifecycle and recovery state of a temporary resource handle.
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
    /// A target was published but source cleanup still belongs to this handle.
    CleanupRequired,
    /// The provider cannot determine the final source or target state.
    Indeterminate,
}
