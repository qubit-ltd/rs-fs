// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Temporary resource persistence options.

use crate::copy::MetadataPreservePolicy;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::metadata::AtomicityRequirement;
use crate::metadata::FileSystemCapabilities;
use crate::metadata::FileSystemCapability;

/// Options controlling temporary resource persistence.
///
/// Defaults are `overwrite = false`, `atomicity = Required`,
/// `preserve_metadata = Portable`, and `create_parent = false`. Required
/// atomicity does not establish a durability guarantee. When the application
/// can accept weaker atomicity, it may request `Preferred`; that request does
/// not guarantee that persistence will succeed.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::metadata::AtomicityRequirement;
/// use qubit_fs::temp::PersistOptions;
///
/// let defaults = PersistOptions::default();
/// assert!(!defaults.overwrite());
/// assert_eq!(AtomicityRequirement::Required, defaults.atomicity());
/// assert_eq!(qubit_fs::copy::MetadataPreservePolicy::Portable, defaults.preserve_metadata());
/// assert!(!defaults.creates_parent());
///
/// // Use only when the application can accept a preferred atomicity level.
/// let preferred = defaults.with_atomicity(AtomicityRequirement::Preferred);
/// assert_eq!(AtomicityRequirement::Preferred, preferred.atomicity());
/// ```
#[must_use = "use the persistence options to configure temporary resource persistence"]
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistOptions {
    /// Whether the destination may be overwritten.
    overwrite: bool,
    /// Required atomicity level.
    atomicity: AtomicityRequirement,
    /// Metadata preservation policy requested for persistence.
    preserve_metadata: MetadataPreservePolicy,
    /// Whether a missing destination parent is created before publication.
    create_parent: bool,
}

impl PersistOptions {
    /// Returns whether the destination may be overwritten.
    ///
    /// # Returns
    /// `true` when persistence may replace an existing destination, or
    /// `false` when it must not overwrite one.
    #[inline]
    #[must_use]
    pub const fn overwrite(&self) -> bool {
        self.overwrite
    }

    /// Returns the required atomicity level.
    ///
    /// # Returns
    /// The atomicity requirement applied to destination publication.
    #[inline]
    #[must_use = "review the atomicity requirement before publishing the destination"]
    pub const fn atomicity(&self) -> AtomicityRequirement {
        self.atomicity
    }

    /// Returns the metadata preservation policy requested from the provider.
    ///
    /// See [`MetadataPreservePolicy`](crate::copy::MetadataPreservePolicy) for
    /// the category semantics. The provider documents the fields supported for
    /// persistence; [`PersistOutcome`](super::PersistOutcome) does not provide
    /// field-by-field evidence that metadata was preserved.
    ///
    /// # Returns
    /// The category requested from the provider for this persistence operation.
    #[inline]
    #[must_use = "use the requested metadata preservation policy"]
    pub const fn preserve_metadata(&self) -> MetadataPreservePolicy {
        self.preserve_metadata
    }

    /// Returns whether missing destination parents are created.
    ///
    /// # Returns
    /// `true` when a missing parent is created before publication, or `false`
    /// when it must already exist.
    #[inline]
    #[must_use]
    pub const fn creates_parent(&self) -> bool {
        self.create_parent
    }

    /// Enables recursive creation of a missing destination parent.
    ///
    /// # Returns
    /// Updated persistence options that create missing destination parents.
    #[inline]
    #[must_use = "apply the updated parent-creation setting"]
    pub const fn with_create_parent(mut self) -> Self {
        self.create_parent = true;
        self
    }

    /// Replaces whether the destination may be overwritten.
    ///
    /// # Parameters
    /// - `overwrite`: Whether an existing destination can be replaced.
    ///
    /// # Returns
    /// Updated persistence options with the requested overwrite behavior.
    #[inline]
    #[must_use = "apply the updated overwrite policy"]
    pub const fn with_overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = overwrite;
        self
    }

    /// Replaces the required atomicity level.
    ///
    /// # Parameters
    /// - `atomicity`: Atomicity requirement for publication.
    ///
    /// # Returns
    /// Updated persistence options with the requested atomicity requirement.
    #[inline]
    #[must_use = "apply the updated atomicity requirement"]
    pub const fn with_atomicity(mut self, atomicity: AtomicityRequirement) -> Self {
        self.atomicity = atomicity;
        self
    }

    /// Replaces the metadata preservation policy requested for persistence.
    ///
    /// The policy selects a category; it does not guarantee a common field set
    /// across providers. Consult the provider's persistence documentation for
    /// supported, best-effort, and excluded fields. The resulting
    /// [`PersistOutcome`](super::PersistOutcome) does not verify individual
    /// metadata fields.
    ///
    /// # Parameters
    /// - `preserve_metadata`: Metadata-preservation category requested from the
    ///   provider.
    ///
    /// # Returns
    /// Updated persistence options requesting the specified metadata category.
    #[inline]
    #[must_use = "apply the updated metadata preservation policy"]
    pub const fn with_preserve_metadata(mut self, preserve_metadata: MetadataPreservePolicy) -> Self {
        self.preserve_metadata = preserve_metadata;
        self
    }

    /// Validates required persistence guarantees before provider side effects.
    ///
    /// # Parameters
    /// - `capabilities`: Capabilities advertised by the configured filesystem.
    ///
    /// # Returns
    /// `Ok(())` when required guarantees are supported.
    ///
    /// # Errors
    /// Returns [`FsErrorKind::RequirementNotMet`] when atomic persistence is
    /// required but the configured filesystem does not guarantee it.
    pub fn validate_against(&self, capabilities: FileSystemCapabilities) -> Result<(), FsError> {
        if self.atomicity() == AtomicityRequirement::Required
            && !capabilities.supports(FileSystemCapability::AtomicTempPersist)
        {
            return Err(FsError::new(
                FsErrorKind::RequirementNotMet,
                FsOperation::PersistTemp,
                "atomic temporary persistence is required but not supported",
            )
            .with_required_capability(FileSystemCapability::AtomicTempPersist));
        }
        Ok(())
    }
}

impl Default for PersistOptions {
    /// Creates options requiring atomic persistence without overwrite.
    ///
    /// # Returns
    /// Options with `overwrite = false`, required atomicity, portable metadata
    /// preservation, and missing-parent creation disabled.
    #[inline]
    fn default() -> Self {
        Self {
            overwrite: false,
            atomicity: AtomicityRequirement::Required,
            preserve_metadata: MetadataPreservePolicy::Portable,
            create_parent: false,
        }
    }
}
