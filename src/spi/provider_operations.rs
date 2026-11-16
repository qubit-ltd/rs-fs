// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Compact immutable sets of provider operation entry points.

use super::ProviderOperation;
use crate::metadata::FileSystemCapabilities;
use crate::metadata::FileSystemCapability;

/// Immutable set of concrete operation entry points implemented by a provider.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::spi::{ProviderOperation, ProviderOperations};
///
/// let ops = ProviderOperations::new().with(ProviderOperation::Stat);
/// assert!(ops.supports(ProviderOperation::Stat));
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProviderOperations {
    /// Bit flags indexed by [`ProviderOperation`] discriminants.
    bits: u128,
}

impl ProviderOperations {
    /// Creates an empty provider-operation set.
    ///
    /// # Returns
    /// A set containing no provider operation entry points.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    /// Returns a copy containing `operation`.
    ///
    /// # Parameters
    /// - `operation`: Provider entry point to insert.
    ///
    /// # Returns
    /// The updated immutable operation set.
    #[inline]
    #[must_use]
    pub const fn with(mut self, operation: ProviderOperation) -> Self {
        self.bits |= 1_u128 << operation as u8;
        self
    }

    /// Returns whether the provider implements `operation`.
    ///
    /// # Parameters
    /// - `operation`: Provider entry point to query.
    ///
    /// # Returns
    /// `true` when the operation is present in this snapshot.
    #[inline]
    #[must_use]
    pub const fn supports(&self, operation: ProviderOperation) -> bool {
        self.bits & (1_u128 << operation as u8) != 0
    }

    /// Derives conditional capability support from the concrete entry points
    /// present in this operation set.
    ///
    /// Entry-point presence only establishes that an operation can be
    /// attempted. It does not establish an unconditional guarantee. Stronger
    /// guarantees must be supplied separately by the provider.
    #[must_use]
    pub fn conditional_capabilities(&self) -> FileSystemCapabilities {
        let mut capabilities = FileSystemCapabilities::new();
        for capability in FileSystemCapability::ALL {
            if self.supports_capability(*capability) {
                capabilities = capabilities.with_conditional(*capability);
            }
        }
        capabilities
    }

    /// Returns whether an operation entry point exists for `capability`.
    ///
    /// This is an exact entry-point mapping, not a provider-name heuristic or
    /// a claim that all requests satisfy the capability.
    #[must_use]
    pub const fn supports_capability(&self, capability: FileSystemCapability) -> bool {
        let supported = match capability {
            FileSystemCapability::List => self.supports(ProviderOperation::List),
            FileSystemCapability::Read
            | FileSystemCapability::RangeRead
            | FileSystemCapability::ConditionalRead
            | FileSystemCapability::ChecksumValidation => self.supports(ProviderOperation::OpenReader),
            FileSystemCapability::Write
            | FileSystemCapability::Append
            | FileSystemCapability::ConditionalWrite
            | FileSystemCapability::AtomicReplace
            | FileSystemCapability::DurableWrite => self.supports(ProviderOperation::OpenWriter),
            FileSystemCapability::CreateDirectory | FileSystemCapability::EmptyDirectory => {
                self.supports(ProviderOperation::CreateDirectory)
            }
            FileSystemCapability::Delete | FileSystemCapability::ConditionalDelete => {
                self.supports(ProviderOperation::DeleteFile) && self.supports(ProviderOperation::DeleteDirectory)
            }
            FileSystemCapability::RecursiveDelete => self.supports(ProviderOperation::DeleteDirectory),
            FileSystemCapability::Rename | FileSystemCapability::AtomicRename | FileSystemCapability::DurableRename => {
                self.supports(ProviderOperation::Rename)
            }
            FileSystemCapability::TempFile => self.supports(ProviderOperation::CreateTempFile),
            FileSystemCapability::TempDirectory => self.supports(ProviderOperation::CreateTempDirectory),
            FileSystemCapability::AtomicTempPersist => {
                self.supports(ProviderOperation::CreateTempFile)
                    && self.supports(ProviderOperation::CreateTempDirectory)
            }
            FileSystemCapability::Copy
            | FileSystemCapability::ServerSideCopy
            | FileSystemCapability::AtomicFileCopy
            | FileSystemCapability::AtomicTreeCopy
            | FileSystemCapability::DurableFileCopy
            | FileSystemCapability::DurableTreeCopy => self.supports(ProviderOperation::TryCopy),
            FileSystemCapability::ReadAll => self.supports(ProviderOperation::ReadAll),
            FileSystemCapability::ObserveReadChanges | FileSystemCapability::VersionConsistentRead => {
                self.supports(ProviderOperation::OpenRandomAccessReader)
            }
            FileSystemCapability::StagedTreePublish => self.supports(ProviderOperation::PublishTree),
            FileSystemCapability::StorageObservation => self.supports(ProviderOperation::ObserveStorage),
            FileSystemCapability::Symlink => false,
        };
        supported
    }
}
