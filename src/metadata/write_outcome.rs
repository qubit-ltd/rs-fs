// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Write operation outcome.

use crate::AchievedAtomicity;
use crate::NonSensitiveMetadata;
use crate::PublicationMethod;
use crate::ResourceVersion;
use crate::UserMetadata;

/// Outcome returned when a writer is committed.
#[derive(Clone, Debug, PartialEq)]
pub struct WriteOutcome {
    /// Number of bytes written when known.
    bytes_written: Option<u64>,
    /// Provider version, generation, or ETag when known.
    version: Option<ResourceVersion>,
    /// Atomicity actually achieved by publication.
    atomicity: AchievedAtomicity,
    /// Concrete publication method that completed the write.
    method: PublicationMethod,
    /// Provider-native non-sensitive diagnostics.
    diagnostics: NonSensitiveMetadata,
}

impl WriteOutcome {
    /// Creates a write outcome with explicit publication semantics.
    ///
    /// # Parameters
    /// - `atomicity`: Atomicity actually achieved.
    /// - `method`: Method used to publish the resource.
    ///
    /// # Returns
    /// A write outcome with no byte count, version, or diagnostics.
    #[inline]
    #[must_use]
    pub fn new(atomicity: AchievedAtomicity, method: PublicationMethod) -> Self {
        Self {
            bytes_written: None,
            version: None,
            atomicity,
            method,
            diagnostics: NonSensitiveMetadata::new(),
        }
    }

    /// Returns the number of bytes accepted by the write session, when known.
    #[inline(always)]
    #[must_use]
    pub const fn bytes_written(&self) -> Option<u64> {
        self.bytes_written
    }

    /// Returns the provider version, generation, or ETag when known.
    #[inline(always)]
    #[must_use]
    pub const fn version(&self) -> Option<&ResourceVersion> {
        self.version.as_ref()
    }

    /// Returns the atomicity actually achieved by publication.
    #[inline(always)]
    #[must_use]
    pub const fn atomicity(&self) -> AchievedAtomicity {
        self.atomicity
    }

    /// Returns the concrete method that published the resource.
    #[inline(always)]
    #[must_use]
    pub const fn method(&self) -> PublicationMethod {
        self.method
    }

    /// Returns provider-native non-sensitive diagnostics.
    #[inline(always)]
    #[must_use]
    pub const fn diagnostics(&self) -> &NonSensitiveMetadata {
        &self.diagnostics
    }

    /// Records the byte count confirmed by the provider.
    #[inline]
    #[must_use]
    pub const fn with_bytes_written(mut self, bytes_written: u64) -> Self {
        self.bytes_written = Some(bytes_written);
        self
    }

    /// Records the provider version, generation, or ETag.
    #[inline]
    #[must_use]
    pub fn with_version(mut self, version: ResourceVersion) -> Self {
        self.version = Some(version);
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
}
