// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Copy source interpretation mode.

/// Copy source interpretation mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CopyMode {
    /// Copy a single file, object, or resource.
    File,
    /// Copy a directory tree, prefix tree, or collection subtree.
    Tree,
    /// Detect the source type before choosing file or tree copy.
    Auto,
}

impl Default for CopyMode {
    /// Uses automatic source type detection by default.
    #[inline]
    fn default() -> Self {
        Self::Auto
    }
}
