// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Write operation options.

use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::metadata::AtomicityRequirement;
use crate::metadata::Checksum;
use crate::metadata::DurabilityRequirement;
use crate::metadata::FileSystemCapabilities;
use crate::metadata::FileSystemCapability;
use crate::metadata::NonSensitiveMetadata;
use crate::metadata::UserMetadata;
use crate::write::WriteDisposition;
use crate::write::WritePrecondition;

/// Options controlling a write operation.
///
/// # Examples
/// ```rust
/// use qubit_fs::write::WriteOptions;
/// use qubit_fs::metadata::AtomicityRequirement;
/// use qubit_fs::metadata::FileSystemCapabilities;
/// let options = WriteOptions::default().with_atomicity(AtomicityRequirement::Required);
/// assert!(options.validate_against(FileSystemCapabilities::new()).is_err());
/// ```
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct WriteOptions {
    /// Whether missing parent directories should be created.
    create_parent: bool,
    /// How an existing destination is treated.
    disposition: WriteDisposition,
    /// Required atomicity of destination publication.
    atomicity: AtomicityRequirement,
    /// Required durability of completed publication.
    durability: DurabilityRequirement,
    /// Version precondition applied to the destination.
    precondition: WritePrecondition,
    /// Optional content type.
    content_type: Option<String>,
    /// User-defined metadata with validated non-sensitive structural keys.
    user_metadata: NonSensitiveMetadata,
    /// Optional expected content checksum.
    checksum: Option<Checksum>,
}

impl Default for WriteOptions {
    #[inline]
    fn default() -> Self {
        Self {
            create_parent: false,
            disposition: WriteDisposition::default(),
            atomicity: AtomicityRequirement::default(),
            durability: DurabilityRequirement::default(),
            precondition: WritePrecondition::default(),
            content_type: None,
            user_metadata: NonSensitiveMetadata::new(),
            checksum: None,
        }
    }
}

impl WriteOptions {
    /// Returns a copy with parent creation replaced.
    #[inline]
    #[must_use]
    pub const fn with_create_parent(mut self, create: bool) -> Self {
        self.create_parent = create;
        self
    }

    /// Returns whether missing parent directories are created.
    #[inline]
    #[must_use]
    pub const fn create_parent(&self) -> bool {
        self.create_parent
    }

    /// Returns a copy with the destination disposition replaced.
    #[inline]
    #[must_use]
    pub const fn with_disposition(mut self, disposition: WriteDisposition) -> Self {
        self.disposition = disposition;
        self
    }

    /// Returns the destination disposition.
    #[inline]
    #[must_use]
    pub const fn disposition(&self) -> WriteDisposition {
        self.disposition
    }

    /// Returns a copy with the atomicity requirement replaced.
    #[inline]
    #[must_use]
    pub const fn with_atomicity(mut self, atomicity: AtomicityRequirement) -> Self {
        self.atomicity = atomicity;
        self
    }

    /// Returns the atomicity requirement.
    #[inline]
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

    /// Returns the completed-publication durability requirement.
    #[inline]
    #[must_use]
    pub const fn durability(&self) -> DurabilityRequirement {
        self.durability
    }

    /// Returns a copy with the version precondition replaced.
    #[inline]
    #[must_use]
    pub fn with_precondition(mut self, precondition: WritePrecondition) -> Self {
        self.precondition = precondition;
        self
    }

    /// Returns the version precondition.
    #[inline]
    #[must_use]
    pub const fn precondition(&self) -> &WritePrecondition {
        &self.precondition
    }

    /// Returns a copy with the content type replaced.
    #[inline]
    #[must_use]
    pub fn with_content_type(mut self, content_type: Option<String>) -> Self {
        self.content_type = content_type;
        self
    }

    /// Returns the optional content type.
    #[inline]
    #[must_use]
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    /// Returns the user metadata attached to this write.
    #[inline]
    #[must_use]
    pub const fn user_metadata(&self) -> &NonSensitiveMetadata {
        &self.user_metadata
    }

    /// Returns a copy with the expected checksum replaced.
    #[inline]
    #[must_use]
    pub fn with_checksum(mut self, checksum: Option<Checksum>) -> Self {
        self.checksum = checksum;
        self
    }

    /// Returns the optional expected checksum.
    #[inline]
    #[must_use]
    pub const fn checksum(&self) -> Option<&Checksum> {
        self.checksum.as_ref()
    }

    /// Replaces user-defined metadata that has already passed key validation.
    #[inline]
    #[must_use]
    pub fn with_user_metadata(mut self, metadata: UserMetadata) -> Self {
        self.user_metadata = NonSensitiveMetadata::from(metadata);
        self
    }

    /// Validates combinations that have no coherent provider interpretation.
    ///
    /// # Errors
    /// Returns [`FsErrorKind::InvalidOptions`] before a writer is opened when
    /// append is combined with atomic publication or a version precondition,
    /// or when create-new is combined with an existing-version requirement.
    #[inline]
    pub fn validate(&self) -> Result<(), FsError> {
        if self.disposition == WriteDisposition::Append
            && (self.atomicity == AtomicityRequirement::Required || self.precondition != WritePrecondition::None)
        {
            return Err(FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::OpenWriter,
                "append cannot require atomic publication or a destination version",
            ));
        }
        if self.disposition == WriteDisposition::CreateNew
            && matches!(&self.precondition, WritePrecondition::IfMatch(_))
        {
            return Err(FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::OpenWriter,
                "create-new cannot require an existing destination version",
            ));
        }
        Ok(())
    }

    /// Validates write options against stable configured capabilities.
    ///
    /// Providers should call this before opening a write session. In
    /// particular, required atomic publication is rejected before any staging
    /// write when [`FileSystemCapability::AtomicReplace`] is absent.
    ///
    /// # Errors
    ///
    /// Returns invalid-option errors from [`Self::validate`], or a typed
    /// [`FsErrorKind::RequirementNotMet`] for unsupported append, conditional,
    /// or required-atomic writes.
    pub fn validate_against(&self, capabilities: FileSystemCapabilities) -> Result<(), FsError> {
        self.validate()?;
        if self.disposition == WriteDisposition::Append && !capabilities.supports(FileSystemCapability::Append) {
            return Err(missing_requirement(
                FileSystemCapability::Append,
                "append writes are required but not supported",
            ));
        }
        if self.precondition != WritePrecondition::None
            && !capabilities.supports(FileSystemCapability::ConditionalWrite)
        {
            return Err(missing_requirement(
                FileSystemCapability::ConditionalWrite,
                "conditional writes are required but not supported",
            ));
        }
        if self.atomicity == AtomicityRequirement::Required
            && !capabilities.supports(FileSystemCapability::AtomicReplace)
        {
            return Err(missing_requirement(
                FileSystemCapability::AtomicReplace,
                "atomic write publication is required but not supported",
            ));
        }
        if self.durability == DurabilityRequirement::Required
            && !capabilities.supports(FileSystemCapability::DurableWrite)
        {
            return Err(missing_requirement(
                FileSystemCapability::DurableWrite,
                "durable write publication is required but not supported",
            ));
        }
        Ok(())
    }
}

/// Builds a typed unmet write requirement.
fn missing_requirement(capability: FileSystemCapability, message: &str) -> FsError {
    FsError::new(FsErrorKind::RequirementNotMet, FsOperation::OpenWriter, message).with_required_capability(capability)
}

#[cfg(test)]
mod tests {
    use std::hint::black_box;

    use super::WriteOptions;
    use crate::metadata::Checksum;
    use crate::metadata::ChecksumAlgorithm;
    use crate::metadata::NonSensitiveMetadata;
    use crate::write::WriteDisposition;
    use crate::write::WritePrecondition;

    #[test]
    fn option_accessors_are_executed_at_runtime() {
        let disposition: fn(&WriteOptions) -> WriteDisposition = black_box(WriteOptions::disposition);
        let precondition: for<'a> fn(&'a WriteOptions) -> &'a WritePrecondition = black_box(WriteOptions::precondition);
        let create_parent: fn(&WriteOptions) -> bool = black_box(WriteOptions::create_parent);
        let user_metadata: fn(&WriteOptions) -> &NonSensitiveMetadata = black_box(WriteOptions::user_metadata);
        let checksum: for<'a> fn(&'a WriteOptions) -> Option<&'a Checksum> = black_box(WriteOptions::checksum);

        let options = WriteOptions::default()
            .with_create_parent(true)
            .with_disposition(WriteDisposition::CreateOrReplace)
            .with_checksum(Some(Checksum::new(ChecksumAlgorithm::Sha256, "abc")));

        assert_eq!(WriteDisposition::CreateOrReplace, disposition(&options));
        assert!(matches!(precondition(&options), WritePrecondition::None));
        assert!(create_parent(&options));
        assert!(user_metadata(&options).is_empty());
        assert!(checksum(&options).is_some());
    }
}
