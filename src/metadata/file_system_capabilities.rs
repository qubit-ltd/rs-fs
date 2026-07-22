// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Filesystem capability guarantees.

use crate::FileSystemCapability;

/// Stable typed capability guarantees for one configured filesystem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileSystemCapabilities {
    flags: u128,
}

impl FileSystemCapabilities {
    /// Creates an empty capability set.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self { flags: 0 }
    }

    /// Returns a copy with one additional guaranteed capability.
    #[inline]
    #[must_use]
    pub const fn with(mut self, capability: FileSystemCapability) -> Self {
        self.flags |= capability.bit();
        self
    }

    /// Inserts one guaranteed capability.
    #[inline(always)]
    pub fn insert(&mut self, capability: FileSystemCapability) {
        self.flags |= capability.bit();
    }

    /// Returns whether the filesystem guarantees `capability`.
    #[inline(always)]
    #[must_use]
    pub const fn contains(&self, capability: FileSystemCapability) -> bool {
        self.flags & capability.bit() != 0
    }
}

impl Default for FileSystemCapabilities {
    /// Creates an empty capability set.
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
