// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Checksum policy for read operations.

/// Checksum behavior requested by a read operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChecksumPolicy {
    /// Do not require checksum validation.
    None,
    /// Validate checksums when the provider can do so cheaply.
    BestEffort,
    /// Require checksum validation or fail.
    Required,
}

impl Default for ChecksumPolicy {
    /// Does not require checksum validation by default.
    #[inline]
    fn default() -> Self {
        Self::None
    }
}
