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
};

/// Options controlling delete operations.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeleteOptions {
    /// Whether container resources should be removed recursively.
    pub recursive: bool,
    /// Whether a missing target should be treated as success.
    pub missing_ok: bool,
    /// Optional required ETag or provider version.
    pub if_match: Option<String>,
}

impl DeleteOptions {
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
        if self.recursive
            && !capabilities.contains(FileSystemCapability::RecursiveDelete)
        {
            return Err(missing_requirement(
                FileSystemCapability::RecursiveDelete,
                "recursive deletion is required but not guaranteed",
            ));
        }
        if self.if_match.is_some()
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
