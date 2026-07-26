// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Rename operation outcome.

use crate::{AchievedAtomicity, NonSensitiveMetadata, PublicationMethod, UserMetadata};

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
    pub fn new(atomicity: AchievedAtomicity, method: PublicationMethod) -> Self {
        Self {
            atomicity,
            method,
            diagnostics: NonSensitiveMetadata::new(),
        }
    }

    /// Replaces provider-native diagnostics that have already passed key
    /// validation.
    #[inline]
    pub fn with_diagnostics(mut self, diagnostics: UserMetadata) -> Self {
        self.diagnostics = NonSensitiveMetadata::from(diagnostics);
        self
    }
}
