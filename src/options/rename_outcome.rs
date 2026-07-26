// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Rename operation outcome.

use crate::{
    AchievedAtomicity,
    FsResult,
    NonSensitiveMetadata,
    PublicationMethod,
    UserMetadata,
};

/// Outcome of a rename, move, or provider-equivalent publication.
#[derive(Clone, Debug, PartialEq)]
pub struct RenameOutcome {
    /// Atomicity actually achieved by the operation.
    pub atomicity: AchievedAtomicity,
    /// Concrete method used to publish the destination.
    pub method: PublicationMethod,
    /// Provider-native non-sensitive diagnostics.
    pub diagnostics: NonSensitiveMetadata,
}

impl RenameOutcome {
    /// Creates a rename outcome with explicit successful semantics.
    ///
    /// # Parameters
    /// - `atomicity`: Atomicity actually achieved.
    /// - `method`: Method used to publish the destination.
    ///
    /// # Returns
    /// A rename outcome without diagnostics.
    #[inline]
    #[must_use]
    pub fn new(
        atomicity: AchievedAtomicity,
        method: PublicationMethod,
    ) -> Self {
        Self {
            atomicity,
            method,
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
