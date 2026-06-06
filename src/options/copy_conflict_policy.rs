// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Copy conflict policy.

/// Conflict policy for existing destination entries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CopyConflictPolicy {
    /// Fail when a destination entry exists.
    Fail,
    /// Overwrite existing destination entries.
    Overwrite,
    /// Skip existing destination entries.
    Skip,
}

impl Default for CopyConflictPolicy {
    /// Fails on destination conflicts by default.
    #[inline]
    fn default() -> Self {
        Self::Fail
    }
}
