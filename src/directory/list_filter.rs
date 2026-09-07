// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Explicit directory and object-key listing filters.
/// Listing selection mode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListFilter {
    /// Select a path and its descendants using component boundaries.
    Subtree(String),
    /// Select keys by their raw string prefix.
    LiteralPrefix(String),
}
