// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Delete operation options.

use crate::{
    FileSystemCapabilities,
    FileSystemCapability,
    FsError,
    FsErrorKind,
    FsOperation,
    ResourceVersion,
};

/// Options controlling delete operations.
#[non_exhaustive]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeleteOptions {
    /// Whether container resources should be removed recursively.
    recursive: bool,
    /// Whether a missing target should be treated as success.
    missing_ok: bool,
    /// Optional required ETag or provider version.
    if_match: Option<ResourceVersion>,
}

impl DeleteOptions {
    /// Returns whether container resources should be removed recursively.
    #[inline(always)]
    #[must_use]
    pub const fn recursive(&self) -> bool {
        self.recursive
    }

    /// Returns whether a missing target should be treated as success.
    #[inline(always)]
    #[must_use]
    pub const fn missing_ok(&self) -> bool {
        self.missing_ok
    }

    /// Returns the optional required ETag or provider version.
    #[inline(always)]
    #[must_use]
    pub const fn if_match(&self) -> Option<&ResourceVersion> {
        self.if_match.as_ref()
    }

    /// Replaces recursive container deletion.
    #[inline]
    #[must_use]
    pub const fn with_recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }

    /// Replaces missing-target acceptance.
    #[inline]
    #[must_use]
    pub const fn with_missing_ok(mut self, missing_ok: bool) -> Self {
        self.missing_ok = missing_ok;
        self
    }

    /// Replaces the optional required ETag or provider version.
    #[inline]
    #[must_use]
    pub fn with_if_match(mut self, if_match: Option<ResourceVersion>) -> Self {
        self.if_match = if_match;
        self
    }

    /// Validates required deletion semantics before provider side effects.
    ///
    /// # Errors
    ///
    /// Returns [`FsErrorKind::RequirementNotMet`] with the exact missing
    /// recursive or conditional-delete capability.
    pub fn validate_against(
        &self,
        capabilities: FileSystemCapabilities,
    ) -> Result<(), FsError> {
        if self.recursive()
            && !capabilities.contains(FileSystemCapability::RecursiveDelete)
        {
            return Err(missing_requirement(
                FileSystemCapability::RecursiveDelete,
                "recursive deletion is required but not guaranteed",
            ));
        }
        if self.if_match().is_some()
            && !capabilities.contains(FileSystemCapability::ConditionalDelete)
        {
            return Err(missing_requirement(
                FileSystemCapability::ConditionalDelete,
                "conditional deletion is required but not guaranteed",
            ));
        }
        Ok(())
    }
}

/// Builds a typed unmet delete requirement.
fn missing_requirement(
    capability: FileSystemCapability,
    message: &str,
) -> FsError {
    FsError::new(FsErrorKind::RequirementNotMet, FsOperation::Delete, message)
        .with_required_capability(capability)
}
