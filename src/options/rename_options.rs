// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Rename operation options.

use crate::AtomicityRequirement;

/// Options controlling rename operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenameOptions {
    /// Whether the destination may be overwritten.
    pub overwrite: bool,
    /// Required atomicity level.
    pub atomic: AtomicityRequirement,
}

impl Default for RenameOptions {
    #[inline]
    fn default() -> Self {
        Self {
            overwrite: false,
            atomic: AtomicityRequirement::BestEffort,
        }
    }
}
