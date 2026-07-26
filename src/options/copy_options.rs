// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Copy operation options and policy types.

use crate::{
    CopyConflictPolicy, CopyMode, FileSystemCapabilities, FileSystemCapability, FsError,
    FsErrorKind, FsOperation, MetadataPreservePolicy, ServerSidePreference,
};

/// Options controlling file, object, or tree copy operations.
#[derive(Clone, Debug, PartialEq)]
pub struct CopyOptions {
    /// Copy source interpretation mode.
    pub mode: CopyMode,
    /// Destination conflict policy.
    pub conflict: CopyConflictPolicy,
    /// Metadata preservation policy.
    pub preserve_metadata: MetadataPreservePolicy,
    /// Server-side copy preference.
    pub server_side: ServerSidePreference,
    /// Whether symbolic links should be followed.
    pub follow_symlinks: bool,
    /// Whether missing destination parents should be created.
    pub create_parent: bool,
    /// Whether tree copy should continue after per-entry failures.
    pub continue_on_error: bool,
}

impl CopyOptions {
    /// Creates options for copying one file-like resource.
    ///
    /// # Returns
    /// Copy options with `mode` set to [`CopyMode::File`].
    #[inline]
    #[must_use]
    pub fn file() -> Self {
        Self {
            mode: CopyMode::File,
            ..Self::default()
        }
    }

    /// Creates options for copying a resource tree.
    ///
    /// # Returns
    /// Copy options with `mode` set to [`CopyMode::Tree`].
    #[inline]
    #[must_use]
    pub fn tree() -> Self {
        Self {
            mode: CopyMode::Tree,
            ..Self::default()
        }
    }

    /// Validates required copy semantics before provider side effects.
    ///
    /// Preferred server-side copy may fall back and report its actual method.
    /// Required server-side copy must fail this preflight when the configured
    /// filesystem does not guarantee it.
    ///
    /// # Errors
    ///
    /// Returns [`FsErrorKind::RequirementNotMet`] with
    /// [`FileSystemCapability::ServerSideCopy`] when required server-side copy
    /// is unavailable.
    pub fn validate_against(&self, capabilities: FileSystemCapabilities) -> Result<(), FsError> {
        if self.server_side == ServerSidePreference::Require
            && !capabilities.contains(FileSystemCapability::ServerSideCopy)
        {
            return Err(FsError::new(
                FsErrorKind::RequirementNotMet,
                FsOperation::Copy,
                "server-side copy is required but not guaranteed",
            )
            .with_required_capability(FileSystemCapability::ServerSideCopy));
        }
        Ok(())
    }
}

impl Default for CopyOptions {
    #[inline]
    fn default() -> Self {
        Self {
            mode: CopyMode::Auto,
            conflict: CopyConflictPolicy::Fail,
            preserve_metadata: MetadataPreservePolicy::Portable,
            server_side: ServerSidePreference::Prefer,
            follow_symlinks: false,
            create_parent: false,
            continue_on_error: false,
        }
    }
}
