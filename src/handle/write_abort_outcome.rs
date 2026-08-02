// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Publication certainty after writer cancellation.

/// Provider-confirmed destination state after writer cleanup completes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[must_use]
pub enum WriteAbortOutcome {
    /// Cleanup completed and the destination was proven unchanged.
    NotPublished,
    /// Cleanup completed after the destination had already changed.
    Published,
    /// Cleanup completed, but the destination state remains unknown.
    Indeterminate,
}
