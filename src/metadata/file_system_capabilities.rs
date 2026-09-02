// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Filesystem capability support.

use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use crate::metadata::FileSystemCapability;
use crate::metadata::FileSystemCapabilitySupport;

const CAPABILITY_DEPENDENCIES: &[(FileSystemCapability, FileSystemCapability)] = &[
    (FileSystemCapability::RangeRead, FileSystemCapability::Read),
    (FileSystemCapability::ConditionalRead, FileSystemCapability::Read),
    (FileSystemCapability::ChecksumValidation, FileSystemCapability::Read),
    (FileSystemCapability::Append, FileSystemCapability::Write),
    (FileSystemCapability::ConditionalWrite, FileSystemCapability::Write),
    (FileSystemCapability::AtomicReplace, FileSystemCapability::Write),
    (FileSystemCapability::DurableWrite, FileSystemCapability::Write),
    (FileSystemCapability::RecursiveDelete, FileSystemCapability::Delete),
    (FileSystemCapability::ConditionalDelete, FileSystemCapability::Delete),
    (FileSystemCapability::AtomicRename, FileSystemCapability::Rename),
    (FileSystemCapability::ServerSideCopy, FileSystemCapability::Copy),
    (FileSystemCapability::AtomicFileCopy, FileSystemCapability::Copy),
    (FileSystemCapability::AtomicTreeCopy, FileSystemCapability::Copy),
    (FileSystemCapability::DurableFileCopy, FileSystemCapability::Copy),
    (FileSystemCapability::DurableTreeCopy, FileSystemCapability::Copy),
    (FileSystemCapability::DurableRename, FileSystemCapability::Rename),
];

/// Stable typed capability support for one configured filesystem.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct FileSystemCapabilities {
    /// Capabilities that can be attempted conditionally.
    conditional: u128,
    /// Capabilities guaranteed for every valid request in this scope.
    guaranteed: u128,
}

impl FileSystemCapabilities {
    /// Creates an empty capability set.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            conditional: 0,
            guaranteed: 0,
        }
    }

    /// Returns a copy with one additional conditional capability.
    #[inline]
    #[must_use]
    pub const fn with_conditional(self, capability: FileSystemCapability) -> Self {
        self.set_support(capability, FileSystemCapabilitySupport::Conditional)
    }

    /// Returns a copy with one additional guaranteed capability.
    #[inline(always)]
    #[must_use]
    pub const fn with_guaranteed(self, capability: FileSystemCapability) -> Self {
        self.set_support(capability, FileSystemCapabilitySupport::Guaranteed)
    }

    /// Replaces the support status of one capability.
    #[inline]
    #[must_use]
    pub const fn set_support(mut self, capability: FileSystemCapability, support: FileSystemCapabilitySupport) -> Self {
        let bit = capability.bit();
        self.conditional &= !bit;
        self.guaranteed &= !bit;
        match support {
            FileSystemCapabilitySupport::Unsupported => {}
            FileSystemCapabilitySupport::Conditional => {
                self.conditional |= bit;
            }
            FileSystemCapabilitySupport::Guaranteed => {
                self.guaranteed |= bit;
            }
        }
        self
    }

    /// Returns the support status of `capability`.
    #[inline(always)]
    pub const fn support(&self, capability: FileSystemCapability) -> FileSystemCapabilitySupport {
        let bit = capability.bit();
        if self.guaranteed & bit != 0 {
            FileSystemCapabilitySupport::Guaranteed
        } else if self.conditional & bit != 0 {
            FileSystemCapabilitySupport::Conditional
        } else {
            FileSystemCapabilitySupport::Unsupported
        }
    }

    /// Returns whether the provider can attempt `capability`.
    #[inline(always)]
    #[must_use]
    pub const fn supports(&self, capability: FileSystemCapability) -> bool {
        !matches!(self.support(capability), FileSystemCapabilitySupport::Unsupported)
    }

    /// Returns whether `capability` is guaranteed in this filesystem scope.
    #[inline(always)]
    #[must_use]
    pub const fn guarantees(&self, capability: FileSystemCapability) -> bool {
        matches!(self.support(capability), FileSystemCapabilitySupport::Guaranteed)
    }

    /// Returns the number of advertised capabilities.
    ///
    /// # Returns
    /// Number of set capability flags.
    #[inline(always)]
    #[must_use]
    pub const fn len(&self) -> usize {
        (self.conditional | self.guaranteed).count_ones() as usize
    }

    /// Returns whether no capability is advertised.
    ///
    /// # Returns
    /// `true` when the set contains no capability.
    #[inline(always)]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.conditional == 0 && self.guaranteed == 0
    }

    /// Iterates advertised capabilities in stable discriminant order.
    ///
    /// # Returns
    /// An iterator over every capability contained in this set.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = FileSystemCapability> + '_ {
        FileSystemCapability::ALL
            .iter()
            .copied()
            .filter(|capability| self.supports(*capability))
    }

    /// Iterates advertised capabilities with their support status.
    #[inline]
    pub fn iter_with_support(&self) -> impl Iterator<Item = (FileSystemCapability, FileSystemCapabilitySupport)> + '_ {
        self.iter().map(|capability| (capability, self.support(capability)))
    }

    /// Returns the first advertised capability whose required base capability
    /// is absent.
    ///
    /// `None` means that every advertised derived capability has its required
    /// base capability. The returned pair contains the derived capability
    /// followed by the missing base capability.
    #[inline]
    #[must_use]
    pub fn missing_dependency(&self) -> Option<(FileSystemCapability, FileSystemCapability)> {
        CAPABILITY_DEPENDENCIES
            .iter()
            .copied()
            .find(|(capability, dependency)| self.supports(*capability) && !self.supports(*dependency))
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
        formatter.debug_map().entries(self.iter_with_support()).finish()
    }
}
