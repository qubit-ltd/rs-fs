// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared, I/O-free planning for stream-copy fallback execution.

use super::stream_copy_policy::fallback_options_supported;
use super::stream_copy_policy::fallback_write_options;
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
        let count = count as u64;
        let next = total
            .checked_add(count)
            .ok_or_else(|| self.budget_error("copy byte count exceeds the filesystem API reporting range"))?;
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
}

#[cfg(test)]
mod tests {
    use super::StreamCopyPlan;
    use crate::copy::CopyConflictPolicy;
    use crate::copy::CopyFailureState;
    use crate::copy::CopyOptions;
    use crate::error::FsErrorKind;
    use crate::metadata::FileKind;
    use crate::metadata::FileMetadata;
    use crate::metadata::FileSystemLimit;
    use crate::metadata::FileSystemLimits;
    use crate::metadata::SymlinkPolicy;
    use crate::path::Path;

    fn plan(options: CopyOptions, limits: FileSystemLimits) -> StreamCopyPlan<'static> {
        let options = Box::leak(Box::new(options));
        let limits = Box::leak(Box::new(limits));
        let source = Box::leak(Box::new(Path::parse("/source").unwrap()));
        let target = Box::leak(Box::new(Path::parse("/target").unwrap()));
        StreamCopyPlan::new(options, limits, source, target)
    }

    #[test]
    fn validates_options_metadata_and_progress() {
        let default_plan = plan(CopyOptions::default(), FileSystemLimits::unknown());
        assert!(default_plan.validate_options(SymlinkPolicy::Reject).is_ok());
        let symlink = plan(
            CopyOptions::default().with_symlink_policy(SymlinkPolicy::Reject),
            FileSystemLimits::unknown(),
        );
        assert!(symlink.validate_options(SymlinkPolicy::FollowWithinFileSystem).is_err());
        assert!(
            default_plan
                .validate_metadata(&FileMetadata::new(FileKind::File))
                .is_ok()
        );
        assert_eq!(default_plan.next_bytes(2, 3).unwrap(), 5);
        assert_eq!(StreamCopyPlan::completed_stats(5).bytes, 5);
        assert!(!default_plan.may_skip_conflict(CopyFailureState::Unchanged));
        let _ = default_plan.writer_options();
    }

    #[test]
    fn rejects_unsupported_options_and_limits() {
        let tree = plan(CopyOptions::tree(), FileSystemLimits::unknown());
        assert_eq!(
            tree.validate_options(SymlinkPolicy::Reject).unwrap_err().kind(),
            FsErrorKind::RequirementNotMet
        );

        let zero = plan(
            CopyOptions::default().with_max_entries(Some(0)),
            FileSystemLimits::unknown(),
        );
        assert_eq!(
            zero.validate_options(SymlinkPolicy::Reject).unwrap_err().kind(),
            FsErrorKind::ResourceLimitExceeded
        );

        let metadata = plan(CopyOptions::default(), FileSystemLimits::unknown());
        assert_eq!(
            metadata
                .validate_metadata(&FileMetadata::new(FileKind::Directory))
                .unwrap_err()
                .kind(),
            FsErrorKind::InvalidOptions
        );
        let limited = plan(
            CopyOptions::default(),
            FileSystemLimits::unknown().with_max_write_bytes(FileSystemLimit::Maximum(4)),
        );
        assert_eq!(
            limited
                .validate_metadata(&FileMetadata::new(FileKind::File).with_len(Some(5)))
                .unwrap_err()
                .kind(),
            FsErrorKind::ResourceLimitExceeded
        );
        let budget = plan(
            CopyOptions::default().with_max_bytes(Some(4)),
            FileSystemLimits::unknown(),
        );
        assert_eq!(
            budget.next_bytes(3, 2).unwrap_err().kind(),
            FsErrorKind::ResourceLimitExceeded
        );
        assert_eq!(
            budget.next_bytes(u64::MAX, 1).unwrap_err().kind(),
            FsErrorKind::ResourceLimitExceeded
        );

        let object = plan(
            CopyOptions::default(),
            FileSystemLimits::unknown().with_max_write_bytes(FileSystemLimit::Maximum(5)),
        );
        assert!(
            object
                .validate_metadata(&FileMetadata::new(FileKind::Object).with_len(Some(5)))
                .is_ok()
        );

        let skip = plan(
            CopyOptions::default().with_conflict(CopyConflictPolicy::Skip),
            FileSystemLimits::unknown(),
        );
        assert!(skip.may_skip_conflict(CopyFailureState::Unchanged));
    }
}
