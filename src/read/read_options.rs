// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Read operation options.

use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::metadata::FileSystemCapabilities;
use crate::metadata::FileSystemCapability;
use crate::metadata::ResourceVersion;
use crate::read::ChecksumPolicy;

/// Options controlling a read operation.
#[non_exhaustive]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReadOptions {
    /// Optional byte offset.
    offset: Option<u64>,
    /// Optional byte length.
    length: Option<u64>,
    /// Optional required ETag or provider version.
    if_match: Option<ResourceVersion>,
    /// Optional ETag or provider version that must not match.
    if_none_match: Option<ResourceVersion>,
    /// Checksum validation policy.
    checksum: ChecksumPolicy,
}

impl ReadOptions {
    /// Returns a copy with the byte offset replaced.
    #[inline]
    #[must_use]
    pub const fn with_offset(mut self, offset: Option<u64>) -> Self {
        self.offset = offset;
        self
    }

    /// Returns the optional byte offset.
    #[inline(always)]
    #[must_use]
    pub const fn offset(&self) -> Option<u64> {
        self.offset
    }

    /// Returns a copy with the byte length replaced.
    #[inline]
    #[must_use]
    pub const fn with_length(mut self, length: Option<u64>) -> Self {
        self.length = length;
        self
    }

    /// Returns the optional byte length.
    #[inline(always)]
    #[must_use]
    pub const fn length(&self) -> Option<u64> {
        self.length
    }

    /// Returns a copy with the positive version precondition replaced.
    #[inline]
    #[must_use]
    pub fn with_if_match(mut self, if_match: Option<ResourceVersion>) -> Self {
        self.if_match = if_match;
        self
    }

    /// Returns the optional positive version precondition.
    #[inline(always)]
    #[must_use]
    pub const fn if_match(&self) -> Option<&ResourceVersion> {
        self.if_match.as_ref()
    }

    /// Returns a copy with the negative version precondition replaced.
    #[inline]
    #[must_use]
    pub fn with_if_none_match(mut self, if_none_match: Option<ResourceVersion>) -> Self {
        self.if_none_match = if_none_match;
        self
    }

    /// Returns the optional negative version precondition.
    #[inline(always)]
    #[must_use]
    pub const fn if_none_match(&self) -> Option<&ResourceVersion> {
        self.if_none_match.as_ref()
    }

    /// Returns a copy with the checksum policy replaced.
    #[inline]
    #[must_use]
    pub const fn with_checksum(mut self, checksum: ChecksumPolicy) -> Self {
        self.checksum = checksum;
        self
    }

    /// Returns the checksum policy.
    #[inline(always)]
    #[must_use]
    pub const fn checksum(&self) -> ChecksumPolicy {
        self.checksum
    }

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
        if (self.offset.is_some() || self.length.is_some()) && !capabilities.supports(FileSystemCapability::RangeRead) {
            return Err(missing_requirement(
                FileSystemCapability::RangeRead,
                "byte-range reads are required but not supported",
            ));
        }
        if (self.if_match.is_some() || self.if_none_match.is_some())
            && !capabilities.supports(FileSystemCapability::ConditionalRead)
        {
            return Err(missing_requirement(
                FileSystemCapability::ConditionalRead,
                "conditional reads are required but not supported",
            ));
        }
        if self.checksum == ChecksumPolicy::Required && !capabilities.supports(FileSystemCapability::ChecksumValidation)
        {
            return Err(missing_requirement(
                FileSystemCapability::ChecksumValidation,
                "checksum validation is required but not supported",
            ));
        }
        Ok(())
    }
}

/// Builds a typed unmet read requirement.
fn missing_requirement(capability: FileSystemCapability, message: &str) -> FsError {
    FsError::new(FsErrorKind::RequirementNotMet, FsOperation::OpenReader, message).with_required_capability(capability)
}
