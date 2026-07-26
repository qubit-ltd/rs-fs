// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Filesystem capability guarantees.

use crate::FileSystemCapability;

const CAPABILITY_DEPENDENCIES: &[(FileSystemCapability, FileSystemCapability)] = &[
    (FileSystemCapability::RangeRead, FileSystemCapability::Read),
    (
        FileSystemCapability::ConditionalRead,
        FileSystemCapability::Read,
    ),
    (
        FileSystemCapability::ChecksumValidation,
        FileSystemCapability::Read,
    ),
    (FileSystemCapability::Append, FileSystemCapability::Write),
    (
        FileSystemCapability::ConditionalWrite,
        FileSystemCapability::Write,
    ),
    (
        FileSystemCapability::AtomicReplace,
        FileSystemCapability::Write,
    ),
    (
        FileSystemCapability::RecursiveDelete,
        FileSystemCapability::Delete,
    ),
    (
        FileSystemCapability::ConditionalDelete,
        FileSystemCapability::Delete,
    ),
    (
        FileSystemCapability::AtomicRename,
        FileSystemCapability::Rename,
    ),
    (
        FileSystemCapability::ServerSideCopy,
        FileSystemCapability::Copy,
    ),
];

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

    /// Returns the first advertised capability whose required base capability is absent.
    ///
    /// `None` means that every advertised derived capability has its required base
    /// capability. The returned pair contains the derived capability followed by the
    /// missing base capability.
    #[must_use]
    pub fn missing_dependency(&self) -> Option<(FileSystemCapability, FileSystemCapability)> {
        CAPABILITY_DEPENDENCIES
            .iter()
            .copied()
            .find(|(capability, dependency)| {
                self.contains(*capability) && !self.contains(*dependency)
            })
    }
}

impl Default for FileSystemCapabilities {
    /// Creates an empty capability set.
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
