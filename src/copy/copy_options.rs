// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Copy operation options and policy types.

use std::time::Duration;

use crate::copy::CopyConflictPolicy;
use crate::copy::CopyMode;
use crate::copy::MetadataPreservePolicy;
use crate::copy::ServerSidePreference;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::metadata::AtomicityRequirement;
use crate::metadata::DurabilityRequirement;
use crate::metadata::FileSystemCapabilities;
use crate::metadata::FileSystemCapability;
use crate::metadata::SymlinkPolicy;

/// Options controlling file, object, or tree copy operations.
///
/// # Examples
/// ```rust
/// use qubit_fs::copy::CopyOptions;
/// use qubit_fs::copy::ServerSidePreference;
/// use qubit_fs::metadata::FileSystemCapabilities;
/// let options = CopyOptions::default().with_server_side(ServerSidePreference::Require);
/// assert!(options.validate_against(FileSystemCapabilities::new()).is_err());
/// ```
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
    /// Maximum descendant depth for tree copy.
    max_depth: Option<usize>,
    /// Maximum number of source entries copied by the operation.
    ///
    /// A file-mode copy always consumes exactly one entry, so `Some(0)`
    /// rejects it before provider I/O. Tree-mode copies count files,
    /// directories, symbolic links, objects, and prefixes in the aggregate.
    max_entries: Option<usize>,
    /// Maximum number of copied payload bytes.
    max_bytes: Option<u64>,
    /// Maximum cumulative elapsed duration for the copy operation.
    ///
    /// The budget starts when the copy operation is constructed and is
    /// checked cooperatively around provider calls. It does not forcibly
    /// interrupt a pending call or roll back a publication that already
    /// completed.
    deadline: Option<Duration>,
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

    /// Returns a copy of these options with the source mode replaced.
    ///
    /// # Parameters
    /// `mode` is the interpretation used for the source resource.
    ///
    /// # Returns
    /// A copy of these options with the requested source mode.
    #[inline]
    #[must_use]
    pub const fn with_mode(mut self, mode: CopyMode) -> Self {
        self.mode = mode;
        self
    }

    /// Returns the source interpretation mode.
    ///
    /// # Returns
    /// The configured source interpretation mode.
    #[inline]
    #[must_use]
    pub const fn mode(&self) -> CopyMode {
        self.mode
    }

    /// Returns a copy with the destination conflict policy replaced.
    ///
    /// # Parameters
    /// `conflict` is the policy applied when the destination already exists.
    ///
    /// # Returns
    /// A copy with the requested destination conflict policy.
    #[inline]
    #[must_use]
    pub const fn with_conflict(mut self, conflict: CopyConflictPolicy) -> Self {
        self.conflict = conflict;
        self
    }

    /// Returns the destination conflict policy.
    ///
    /// # Returns
    /// The configured destination conflict policy.
    #[inline]
    #[must_use]
    pub const fn conflict(&self) -> CopyConflictPolicy {
        self.conflict
    }

    /// Returns a copy with the metadata preservation policy replaced.
    ///
    /// # Parameters
    /// `preserve_metadata` selects which source metadata to preserve.
    ///
    /// # Returns
    /// A copy with the requested metadata preservation policy.
    #[inline]
    #[must_use]
    pub const fn with_preserve_metadata(mut self, preserve_metadata: MetadataPreservePolicy) -> Self {
        self.preserve_metadata = preserve_metadata;
        self
    }

    /// Returns the metadata preservation policy.
    ///
    /// # Returns
    /// The configured metadata preservation policy.
    #[inline]
    #[must_use]
    pub const fn preserve_metadata(&self) -> MetadataPreservePolicy {
        self.preserve_metadata
    }

    /// Returns a copy with the server-side preference replaced.
    ///
    /// # Parameters
    /// `server_side` selects whether server-side copy is disabled, preferred,
    /// or required.
    ///
    /// # Returns
    /// A copy with the requested server-side preference.
    #[inline]
    #[must_use]
    pub const fn with_server_side(mut self, server_side: ServerSidePreference) -> Self {
        self.server_side = server_side;
        self
    }

    /// Returns the server-side preference.
    ///
    /// # Returns
    /// The configured server-side copy preference.
    #[inline]
    #[must_use]
    pub const fn server_side(&self) -> ServerSidePreference {
        self.server_side
    }

    /// Returns a copy with the symbolic-link policy override replaced.
    ///
    /// # Parameters
    /// `policy` is the symbolic-link policy for this copy.
    ///
    /// # Returns
    /// A copy with the symbolic-link policy override set.
    #[inline]
    #[must_use]
    pub const fn with_symlink_policy(mut self, policy: SymlinkPolicy) -> Self {
        self.symlink_policy = Some(policy);
        self
    }

    /// Returns the optional symbolic-link policy override.
    ///
    /// # Returns
    /// `Some(policy)` when an override is set, or `None` when the filesystem
    /// default applies.
    #[inline]
    #[must_use]
    pub const fn symlink_policy_override(&self) -> Option<SymlinkPolicy> {
        self.symlink_policy
    }

    /// Returns a copy with parent creation replaced.
    ///
    /// # Parameters
    /// `create` determines whether missing destination parents are created.
    ///
    /// # Returns
    /// A copy with the requested parent-creation setting.
    #[inline]
    #[must_use]
    pub const fn with_create_parent(mut self, create: bool) -> Self {
        self.create_parent = create;
        self
    }

    /// Returns whether missing destination parents are created.
    ///
    /// # Returns
    /// `true` when missing destination parents are created, or `false` when
    /// they must already exist.
    #[inline]
    #[must_use]
    pub const fn create_parent(&self) -> bool {
        self.create_parent
    }

    /// Returns a copy with continuation policy replaced.
    ///
    /// # Parameters
    /// `continue_on_error` determines whether tree copy continues after an
    /// entry fails.
    ///
    /// # Returns
    /// A copy with the requested continuation policy.
    #[inline]
    #[must_use]
    pub const fn with_continue_on_error(mut self, continue_on_error: bool) -> Self {
        self.continue_on_error = continue_on_error;
        self
    }

    /// Returns whether tree copy continues after per-entry failures.
    ///
    /// # Returns
    /// `true` when tree copy continues after per-entry failures, or `false`
    /// when the first entry failure stops the operation.
    #[inline]
    #[must_use]
    pub const fn continue_on_error(&self) -> bool {
        self.continue_on_error
    }

    /// Returns a copy with the atomicity requirement replaced.
    ///
    /// # Parameters
    /// `atomicity` is the requested atomicity level for publication.
    ///
    /// # Returns
    /// A copy with the requested atomicity requirement.
    #[inline]
    #[must_use]
    pub const fn with_atomicity(mut self, atomicity: AtomicityRequirement) -> Self {
        self.atomicity = atomicity;
        self
    }

    /// Returns the atomicity requirement.
    ///
    /// # Returns
    /// The configured atomicity requirement.
    #[inline]
    #[must_use]
    pub const fn atomicity(&self) -> AtomicityRequirement {
        self.atomicity
    }

    /// Returns a copy with the durability requirement replaced.
    ///
    /// # Parameters
    /// `durability` is the requested durability level for publication.
    ///
    /// # Returns
    /// A copy with the requested durability requirement.
    #[inline]
    #[must_use]
    pub const fn with_durability(mut self, durability: DurabilityRequirement) -> Self {
        self.durability = durability;
        self
    }

    /// Returns the durability requirement.
    ///
    /// # Returns
    /// The configured durability requirement.
    #[inline]
    #[must_use]
    pub const fn durability(&self) -> DurabilityRequirement {
        self.durability
    }

    /// Returns a copy with the maximum tree depth replaced.
    ///
    /// # Parameters
    /// `max_depth` is the maximum descendant depth, or `None` for no limit.
    ///
    /// # Returns
    /// A copy with the requested tree-depth limit.
    #[inline]
    #[must_use]
    pub const fn with_max_depth(mut self, max_depth: Option<usize>) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Returns the optional maximum tree depth.
    ///
    /// # Returns
    /// `Some(depth)` for a configured limit, or `None` when depth is unbounded.
    #[inline]
    #[must_use]
    pub const fn max_depth(&self) -> Option<usize> {
        self.max_depth
    }

    /// Returns a copy with the maximum source entry count replaced.
    ///
    /// `Some(0)` rejects every copy because even a file-mode copy consumes one
    /// source entry. For tree copies, the limit applies to the aggregate of
    /// files, directories, symbolic links, objects, and prefixes.
    ///
    /// # Parameters
    /// `max_entries` is the maximum number of source entries, or `None` for no
    /// limit.
    ///
    /// # Returns
    /// A copy with the requested source-entry limit.
    #[inline]
    #[must_use]
    pub const fn with_max_entries(mut self, max_entries: Option<usize>) -> Self {
        self.max_entries = max_entries;
        self
    }

    /// Returns the optional maximum source entry count.
    ///
    /// `None` leaves the entry count unbounded. The count includes every
    /// source resource represented in the completed
    /// [`CopyOutcome`](crate::copy::CopyOutcome) statistics, including
    /// directories and prefixes.
    ///
    /// # Returns
    /// `Some(limit)` for a configured maximum, or `None` when the entry count
    /// is unbounded.
    #[inline]
    #[must_use]
    pub const fn max_entries(&self) -> Option<usize> {
        self.max_entries
    }

    /// Returns a copy with the maximum copied byte count replaced.
    ///
    /// # Parameters
    /// `max_bytes` is the maximum payload size, or `None` for no limit.
    ///
    /// # Returns
    /// A copy with the requested byte limit.
    #[inline]
    #[must_use]
    pub const fn with_max_bytes(mut self, max_bytes: Option<u64>) -> Self {
        self.max_bytes = max_bytes;
        self
    }

    /// Returns the optional maximum copied byte count.
    ///
    /// # Returns
    /// `Some(limit)` for a configured byte limit, or `None` when bytes are
    /// unbounded.
    #[inline]
    #[must_use]
    pub const fn max_bytes(&self) -> Option<u64> {
        self.max_bytes
    }

    /// Returns a copy with the maximum elapsed duration replaced.
    ///
    /// # Parameters
    /// `deadline` is the elapsed-time budget, or `None` for no limit.
    ///
    /// # Returns
    /// A copy with the requested elapsed-time budget.
    #[inline]
    #[must_use]
    pub const fn with_deadline(mut self, deadline: Option<Duration>) -> Self {
        self.deadline = deadline;
        self
    }

    /// Returns the optional maximum elapsed duration.
    ///
    /// # Returns
    /// `Some(duration)` for a configured budget, or `None` when elapsed time
    /// is unbounded.
    #[inline]
    #[must_use]
    pub const fn deadline(&self) -> Option<Duration> {
        self.deadline
    }

    /// Validates required copy semantics before provider side effects.
    ///
    /// Preferred server-side copy may fall back and report its actual method.
    /// Required server-side copy must fail this preflight when the configured
    /// filesystem does not guarantee it.
    ///
    /// # Parameters
    /// `capabilities` describes the guarantees available from the filesystem.
    ///
    /// # Returns
    /// `Ok(())` when all required copy semantics are supported.
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
            max_depth: None,
            max_entries: None,
            max_bytes: None,
            deadline: None,
        }
    }
}
