// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow inline-tests -- the private capability-operation mapping
// requires direct coverage.
//! Validated provider operation and guarantee snapshots.

use super::ProviderOperation;
use super::ProviderOperations;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::FileSystemCapabilities;
use crate::metadata::FileSystemCapability;
use crate::metadata::FileSystemInfo;
use crate::metadata::FileSystemLimits;
use crate::metadata::FileSystemProperties;
use crate::metadata::SymlinkPolicy;
use crate::path::PathConstraints;

/// Immutable provider-declared operations, guarantees, and constraints.
#[derive(Clone, Debug)]
pub struct ProviderProperties {
    /// Stable filesystem information.
    info: FileSystemInfo,
    /// Concrete provider entry points available for facade dispatch.
    operations: ProviderOperations,
    /// Capabilities explicitly declared by the provider.
    declared_capabilities: FileSystemCapabilities,
    /// Stable provider limits.
    limits: FileSystemLimits,
    /// Accepted logical path forms.
    path_constraints: PathConstraints,
    /// Provider-declared symbolic-link traversal policy.
    symlink_policy: SymlinkPolicy,
}

impl ProviderProperties {
    /// Builds and validates an immutable provider snapshot.
    ///
    /// This method performs no I/O.
    ///
    /// # Parameters
    /// - `info`: Stable provider identity and path semantics.
    /// - `operations`: Concrete provider entry points available for dispatch.
    /// - `declared_capabilities`: Guarantees explicitly declared by the
    ///   provider.
    /// - `limits`: Provider resource and operation limits.
    /// - `path_constraints`: Accepted absolute and relative path forms.
    /// - `symlink_policy`: Provider-declared symbolic-link traversal policy.
    ///
    /// # Returns
    /// A validated immutable provider snapshot.
    ///
    /// # Errors
    /// Returns an invalid-options error when shared property invariants fail or
    /// a declared capability lacks its required provider operation entry point.
    pub fn new(
        info: FileSystemInfo,
        operations: ProviderOperations,
        declared_capabilities: FileSystemCapabilities,
        limits: FileSystemLimits,
        path_constraints: PathConstraints,
        symlink_policy: SymlinkPolicy,
    ) -> FsResult<Self> {
        let value = Self {
            info,
            operations,
            declared_capabilities,
            limits,
            path_constraints,
            symlink_policy,
        };
        value.validate()?;
        Ok(value)
    }

    /// Returns the stable filesystem identity and configuration.
    ///
    /// # Returns
    /// The immutable provider information snapshot.
    #[inline(always)]
    #[must_use]
    pub const fn info(&self) -> &FileSystemInfo {
        &self.info
    }

    /// Returns the concrete provider operation snapshot.
    ///
    /// # Returns
    /// Provider entry points available for facade dispatch.
    #[inline(always)]
    #[must_use]
    pub const fn operations(&self) -> ProviderOperations {
        self.operations
    }

    /// Returns capabilities explicitly declared by the provider.
    ///
    /// # Returns
    /// The provider guarantee snapshot before facade derivation.
    #[inline(always)]
    #[must_use]
    pub const fn declared_capabilities(&self) -> FileSystemCapabilities {
        self.declared_capabilities
    }

    /// Returns the stable filesystem limits.
    ///
    /// # Returns
    /// The immutable provider limit snapshot.
    #[inline(always)]
    #[must_use]
    pub const fn limits(&self) -> &FileSystemLimits {
        &self.limits
    }

    /// Returns the immutable accepted path constraints.
    ///
    /// # Returns
    /// The accepted logical path forms.
    #[inline(always)]
    #[must_use]
    pub const fn path_constraints(&self) -> &PathConstraints {
        &self.path_constraints
    }

    /// Returns the provider-declared symbolic-link traversal policy.
    ///
    /// # Returns
    /// The immutable symbolic-link policy.
    #[inline(always)]
    #[must_use = "the provider symbolic-link policy must be used"]
    pub const fn symlink_policy(&self) -> SymlinkPolicy {
        self.symlink_policy
    }

    /// Validates shared value invariants and capability-operation consistency.
    ///
    /// # Errors
    /// Returns an invalid-options error for invalid shared fields or for the
    /// first declared capability whose provider entry points are absent.
    fn validate(&self) -> FsResult<()> {
        FileSystemProperties::new(
            self.info.clone(),
            self.declared_capabilities,
            self.limits,
            self.path_constraints.clone(),
            self.symlink_policy,
        )?;
        let error = FileSystemCapability::ALL.iter().copied().find_map(|capability| {
            if self.declared_capabilities.supports(capability) {
                self.missing_capability_operation_error(capability)
            } else {
                None
            }
        });
        if let Some(error) = error {
            return Err(error);
        }
        Ok(())
    }

    /// Builds exact validation context when `capability` lacks a required
    /// provider entry point.
    ///
    /// # Returns
    /// `Some` error containing `capability` and the first missing operation, or
    /// `None` when the capability's operation contract is satisfied.
    fn missing_capability_operation_error(&self, capability: FileSystemCapability) -> Option<FsError> {
        let operation = self.missing_capability_operation(capability)?;
        let message =
            format!("declared capability {capability:?} requires unavailable provider operation {operation:?}",);
        Some(
            FsError::new(FsErrorKind::InvalidOptions, FsOperation::ValidateProperties, &message)
                .with_required_capability(capability),
        )
    }

    /// Returns the first provider entry point required by `capability` that is
    /// absent, or `None` when its operation contract is satisfied.
    fn missing_capability_operation(&self, capability: FileSystemCapability) -> Option<ProviderOperation> {
        let operations = self.operations;
        match capability {
            FileSystemCapability::List => {
                (!operations.supports(ProviderOperation::List)).then_some(ProviderOperation::List)
            }
            FileSystemCapability::Read
            | FileSystemCapability::RangeRead
            | FileSystemCapability::ConditionalRead
            | FileSystemCapability::ChecksumValidation => {
                (!operations.supports(ProviderOperation::OpenReader)).then_some(ProviderOperation::OpenReader)
            }
            FileSystemCapability::Write
            | FileSystemCapability::Append
            | FileSystemCapability::ConditionalWrite
            | FileSystemCapability::AtomicReplace
            | FileSystemCapability::DurableWrite => {
                (!operations.supports(ProviderOperation::OpenWriter)).then_some(ProviderOperation::OpenWriter)
            }
            FileSystemCapability::CreateDirectory | FileSystemCapability::EmptyDirectory => {
                (!operations.supports(ProviderOperation::CreateDirectory)).then_some(ProviderOperation::CreateDirectory)
            }
            FileSystemCapability::Delete | FileSystemCapability::ConditionalDelete => {
                if !operations.supports(ProviderOperation::DeleteFile) {
                    Some(ProviderOperation::DeleteFile)
                } else if !operations.supports(ProviderOperation::DeleteDirectory) {
                    Some(ProviderOperation::DeleteDirectory)
                } else {
                    None
                }
            }
            FileSystemCapability::RecursiveDelete => {
                (!operations.supports(ProviderOperation::DeleteDirectory)).then_some(ProviderOperation::DeleteDirectory)
            }
            FileSystemCapability::Rename | FileSystemCapability::AtomicRename | FileSystemCapability::DurableRename => {
                (!operations.supports(ProviderOperation::Rename)).then_some(ProviderOperation::Rename)
            }
            FileSystemCapability::TempFile => {
                (!operations.supports(ProviderOperation::CreateTempFile)).then_some(ProviderOperation::CreateTempFile)
            }
            FileSystemCapability::TempDirectory => (!operations.supports(ProviderOperation::CreateTempDirectory))
                .then_some(ProviderOperation::CreateTempDirectory),
            FileSystemCapability::AtomicTempPersist => {
                if !operations.supports(ProviderOperation::CreateTempFile) {
                    Some(ProviderOperation::CreateTempFile)
                } else if !operations.supports(ProviderOperation::CreateTempDirectory) {
                    Some(ProviderOperation::CreateTempDirectory)
                } else {
                    None
                }
            }
            FileSystemCapability::Copy
            | FileSystemCapability::ServerSideCopy
            | FileSystemCapability::AtomicFileCopy
            | FileSystemCapability::AtomicTreeCopy
            | FileSystemCapability::DurableFileCopy
            | FileSystemCapability::DurableTreeCopy => {
                (!operations.supports(ProviderOperation::TryCopy)).then_some(ProviderOperation::TryCopy)
            }
            FileSystemCapability::Symlink => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderProperties;
    use crate::error::FsOperation;
    use crate::metadata::FileSystemCapabilities;
    use crate::metadata::FileSystemCapability;
    use crate::metadata::FileSystemId;
    use crate::metadata::FileSystemInfo;
    use crate::metadata::FileSystemLimits;
    use crate::metadata::SymlinkPolicy;
    use crate::path::PathConstraints;
    use crate::path::PathSemantics;
    use crate::spi::ProviderOperation;
    use crate::spi::ProviderOperations;

    /// Builds an unvalidated snapshot for direct mapping-helper coverage.
    fn properties_with_operations(operations: ProviderOperations) -> ProviderProperties {
        ProviderProperties {
            info: FileSystemInfo::new(
                FileSystemId::new("provider-mapping-test").expect("test filesystem id should be valid"),
                "provider-mapping-test",
                PathSemantics::Hierarchical,
            ),
            operations,
            declared_capabilities: FileSystemCapabilities::new(),
            limits: FileSystemLimits::unknown(),
            path_constraints: PathConstraints::absolute(),
            symlink_policy: SymlinkPolicy::Reject,
        }
    }

    /// Returns every provider entry point except `missing`, or every entry
    /// point when `missing` is `None`.
    fn operations_without(missing: Option<ProviderOperation>) -> ProviderOperations {
        let mut operations = ProviderOperations::new();
        for operation in [
            ProviderOperation::Stat,
            ProviderOperation::List,
            ProviderOperation::OpenReader,
            ProviderOperation::OpenWriter,
            ProviderOperation::CreateDirectory,
            ProviderOperation::DeleteFile,
            ProviderOperation::DeleteDirectory,
            ProviderOperation::TryCopy,
            ProviderOperation::Rename,
            ProviderOperation::CreateTempFile,
            ProviderOperation::CreateTempDirectory,
        ] {
            if missing != Some(operation) {
                operations = operations.with(operation);
            }
        }
        operations
    }

    /// Verifies every capability maps directly to its own required operation
    /// and exact error context, independent of capability dependencies.
    #[test]
    fn test_missing_capability_operation_maps_each_capability_directly() {
        let cases = [
            (FileSystemCapability::List, Some(ProviderOperation::List)),
            (FileSystemCapability::Read, Some(ProviderOperation::OpenReader)),
            (FileSystemCapability::RangeRead, Some(ProviderOperation::OpenReader)),
            (
                FileSystemCapability::ConditionalRead,
                Some(ProviderOperation::OpenReader),
            ),
            (
                FileSystemCapability::ChecksumValidation,
                Some(ProviderOperation::OpenReader),
            ),
            (FileSystemCapability::Write, Some(ProviderOperation::OpenWriter)),
            (FileSystemCapability::Append, Some(ProviderOperation::OpenWriter)),
            (
                FileSystemCapability::ConditionalWrite,
                Some(ProviderOperation::OpenWriter),
            ),
            (FileSystemCapability::AtomicReplace, Some(ProviderOperation::OpenWriter)),
            (FileSystemCapability::DurableWrite, Some(ProviderOperation::OpenWriter)),
            (
                FileSystemCapability::CreateDirectory,
                Some(ProviderOperation::CreateDirectory),
            ),
            (
                FileSystemCapability::EmptyDirectory,
                Some(ProviderOperation::CreateDirectory),
            ),
            (FileSystemCapability::Delete, Some(ProviderOperation::DeleteFile)),
            (FileSystemCapability::Delete, Some(ProviderOperation::DeleteDirectory)),
            (
                FileSystemCapability::RecursiveDelete,
                Some(ProviderOperation::DeleteDirectory),
            ),
            (
                FileSystemCapability::ConditionalDelete,
                Some(ProviderOperation::DeleteFile),
            ),
            (
                FileSystemCapability::ConditionalDelete,
                Some(ProviderOperation::DeleteDirectory),
            ),
            (FileSystemCapability::Rename, Some(ProviderOperation::Rename)),
            (FileSystemCapability::AtomicRename, Some(ProviderOperation::Rename)),
            (FileSystemCapability::DurableRename, Some(ProviderOperation::Rename)),
            (FileSystemCapability::TempFile, Some(ProviderOperation::CreateTempFile)),
            (
                FileSystemCapability::TempDirectory,
                Some(ProviderOperation::CreateTempDirectory),
            ),
            (
                FileSystemCapability::AtomicTempPersist,
                Some(ProviderOperation::CreateTempFile),
            ),
            (
                FileSystemCapability::AtomicTempPersist,
                Some(ProviderOperation::CreateTempDirectory),
            ),
            (FileSystemCapability::Copy, Some(ProviderOperation::TryCopy)),
            (FileSystemCapability::ServerSideCopy, Some(ProviderOperation::TryCopy)),
            (FileSystemCapability::AtomicFileCopy, Some(ProviderOperation::TryCopy)),
            (FileSystemCapability::AtomicTreeCopy, Some(ProviderOperation::TryCopy)),
            (FileSystemCapability::DurableFileCopy, Some(ProviderOperation::TryCopy)),
            (FileSystemCapability::DurableTreeCopy, Some(ProviderOperation::TryCopy)),
            (FileSystemCapability::Symlink, None),
        ];

        for (capability, required_operation) in cases {
            let properties = properties_with_operations(operations_without(required_operation));
            assert_eq!(required_operation, properties.missing_capability_operation(capability),);
            match required_operation {
                Some(required_operation) => {
                    let error = properties
                        .missing_capability_operation_error(capability)
                        .expect("missing operation must produce an error");
                    assert_eq!(FsOperation::ValidateProperties, error.operation());
                    assert_eq!(Some(capability), error.required_capability());
                    assert!(format!("{error}").contains(&format!("{required_operation:?}")));
                }
                None => assert!(properties.missing_capability_operation_error(capability).is_none()),
            }
        }
    }
}
