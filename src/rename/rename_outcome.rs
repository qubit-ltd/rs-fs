// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Rename operation outcome.

use crate::metadata::AchievedAtomicity;
use crate::metadata::NonSensitiveMetadata;
use crate::metadata::PublicationMethod;
use crate::metadata::UserMetadata;
use crate::path::Path;

/// Outcome of a rename, move, or provider-equivalent publication.
#[derive(Clone, Debug, PartialEq)]
pub struct RenameOutcome {
    /// Atomicity achieved while publishing the destination.
    atomicity: AchievedAtomicity,
    /// Actual publication mechanism used.
    method: PublicationMethod,
    /// Whether the provider synchronized durable destination publication.
    durable: bool,
    /// Source identity confirmed by the provider.
    source: Path,
    /// Destination identity confirmed by the provider.
    target: Path,
    /// Scrubbed provider diagnostics.
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
    pub fn new(source: Path, target: Path, atomicity: AchievedAtomicity, method: PublicationMethod) -> Self {
        Self {
            atomicity,
            method,
            durable: false,
            source,
            target,
            diagnostics: NonSensitiveMetadata::new(),
        }
    }

    /// Replaces the provider-reported durability completion fact.
    ///
    /// # Parameters
    ///
    /// - `durable`: Whether destination publication was synchronized.
    ///
    /// # Returns
    ///
    /// This outcome with its durability fact replaced.
    #[inline]
    #[must_use]
    pub fn with_durable(mut self, durable: bool) -> Self {
        self.durable = durable;
        self
    }

    /// Replaces provider-native diagnostics that have already passed key
    /// validation.
    #[inline]
    #[must_use]
    pub fn with_diagnostics(mut self, diagnostics: UserMetadata) -> Self {
        self.diagnostics = NonSensitiveMetadata::from(diagnostics);
        self
    }
    /// Returns the source identity.
    #[inline]
    #[must_use]
    pub fn source(&self) -> &Path {
        &self.source
    }
    /// Returns the target identity.
    #[inline(always)]
    #[must_use]
    pub fn target(&self) -> &Path {
        &self.target
    }
    /// Returns actual publication atomicity.
    #[inline(always)]
    #[must_use]
    pub const fn atomicity(&self) -> AchievedAtomicity {
        self.atomicity
    }
    /// Returns the provider's publication method.
    #[inline(always)]
    #[must_use]
    pub const fn method(&self) -> PublicationMethod {
        self.method
    }

    /// Returns whether the provider synchronized durable destination
    /// publication.
    #[inline(always)]
    #[must_use]
    pub const fn durable(&self) -> bool {
        self.durable
    }
    /// Returns provider diagnostics that are safe to expose.
    #[inline(always)]
    #[must_use]
    pub const fn diagnostics(&self) -> &NonSensitiveMetadata {
        &self.diagnostics
    }
}
