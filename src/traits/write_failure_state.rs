// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Recovery states for failed synchronous writes.

/// Provider-confirmed recovery state when a write commit fails.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteFailureState {
    /// Publication can be retried using the retained session.
    Retryable,
    /// The target was not published and only cleanup remains possible.
    NotPublished,
    /// The target was published and only cleanup remains possible.
    Published,
    /// The provider cannot determine whether publication occurred.
    Indeterminate,
}
