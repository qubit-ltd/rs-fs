// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Portable symbolic-link resolution policies.

/// Controls whether a filesystem may resolve symbolic links while operating
/// within its configured namespace.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[must_use]
#[non_exhaustive]
pub enum SymlinkPolicy {
    /// Reject an operation that requires symbolic-link resolution.
    #[default]
    Reject,
    /// Follow symbolic links only when the resolved target remains inside the
    /// configured filesystem namespace.
    FollowWithinFileSystem,
}

impl SymlinkPolicy {
    /// Returns whether this policy permits symbolic-link resolution.
    #[inline(always)]
    #[must_use]
    pub const fn follows(self) -> bool {
        matches!(self, Self::FollowWithinFileSystem)
    }
}
