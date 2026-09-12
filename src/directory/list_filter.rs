// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Explicit directory and object-key listing filters.

/// Listing selection mode.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::directory::ListFilter;
///
/// let filter = ListFilter::Subtree("reports/2026".to_owned());
/// assert!(matches!(filter, ListFilter::Subtree(_)));
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListFilter {
    /// Select a path and its descendants using component boundaries.
    Subtree(String),
    /// Select keys by their raw string prefix.
    LiteralPrefix(String),
}
