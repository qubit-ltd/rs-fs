// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Read operation options.

use crate::{
    ChecksumPolicy, FileSystemCapabilities, FileSystemCapability, FsError, FsErrorKind, FsOperation,
};

/// Options controlling a read operation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReadOptions {
    /// Optional byte offset.
    pub offset: Option<u64>,
    /// Optional byte length.
    pub length: Option<u64>,
    /// Optional required ETag or provider version.
    pub if_match: Option<String>,
    /// Optional ETag or provider version that must not match.
    pub if_none_match: Option<String>,
    /// Checksum validation policy.
    pub checksum: ChecksumPolicy,
}

impl ReadOptions {
    /// Validates required read semantics against configured capabilities.
    ///
    /// Providers should call this method before opening a reader or producing
    /// any observable side effect.
    ///
    /// # Errors
    ///
    /// Returns [`FsErrorKind::InvalidOptions`] for mutually exclusive version
    /// conditions, or [`FsErrorKind::RequirementNotMet`] with the exact
    /// missing capability for range, conditional, or required-checksum reads.
    pub fn validate_against(&self, capabilities: FileSystemCapabilities) -> Result<(), FsError> {
        if self.if_match.is_some() && self.if_none_match.is_some() {
            return Err(FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::OpenReader,
                "if_match and if_none_match cannot both be specified",
            ));
        }
        if (self.offset.is_some() || self.length.is_some())
            && !capabilities.contains(FileSystemCapability::RangeRead)
        {
            return Err(missing_requirement(
                FileSystemCapability::RangeRead,
                "byte-range reads are required but not guaranteed",
            ));
        }
        if (self.if_match.is_some() || self.if_none_match.is_some())
            && !capabilities.contains(FileSystemCapability::ConditionalRead)
        {
            return Err(missing_requirement(
                FileSystemCapability::ConditionalRead,
                "conditional reads are required but not guaranteed",
            ));
        }
        if self.checksum == ChecksumPolicy::Required
            && !capabilities.contains(FileSystemCapability::ChecksumValidation)
        {
            return Err(missing_requirement(
                FileSystemCapability::ChecksumValidation,
                "checksum validation is required but not guaranteed",
            ));
        }
        Ok(())
    }
}

/// Builds a typed unmet read requirement.
fn missing_requirement(capability: FileSystemCapability, message: &str) -> FsError {
    FsError::new(
        FsErrorKind::RequirementNotMet,
        FsOperation::OpenReader,
        message,
    )
    .with_required_capability(capability)
}
