// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Temporary resource persistence options.

use crate::{
    AtomicityRequirement, FileSystemCapabilities, FileSystemCapability, FsError, FsErrorKind,
    FsOperation, MetadataPreservePolicy,
};

/// Options controlling temporary resource persistence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistOptions {
    /// Whether the destination may be overwritten.
    pub overwrite: bool,
    /// Required atomicity level.
    pub atomicity: AtomicityRequirement,
    /// Metadata preservation policy.
    pub preserve_metadata: MetadataPreservePolicy,
}

impl PersistOptions {
    /// Validates required persistence guarantees before provider side effects.
    ///
    /// # Errors
    /// Returns [`FsErrorKind::RequirementNotMet`] when atomic persistence is
    /// required but the configured filesystem does not guarantee it.
    pub fn validate_against(&self, capabilities: FileSystemCapabilities) -> Result<(), FsError> {
        if self.atomicity == AtomicityRequirement::Required
            && !capabilities.contains(FileSystemCapability::AtomicTempPersist)
        {
            return Err(FsError::new(
                FsErrorKind::RequirementNotMet,
                FsOperation::PersistTemp,
                "atomic temporary persistence is required but not guaranteed",
            )
            .with_required_capability(FileSystemCapability::AtomicTempPersist));
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
