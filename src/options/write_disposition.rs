// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Destination disposition for write operations.

/// How opening a writer treats an existing destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteDisposition {
    /// Create a new destination and fail if one already exists.
    CreateNew,
    /// Create a destination or replace its current contents.
    CreateOrReplace,
    /// Append bytes to an existing destination.
    Append,
}

impl Default for WriteDisposition {
    #[inline]
    fn default() -> Self {
        Self::CreateOrReplace
    }
}
