// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Rename operation outcome.

use crate::{AchievedAtomicity, NonSensitiveMetadata, Path, PublicationMethod, UserMetadata};

/// Outcome of a rename, move, or provider-equivalent publication.
#[derive(Clone, Debug, PartialEq)]
pub struct RenameOutcome {
    atomicity: AchievedAtomicity,
    method: PublicationMethod,
    source: Path,
    target: Path,
    diagnostics: NonSensitiveMetadata,
}

impl RenameOutcome {
    /// Creates a rename outcome with explicit successful semantics.
    ///
    /// # Parameters
    /// - `source`: Source identity supplied by the provider.
    /// - `target`: Target identity supplied by the provider.
    /// - `atomicity`: Atomicity actually achieved.
    /// - `method`: Method used to publish the destination.
    ///
    /// # Returns
    /// A rename outcome without diagnostics.
    #[inline]
    #[must_use]
    pub fn new(
        source: Path,
        target: Path,
        atomicity: AchievedAtomicity,
        method: PublicationMethod,
    ) -> Self {
        Self {
            atomicity,
            method,
            source,
            target,
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
    /// Returns the source identity.
    #[must_use]
    pub fn source(&self) -> &Path {
        &self.source
    }
    /// Returns the target identity.
    #[must_use]
    pub fn target(&self) -> &Path {
        &self.target
    }
    /// Returns actual publication atomicity.
    #[must_use]
    pub const fn atomicity(&self) -> AchievedAtomicity {
        self.atomicity
    }
    /// Returns the provider's publication method.
    #[must_use]
    pub const fn method(&self) -> PublicationMethod {
        self.method
    }
    /// Returns provider diagnostics that are safe to expose.
    #[must_use]
    pub const fn diagnostics(&self) -> &NonSensitiveMetadata {
        &self.diagnostics
    }
    /// Binds the facade-validated operation identities.
    pub(crate) fn with_identity(mut self, source: &Path, target: &Path) -> Self {
        self.source = source.clone();
        self.target = target.clone();
        self
    }
}
