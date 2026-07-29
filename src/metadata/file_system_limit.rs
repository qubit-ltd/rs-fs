// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow all -- facade integration tests exercise this API group.
//! A single configured filesystem limit.

/// A provider-declared limit for one filesystem property.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FileSystemLimit {
    /// The provider cannot report a stable limit at construction time.
    Unknown,
    /// The limit dimension does not apply to this provider.
    NotApplicable,
    /// The provider imposes no finite limit at the `rs-fs` API layer.
    Unbounded,
    /// The inclusive maximum accepted value.
    Maximum(u64),
}

impl FileSystemLimit {
    /// Returns the finite inclusive maximum, when one exists.
    #[inline(always)]
    #[must_use]
    pub const fn maximum(self) -> Option<u64> {
        match self {
            Self::Maximum(maximum) => Some(maximum),
            Self::Unknown | Self::NotApplicable | Self::Unbounded => None,
        }
    }

    /// Returns whether `actual` exceeds a declared finite maximum.
    #[inline(always)]
    #[must_use]
    pub const fn is_exceeded_by(self, actual: u64) -> bool {
        matches!(self, Self::Maximum(maximum) if actual > maximum)
    }
}
