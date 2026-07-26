// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Copy operation outcome.

use crate::{
    AchievedAtomicity,
    CopyMethod,
    CopyStats,
    FsResult,
    NonSensitiveMetadata,
    UserMetadata,
};

/// Outcome returned by copy operations.
#[derive(Clone, Debug, PartialEq)]
pub struct CopyOutcome {
    /// Copy statistics.
    pub stats: CopyStats,
    /// Method used to complete the copy.
    pub method: CopyMethod,
    /// Atomicity actually achieved when publishing the destination.
    pub atomicity: AchievedAtomicity,
    /// Provider-native non-sensitive diagnostics.
    pub diagnostics: NonSensitiveMetadata,
}

impl CopyOutcome {
    /// Creates a copy outcome.
    ///
    /// # Parameters
    /// - `stats`: Copy statistics.
    /// - `method`: Method used to complete the copy.
    /// - `atomicity`: Atomicity achieved while publishing the destination.
    ///
    /// # Returns
    /// New copy outcome without diagnostics.
    #[inline]
    #[must_use]
    pub fn new(
        stats: CopyStats,
        method: CopyMethod,
        atomicity: AchievedAtomicity,
    ) -> Self {
        Self {
            stats,
            method,
            atomicity,
            diagnostics: NonSensitiveMetadata::new(),
        }
    }

    /// Replaces the provider-native diagnostics after validating their keys.
    ///
    /// # Errors
    ///
    /// Returns an invalid-options error when a top-level key or a key nested
    /// in a string map or JSON object resembles credential material.
    #[inline]
    pub fn with_diagnostics(
        mut self,
        diagnostics: UserMetadata,
    ) -> FsResult<Self> {
        self.diagnostics = NonSensitiveMetadata::from(diagnostics);
        Ok(self)
    }
}
