// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared, I/O-free planning for stream-copy fallback execution.

use crate::copy::CopyConflictPolicy;
use crate::copy::CopyFailureState;
use crate::copy::CopyOptions;
use crate::copy::CopyStats;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::metadata::FileKind;
use crate::metadata::FileMetadata;
use crate::metadata::FileSystemLimits;
use crate::metadata::SymlinkPolicy;
use crate::path::Path;
use crate::write::WriteOptions;

use super::stream_copy_policy::fallback_options_supported;
use super::stream_copy_policy::fallback_write_options;

/// Immutable policy and progress decisions shared by sync and async fallback.
pub(crate) struct StreamCopyPlan<'a> {
    options: &'a CopyOptions,
    limits: &'a FileSystemLimits,
    source: &'a Path,
    target: &'a Path,
}

impl<'a> StreamCopyPlan<'a> {
    /// Creates a plan for one validated copy request.
    #[inline]
    pub(crate) const fn new(
        options: &'a CopyOptions,
        limits: &'a FileSystemLimits,
        source: &'a Path,
        target: &'a Path,
    ) -> Self {
        Self {
            options,
            limits,
            source,
            target,
        }
    }

    /// Validates options that the stream fallback can faithfully implement.
    pub(crate) fn validate_options(&self, symlink_policy: SymlinkPolicy) -> Result<(), FsError> {
        if !fallback_options_supported(self.options, symlink_policy) {
            return Err(FsError::new(
                FsErrorKind::RequirementNotMet,
                FsOperation::Copy,
                "declined copy cannot use the stream fallback for these options",
            ));
        }
        if self.options.max_entries() == Some(0) {
            return Err(self.budget_error("copy entry limit was exceeded"));
        }
        Ok(())
    }

    /// Validates source metadata before opening either stream handle.
    pub(crate) fn validate_metadata(&self, metadata: &FileMetadata) -> Result<(), FsError> {
        if !matches!(metadata.kind(), FileKind::File | FileKind::Object) {
            return Err(FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::Copy,
                "stream fallback only supports regular files and objects",
            ));
        }
        if let Some(length) = metadata.len() {
            super::stream_copy_policy::validate_stream_copy_length_limits(
                self.limits,
                self.source,
                self.target,
                length,
            )?;
            if self.options.max_bytes().is_some_and(|maximum| length > maximum) {
                return Err(self.budget_error("copy byte limit was exceeded"));
            }
        }
        Ok(())
    }

    /// Builds the writer request used by the fallback.
    #[inline]
    pub(crate) fn writer_options(&self) -> WriteOptions {
        fallback_write_options(self.options)
    }

    /// Adds a read count and enforces the caller byte budget.
    pub(crate) fn next_bytes(&self, total: u64, count: usize) -> Result<u64, FsError> {
        let count = u64::try_from(count).map_err(|_| self.byte_count_error())?;
        let next = total.checked_add(count).ok_or_else(|| self.byte_count_error())?;
        if self.options.max_bytes().is_some_and(|maximum| next > maximum) {
            return Err(self.budget_error("copy byte limit was exceeded"));
        }
        Ok(next)
    }

    /// Returns the fallback statistics for a completed regular-file copy.
    #[inline]
    pub(crate) fn completed_stats(bytes: u64) -> CopyStats {
        CopyStats {
            files: 1,
            bytes,
            ..CopyStats::default()
        }
    }

    /// Returns whether an unchanged conflict may be reported as skipped.
    #[inline]
    pub(crate) fn may_skip_conflict(&self, state: CopyFailureState) -> bool {
        self.options.conflict() == CopyConflictPolicy::Skip && state == CopyFailureState::Unchanged
    }

    /// Builds a stable caller-budget error with source and target context.
    pub(crate) fn budget_error(&self, message: &str) -> FsError {
        FsError::new(FsErrorKind::ResourceLimitExceeded, FsOperation::Copy, message)
            .with_path(self.source.clone())
            .with_target(self.target.clone())
    }

    fn byte_count_error(&self) -> FsError {
        FsError::new(
            FsErrorKind::ResourceLimitExceeded,
            FsOperation::Copy,
            "copy byte count exceeds the filesystem API reporting range",
        )
        .with_path(self.source.clone())
        .with_target(self.target.clone())
    }
}
