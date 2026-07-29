// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow all -- facade integration tests exercise this API group.
//! Successful temporary resource persistence outcome.

use crate::{AchievedAtomicity, NonSensitiveMetadata, Path, PublicationMethod, UserMetadata};

/// Confirmed result of publishing a temporary source to its final target.
#[derive(Clone, Debug, PartialEq)]
pub struct PersistOutcome {
    /// Final provider-local target path.
    pub target: Path,
    /// Atomicity actually achieved by publication.
    pub atomicity: AchievedAtomicity,
    /// Concrete publication method used.
    pub method: PublicationMethod,
    /// Provider-native non-sensitive diagnostics.
    pub diagnostics: NonSensitiveMetadata,
}

impl PersistOutcome {
    /// Creates a confirmed persistence outcome.
    ///
    /// # Parameters
    /// - `target`: Final target path.
    /// - `atomicity`: Atomicity actually achieved.
    /// - `method`: Method used to publish the target.
    ///
    /// # Returns
    /// An outcome without provider diagnostics.
    #[inline]
    #[must_use]
    pub fn new(target: Path, atomicity: AchievedAtomicity, method: PublicationMethod) -> Self {
        Self {
            target,
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
