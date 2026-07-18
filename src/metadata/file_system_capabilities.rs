// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Filesystem capability guarantees.

use crate::{
    FileSystemCapability,
    FileSystemLimits,
};

/// Stable typed capability guarantees for one configured filesystem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileSystemCapabilities {
    flags: u128,
    limits: FileSystemLimits,
}

impl FileSystemCapabilities {
    /// Creates an empty capability set with configured limits.
    #[inline]
    #[must_use]
    pub const fn new(limits: FileSystemLimits) -> Self {
        Self { flags: 0, limits }
    }

    /// Returns a copy with one additional guaranteed capability.
    #[inline]
    #[must_use]
    pub const fn with(mut self, capability: FileSystemCapability) -> Self {
        self.flags |= capability.bit();
        self
    }

    /// Inserts one guaranteed capability.
    #[inline]
    pub fn insert(&mut self, capability: FileSystemCapability) {
        self.flags |= capability.bit();
    }

    /// Returns whether the filesystem guarantees `capability`.
    #[inline]
    #[must_use]
    pub const fn contains(&self, capability: FileSystemCapability) -> bool {
        self.flags & capability.bit() != 0
    }

    /// Returns the configured filesystem limits.
    #[inline]
    #[must_use]
    pub const fn limits(&self) -> &FileSystemLimits {
        &self.limits
    }
}

impl Default for FileSystemCapabilities {
    /// Creates an empty capability set with unknown limits.
    #[inline]
    fn default() -> Self {
        Self::new(FileSystemLimits::default())
    }
}
