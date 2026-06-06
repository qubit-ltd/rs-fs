// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Copy operation outcome.

use qubit_metadata::Metadata;

use crate::{
    CopyMethod,
    CopyStats,
};

/// Outcome returned by copy operations.
#[derive(Clone, Debug, PartialEq)]
pub struct CopyOutcome {
    /// Copy statistics.
    pub stats: CopyStats,
    /// Method used to complete the copy.
    pub method: CopyMethod,
    /// Provider-native diagnostics.
    pub diagnostics: Metadata,
}

impl CopyOutcome {
    /// Creates a copy outcome.
    ///
    /// # Parameters
    /// - `stats`: Copy statistics.
    /// - `method`: Method used to complete the copy.
    ///
    /// # Returns
    /// New copy outcome without diagnostics.
    #[inline]
    #[must_use]
    pub fn new(stats: CopyStats, method: CopyMethod) -> Self {
        Self {
            stats,
            method,
            diagnostics: Metadata::new(),
        }
    }
}
