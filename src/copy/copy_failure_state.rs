// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Copy publication state at the point of provider failure.

/// Stable copy state for recovery decisions; error text is not a state protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CopyFailureState {
    /// No destination effect was published.
    Unchanged,
    /// Some destination effects were published.
    PartiallyPublished,
    /// The destination was published before a later failure.
    Published,
    /// The provider cannot determine publication state.
    Indeterminate,
}
