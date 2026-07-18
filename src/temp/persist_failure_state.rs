// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Partial-progress states for failed temporary persistence.

/// Provider-confirmed progress when a persist call does not fully complete.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistFailureState {
    /// The target was not published and the handle still owns the source.
    NotPublished,
    /// The target was published, but the handle still owns source cleanup.
    PublishedSourceRetained,
    /// The provider cannot determine the final target or source state.
    Indeterminate,
}
