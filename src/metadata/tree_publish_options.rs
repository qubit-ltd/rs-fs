// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use super::AtomicityRequirement;
use super::DurabilityRequirement;
use super::FileSystemCapabilities;
use super::FileSystemCapability;
use super::FileSystemCapabilitySupport;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;

/// Publication guarantees requested for a staged directory tree.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::metadata::AtomicityRequirement;
/// use qubit_fs::metadata::FileSystemCapabilities;
/// use qubit_fs::metadata::FileSystemCapability;
/// use qubit_fs::metadata::TreePublishOptions;
///
/// let options = TreePublishOptions {
///     atomicity: AtomicityRequirement::Required,
///     ..TreePublishOptions::default()
/// };
/// let capabilities = FileSystemCapabilities::new()
///     .with_guaranteed(FileSystemCapability::AtomicTreeCopy);
/// options.validate_capabilities(&capabilities)?;
///
/// # Ok::<(), qubit_fs::error::FsError>(())
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TreePublishOptions {
    /// Required namespace publication atomicity.
    pub atomicity: AtomicityRequirement,
    /// Required persistence strength after publication.
    pub durability: DurabilityRequirement,
}

impl TreePublishOptions {
    /// Rejects required guarantees known to be unsupported before publication
    /// begins. Conditional capabilities remain eligible for provider-specific
    /// preflight against the concrete staging request.
    ///
    /// # Parameters
    ///
    /// - `capabilities`: Provider capabilities used to preflight the required
    ///   guarantees.
    ///
    /// # Errors
    ///
    /// Returns an error when a required atomic or durable tree-copy capability
    /// is known to be unsupported. Conditional capabilities and preferred or
    /// unrequired guarantees do not fail this check.
    pub fn validate_capabilities(&self, capabilities: &FileSystemCapabilities) -> FsResult<()> {
        let required = match self.atomicity {
            AtomicityRequirement::Required => Some(FileSystemCapability::AtomicTreeCopy),
            AtomicityRequirement::Preferred | AtomicityRequirement::NotRequired => None,
        };
        if let Some(capability) = required {
            if capabilities.support(capability) == FileSystemCapabilitySupport::Unsupported {
                return Err(missing_tree_capability(capability));
            }
        }
        let required = match self.durability {
            DurabilityRequirement::Required => Some(FileSystemCapability::DurableTreeCopy),
            DurabilityRequirement::Preferred | DurabilityRequirement::NotRequired => None,
        };
        if let Some(capability) = required {
            if capabilities.support(capability) == FileSystemCapabilitySupport::Unsupported {
                return Err(missing_tree_capability(capability));
            }
        }
        Ok(())
    }
}

/// Builds a validation error for a required tree guarantee reported as
/// unsupported.
///
/// The error retains the capability so callers can explain which requirement
/// failed.
///
/// # Parameters
/// - `capability`: Required tree guarantee that the provider cannot supply.
///
/// # Returns
/// An unsupported-capability error naming the missing guarantee.
fn missing_tree_capability(capability: FileSystemCapability) -> FsError {
    FsError::new(
        FsErrorKind::UnsupportedCapability,
        FsOperation::ValidateProviderOutcome,
        "required tree publication guarantee is unsupported",
    )
    .with_required_capability(capability)
}
