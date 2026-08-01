// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Rename operation options.

use crate::{
    AtomicityRequirement,
    FileSystemCapabilities,
    FileSystemCapability,
    FsError,
    FsErrorKind,
    FsOperation,
};

/// Options controlling rename operations.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenameOptions {
    /// Whether the destination may be overwritten.
    overwrite: bool,
    /// Required atomicity level.
    atomicity: AtomicityRequirement,
}

impl Default for RenameOptions {
    #[inline]
    fn default() -> Self {
        Self {
            overwrite: false,
            atomicity: AtomicityRequirement::Preferred,
        }
    }
}

impl RenameOptions {
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

    /// Validates required atomicity against a configured capability snapshot.
    ///
    /// Providers should call this before making any source or destination
    /// change. Preferred and not-required requests always pass preflight and
    /// must report the actual successful method in [`crate::RenameOutcome`].
    ///
    /// # Parameters
    /// - `capabilities`: Stable capabilities of the configured filesystem.
    ///
    /// # Errors
    /// Returns [`FsErrorKind::RequirementNotMet`] when atomic rename is
    /// required but not guaranteed.
    pub fn validate_against(
        &self,
        capabilities: FileSystemCapabilities,
    ) -> Result<(), FsError> {
        if self.atomicity() == AtomicityRequirement::Required
            && !capabilities.contains(FileSystemCapability::AtomicRename)
        {
            return Err(FsError::new(
                FsErrorKind::RequirementNotMet,
                FsOperation::Rename,
                "atomic rename is required but not guaranteed",
            )
            .with_required_capability(FileSystemCapability::AtomicRename));
        }
        Ok(())
    }
}
