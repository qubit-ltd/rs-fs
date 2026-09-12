// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================

//! Optional provider optimization for bounded prefix reads.

/// A non-binding upper bound for bytes consumed by one prefix open.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrefixReadHint {
    max_bytes: u64,
}

impl PrefixReadHint {
    /// Creates a hint for a positive prefix size.
    #[inline]
    pub(crate) const fn new(max_bytes: u64) -> Self {
        Self { max_bytes }
    }

    /// Returns the suggested maximum prefix size.
    #[inline]
    #[must_use]
    pub const fn max_bytes(self) -> u64 {
        self.max_bytes
    }
}
