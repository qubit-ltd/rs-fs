// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Directory listing options.

/// Options controlling directory or prefix listing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ListOptions {
    /// Whether listing should recurse into child containers.
    pub recursive: bool,
    /// Whether symbolic links should be followed.
    pub follow_symlinks: bool,
    /// Whether entries should include metadata when available.
    pub include_metadata: bool,
    /// Optional provider page size hint.
    pub page_size: Option<usize>,
    /// Optional lexical prefix filter relative to the requested list root.
    ///
    /// The filter uses canonical `/`-separated relative paths. For example,
    /// listing `/root` with `prefix: Some("nested/item")` matches
    /// `/root/nested/item`, while `prefix: Some("item")` only matches an
    /// immediate child named `item`.
    pub prefix: Option<String>,
}
