// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Write creation mode.

/// Write creation mode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WriteMode {
    /// Create a new resource and fail when the target exists.
    CreateNew,
    /// Create or truncate an existing resource.
    CreateOrTruncate,
    /// Append to an existing resource.
    Append,
    /// Replace the target atomically.
    ReplaceAtomic,
    /// Replace only when the target version matches.
    ConditionalReplace {
        /// Required target ETag or provider version.
        etag: String,
    },
}

impl Default for WriteMode {
    /// Creates or truncates by default.
    #[inline]
    fn default() -> Self {
        Self::CreateOrTruncate
    }
}
