// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Write operation options.

use qubit_metadata::Metadata;

use crate::{
    AtomicityRequirement,
    Checksum,
    FileSystemCapabilities,
    FileSystemCapability,
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
    NonSensitiveMetadata,
    WriteDisposition,
    WritePrecondition,
};

/// Options controlling a write operation.
#[derive(Clone, Debug, PartialEq)]
pub struct WriteOptions {
    /// Whether missing parent directories should be created.
    pub create_parent: bool,
    /// How an existing destination is treated.
    pub disposition: WriteDisposition,
    /// Required atomicity of destination publication.
    pub atomicity: AtomicityRequirement,
    /// Version precondition applied to the destination.
    pub precondition: WritePrecondition,
    /// Optional content type.
    pub content_type: Option<String>,
    /// User-defined metadata with validated non-sensitive structural keys.
    pub user_metadata: NonSensitiveMetadata,
    /// Optional expected content checksum.
    pub checksum: Option<Checksum>,
}

impl Default for WriteOptions {
    #[inline]
    fn default() -> Self {
        Self {
            create_parent: false,
            disposition: WriteDisposition::default(),
            atomicity: AtomicityRequirement::default(),
            precondition: WritePrecondition::default(),
            content_type: None,
            user_metadata: NonSensitiveMetadata::new(),
            checksum: None,
        }
    }
}

impl WriteOptions {
    /// Replaces user-defined metadata after validating its structural keys.
    ///
    /// # Errors
    ///
    /// Returns an invalid-options error when a top-level key or a key nested
    /// in a string map or JSON object resembles credential material.
    #[inline]
    pub fn with_user_metadata(mut self, metadata: Metadata) -> FsResult<Self> {
        self.user_metadata = NonSensitiveMetadata::try_from_with_context(
            metadata,
            FsOperation::OpenWriter,
            "credential-like write metadata keys are forbidden",
        )?;
        Ok(self)
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
            && (self.atomicity == AtomicityRequirement::Required
                || self.precondition != WritePrecondition::None)
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
    pub fn validate_against(
        &self,
        capabilities: FileSystemCapabilities,
    ) -> Result<(), FsError> {
        self.validate()?;
        if self.disposition == WriteDisposition::Append
            && !capabilities.contains(FileSystemCapability::Append)
        {
            return Err(missing_requirement(
                FileSystemCapability::Append,
                "append writes are required but not guaranteed",
            ));
        }
        if self.precondition != WritePrecondition::None
            && !capabilities.contains(FileSystemCapability::ConditionalWrite)
        {
            return Err(missing_requirement(
                FileSystemCapability::ConditionalWrite,
                "conditional writes are required but not guaranteed",
            ));
        }
        if self.atomicity == AtomicityRequirement::Required
            && !capabilities.contains(FileSystemCapability::AtomicReplace)
        {
            return Err(missing_requirement(
                FileSystemCapability::AtomicReplace,
                "atomic write publication is required but not guaranteed",
            ));
        }
        Ok(())
    }
}

/// Builds a typed unmet write requirement.
fn missing_requirement(
    capability: FileSystemCapability,
    message: &str,
) -> FsError {
    FsError::new(
        FsErrorKind::RequirementNotMet,
        FsOperation::OpenWriter,
        message,
    )
    .with_required_capability(capability)
}
