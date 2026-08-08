// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Copy operation options and policy types.

use crate::AtomicityRequirement;
use crate::CopyConflictPolicy;
use crate::CopyMode;
use crate::DurabilityRequirement;
use crate::FileSystemCapabilities;
use crate::FileSystemCapability;
use crate::FsError;
use crate::FsErrorKind;
use crate::FsOperation;
use crate::MetadataPreservePolicy;
use crate::ServerSidePreference;
use crate::SymlinkPolicy;

/// Options controlling file, object, or tree copy operations.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct CopyOptions {
    /// Copy source interpretation mode.
    mode: CopyMode,
    /// Destination conflict policy.
    conflict: CopyConflictPolicy,
    /// Metadata preservation policy.
    preserve_metadata: MetadataPreservePolicy,
    /// Server-side copy preference.
    server_side: ServerSidePreference,
    /// Optional symbolic-link policy overriding the filesystem default.
    symlink_policy: Option<SymlinkPolicy>,
    /// Whether missing destination parents should be created.
    create_parent: bool,
    /// Whether tree copy should continue after per-entry failures.
    continue_on_error: bool,
    /// Required atomicity of destination publication.
    atomicity: AtomicityRequirement,
    /// Required durability of destination publication.
    durability: DurabilityRequirement,
}

impl CopyOptions {
    /// Returns a copy of these options with the source mode replaced.
    #[inline]
    #[must_use]
    pub const fn with_mode(mut self, mode: CopyMode) -> Self {
        self.mode = mode;
        self
    }

    /// Returns the source interpretation mode.
    #[inline(always)]
    #[must_use]
    pub const fn mode(&self) -> CopyMode {
        self.mode
    }

    /// Returns a copy with the destination conflict policy replaced.
    #[inline]
    #[must_use]
    pub const fn with_conflict(mut self, conflict: CopyConflictPolicy) -> Self {
        self.conflict = conflict;
        self
    }

    /// Returns the destination conflict policy.
    #[inline(always)]
    #[must_use]
    pub const fn conflict(&self) -> CopyConflictPolicy {
        self.conflict
    }

    /// Returns a copy with the metadata preservation policy replaced.
    #[inline]
    #[must_use]
    pub const fn with_preserve_metadata(
        mut self,
        preserve_metadata: MetadataPreservePolicy,
    ) -> Self {
        self.preserve_metadata = preserve_metadata;
        self
    }

    /// Returns the metadata preservation policy.
    #[inline(always)]
    #[must_use]
    pub const fn preserve_metadata(&self) -> MetadataPreservePolicy {
        self.preserve_metadata
    }

    /// Returns a copy with the server-side preference replaced.
    #[inline]
    #[must_use]
    pub const fn with_server_side(mut self, server_side: ServerSidePreference) -> Self {
        self.server_side = server_side;
        self
    }

    /// Returns the server-side preference.
    #[inline(always)]
    #[must_use]
    pub const fn server_side(&self) -> ServerSidePreference {
        self.server_side
    }

    /// Returns a copy with the symbolic-link policy override replaced.
    #[inline]
    #[must_use]
    pub const fn with_symlink_policy(mut self, policy: SymlinkPolicy) -> Self {
        self.symlink_policy = Some(policy);
        self
    }

    /// Returns the optional symbolic-link policy override.
    #[inline(always)]
    #[must_use]
    pub const fn symlink_policy_override(&self) -> Option<SymlinkPolicy> {
        self.symlink_policy
    }

    /// Returns a copy with parent creation replaced.
    #[inline]
    #[must_use]
    pub const fn with_create_parent(mut self, create: bool) -> Self {
        self.create_parent = create;
        self
    }

    /// Returns whether missing destination parents are created.
    #[inline(always)]
    #[must_use]
    pub const fn create_parent(&self) -> bool {
        self.create_parent
    }

    /// Returns a copy with continuation policy replaced.
    #[inline]
    #[must_use]
    pub const fn with_continue_on_error(mut self, continue_on_error: bool) -> Self {
        self.continue_on_error = continue_on_error;
        self
    }

    /// Returns whether tree copy continues after per-entry failures.
    #[inline(always)]
    #[must_use]
    pub const fn continue_on_error(&self) -> bool {
        self.continue_on_error
    }

    /// Returns a copy with the atomicity requirement replaced.
    #[inline]
    #[must_use]
    pub const fn with_atomicity(mut self, atomicity: AtomicityRequirement) -> Self {
        self.atomicity = atomicity;
        self
    }

    /// Returns the atomicity requirement.
    #[inline(always)]
    #[must_use]
    pub const fn atomicity(&self) -> AtomicityRequirement {
        self.atomicity
    }

    /// Returns a copy with the durability requirement replaced.
    #[inline]
    #[must_use]
    pub const fn with_durability(mut self, durability: DurabilityRequirement) -> Self {
        self.durability = durability;
        self
    }

    /// Returns the durability requirement.
    #[inline(always)]
    #[must_use]
    pub const fn durability(&self) -> DurabilityRequirement {
        self.durability
    }

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
            && !capabilities.supports(FileSystemCapability::ServerSideCopy)
        {
            return Err(FsError::new(
                FsErrorKind::RequirementNotMet,
                FsOperation::Copy,
                "server-side copy is required but not supported",
            )
            .with_required_capability(FileSystemCapability::ServerSideCopy));
        }
        let atomic_capability = match self.mode {
            CopyMode::Auto => FileSystemCapability::AtomicFileCopy,
            CopyMode::File => FileSystemCapability::AtomicFileCopy,
            CopyMode::Tree => FileSystemCapability::AtomicTreeCopy,
        };
        let atomic_supported = match self.mode {
            CopyMode::Auto => {
                capabilities.supports(FileSystemCapability::AtomicFileCopy)
                    || capabilities.supports(FileSystemCapability::AtomicTreeCopy)
            }
            CopyMode::File | CopyMode::Tree => capabilities.supports(atomic_capability),
        };
        if self.atomicity == AtomicityRequirement::Required && !atomic_supported {
            return Err(FsError::new(
                FsErrorKind::RequirementNotMet,
                FsOperation::Copy,
                "atomic copy publication is required but not supported",
            )
            .with_required_capability(atomic_capability));
        }
        let durable_capability = match self.mode {
            CopyMode::Auto => FileSystemCapability::DurableFileCopy,
            CopyMode::File => FileSystemCapability::DurableFileCopy,
            CopyMode::Tree => FileSystemCapability::DurableTreeCopy,
        };
        let durable_supported = match self.mode {
            CopyMode::Auto => {
                capabilities.supports(FileSystemCapability::DurableFileCopy)
                    || capabilities.supports(FileSystemCapability::DurableTreeCopy)
            }
            CopyMode::File | CopyMode::Tree => capabilities.supports(durable_capability),
        };
        if self.durability == DurabilityRequirement::Required && !durable_supported {
            return Err(FsError::new(
                FsErrorKind::RequirementNotMet,
                FsOperation::Copy,
                "durable copy publication is required but not supported",
            )
            .with_required_capability(durable_capability));
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
            preserve_metadata: MetadataPreservePolicy::None,
            server_side: ServerSidePreference::Disable,
            symlink_policy: None,
            create_parent: false,
            continue_on_error: false,
            atomicity: AtomicityRequirement::NotRequired,
            durability: DurabilityRequirement::NotRequired,
        }
    }
}
