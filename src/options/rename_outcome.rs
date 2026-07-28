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
    NonSensitiveMetadata,
    Path,
    PublicationMethod,
    UserMetadata,
};

/// Outcome of a rename, move, or provider-equivalent publication.
#[derive(Clone, Debug, PartialEq)]
pub struct RenameOutcome {
    atomicity: AchievedAtomicity,
    method: PublicationMethod,
    source: Option<Path>,
    target: Option<Path>,
    diagnostics: NonSensitiveMetadata,
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
            source: None,
            target: None,
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
    /// Returns the source identity attached by the facade.
    #[must_use]
    pub fn source(&self) -> &Path {
        self.source
            .as_ref()
            .expect("facade must bind rename source")
    }
    /// Returns the target identity attached by the facade.
    #[must_use]
    pub fn target(&self) -> &Path {
        self.target
            .as_ref()
            .expect("facade must bind rename target")
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
    pub(crate) fn with_identity(
        mut self,
        source: &Path,
        target: &Path,
    ) -> Self {
        self.source = Some(source.clone());
        self.target = Some(target.clone());
        self
    }
}
