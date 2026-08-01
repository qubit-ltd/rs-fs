// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Filesystem capability guarantees.

use std::fmt::{
    Debug,
    Formatter,
    Result as FmtResult,
};

use crate::FileSystemCapability;

const CAPABILITY_DEPENDENCIES: &[(
    FileSystemCapability,
    FileSystemCapability,
)] = &[
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
    (FileSystemCapability::DurableCopy, FileSystemCapability::Copy),
];

/// Stable typed capability guarantees for one configured filesystem.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct FileSystemCapabilities {
    /// Bit set indexed by stable [`FileSystemCapability`] discriminants.
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

    /// Returns the number of advertised capabilities.
    ///
    /// # Returns
    /// Number of set capability flags.
    #[inline(always)]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.flags.count_ones() as usize
    }

    /// Returns whether no capability is advertised.
    ///
    /// # Returns
    /// `true` when the set contains no capability.
    #[inline(always)]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.flags == 0
    }

    /// Iterates advertised capabilities in stable discriminant order.
    ///
    /// # Returns
    /// An iterator over every capability contained in this set.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = FileSystemCapability> + '_ {
        FileSystemCapability::ALL
            .into_iter()
            .filter(|capability| self.contains(*capability))
    }

    /// Returns the first advertised capability whose required base capability
    /// is absent.
    ///
    /// `None` means that every advertised derived capability has its required
    /// base capability. The returned pair contains the derived capability
    /// followed by the missing base capability.
    #[inline]
    #[must_use]
    pub fn missing_dependency(
        &self,
    ) -> Option<(FileSystemCapability, FileSystemCapability)> {
        CAPABILITY_DEPENDENCIES.iter().copied().find(
            |(capability, dependency)| {
                self.contains(*capability) && !self.contains(*dependency)
            },
        )
    }
}

impl Default for FileSystemCapabilities {
    /// Creates an empty capability set.
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Debug for FileSystemCapabilities {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.debug_set().entries(self.iter()).finish()
    }
}
