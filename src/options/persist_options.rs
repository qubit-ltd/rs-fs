// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Temporary resource persistence options.

use crate::{
    AtomicityRequirement,
    FileSystemCapabilities,
    FileSystemCapability,
    FsError,
    FsErrorKind,
    FsOperation,
    MetadataPreservePolicy,
};

/// Options controlling temporary resource persistence.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistOptions {
    /// Whether the destination may be overwritten.
    overwrite: bool,
    /// Required atomicity level.
    atomicity: AtomicityRequirement,
    /// Metadata preservation policy.
    preserve_metadata: MetadataPreservePolicy,
}

impl PersistOptions {
    /// Returns whether the destination may be overwritten.
    #[inline(always)]
    #[must_use]
    pub const fn overwrite(&self) -> bool {
        self.overwrite
    }

    /// Returns the required atomicity level.
    #[inline(always)]
    #[must_use]
    pub const fn atomicity(&self) -> AtomicityRequirement {
        self.atomicity
    }

    /// Returns the metadata preservation policy.
    #[inline(always)]
    #[must_use]
    pub const fn preserve_metadata(&self) -> MetadataPreservePolicy {
        self.preserve_metadata
    }

    /// Replaces whether the destination may be overwritten.
    #[inline]
    #[must_use]
    pub const fn with_overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = overwrite;
        self
    }

    /// Replaces the required atomicity level.
    #[inline]
    #[must_use]
    pub const fn with_atomicity(
        mut self,
        atomicity: AtomicityRequirement,
    ) -> Self {
        self.atomicity = atomicity;
        self
    }

    /// Replaces the metadata preservation policy.
    #[inline]
    #[must_use]
    pub const fn with_preserve_metadata(
        mut self,
        preserve_metadata: MetadataPreservePolicy,
    ) -> Self {
        self.preserve_metadata = preserve_metadata;
        self
    }

    /// Validates required persistence guarantees before provider side effects.
    ///
    /// # Errors
    /// Returns [`FsErrorKind::RequirementNotMet`] when atomic persistence is
    /// required but the configured filesystem does not guarantee it.
    pub fn validate_against(
        &self,
        capabilities: FileSystemCapabilities,
    ) -> Result<(), FsError> {
        if self.atomicity() == AtomicityRequirement::Required
            && !capabilities.contains(FileSystemCapability::AtomicTempPersist)
        {
            return Err(FsError::new(
                FsErrorKind::RequirementNotMet,
                FsOperation::PersistTemp,
                "atomic temporary persistence is required but not guaranteed",
            )
            .with_required_capability(
                FileSystemCapability::AtomicTempPersist,
            ));
        }
        Ok(())
    }
}

impl Default for PersistOptions {
    #[inline]
    fn default() -> Self {
        Self {
            overwrite: false,
            atomicity: AtomicityRequirement::Required,
            preserve_metadata: MetadataPreservePolicy::Portable,
        }
    }
}
