// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Write operation outcome.

use qubit_metadata::Metadata;

use crate::{
    AchievedAtomicity,
    FsOperation,
    FsResult,
    NonSensitiveMetadata,
    PublicationMethod,
    ResourceVersion,
};

/// Outcome returned when a writer is committed.
#[derive(Clone, Debug, PartialEq)]
pub struct WriteOutcome {
    /// Number of bytes written when known.
    pub bytes_written: Option<u64>,
    /// Provider version, generation, or ETag when known.
    pub version: Option<ResourceVersion>,
    /// Atomicity actually achieved by publication.
    pub atomicity: AchievedAtomicity,
    /// Concrete publication method that completed the write.
    pub method: PublicationMethod,
    /// Provider-native non-sensitive diagnostics.
    pub diagnostics: NonSensitiveMetadata,
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
    pub fn new(
        atomicity: AchievedAtomicity,
        method: PublicationMethod,
    ) -> Self {
        Self {
            bytes_written: None,
            version: None,
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
    pub fn with_diagnostics(mut self, diagnostics: Metadata) -> FsResult<Self> {
        self.diagnostics = NonSensitiveMetadata::try_from_with_context(
            diagnostics,
            FsOperation::CommitWriter,
            "credential-like write diagnostic keys are forbidden",
        )?;
        Ok(self)
    }
}
