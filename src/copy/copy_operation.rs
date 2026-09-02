// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through facade
// copy fallback tests.
//! Synchronous copy operation implementation.

use std::time::Instant;

use qubit_io::Input;
use qubit_io::Output;

use super::CopyConflictPolicy;
use super::CopyFailure;
use super::CopyFailureState;
use super::CopyOptions;
use super::CopyOutcome;
use super::CopyStats;
use super::fallback_failure_stats;
use super::fallback_options_supported;
use super::from_write_failure_state;
use super::from_writer_state;
use super::is_file_kind_supported;
use super::validate_stream_copy_length_limits;
use crate::FileSystem;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::AchievedAtomicity;
use crate::metadata::FileSystemCapability;
use crate::path::Path;
use crate::read::ReadOptions;
use crate::spi::CopyAttempt;
use crate::spi::CopyRequest;
use crate::spi::ProviderOperation;
use crate::spi::ResolvedCopyOptions;
use crate::write::FileWriter;
use crate::write::WriteDisposition;
use crate::write::WriteOptions;

/// Executes one synchronous copy request and retains recovery state on failure.
pub(crate) struct CopyOperation<'a> {
    /// Facade that validates paths and dispatches provider calls.
    filesystem: &'a FileSystem,
    /// Validated source path.
    source: &'a Path,
    /// Validated target path.
    target: &'a Path,
    /// Requested copy policy.
    options: CopyOptions,
    /// Monotonic start used to enforce caller elapsed-time budgets.
    started_at: Instant,
}

impl<'a> CopyOperation<'a> {
    /// Creates a pending synchronous copy operation.
    #[inline]
    pub(crate) fn new(filesystem: &'a FileSystem, source: &'a Path, target: &'a Path, options: CopyOptions) -> Self {
        Self {
            filesystem,
            source,
            target,
            options,
            started_at: Instant::now(),
        }
    }

    /// Executes the provider attempt or the allowlisted stream fallback.
    #[allow(clippy::result_large_err)]
    pub(crate) fn execute(self) -> Result<CopyOutcome, CopyFailure> {
        if let Err(error) = self.copy_preflight() {
            return Err(self.contextualize_failure(self.failure(
                error,
                CopyFailureState::Unchanged,
                CopyStats::default(),
                None,
            )));
        }
        if let Some(error) = self.deadline_error() {
            return Err(self.contextualize_failure(self.failure(
                error,
                CopyFailureState::Unchanged,
                CopyStats::default(),
                None,
            )));
        }
        if self.options.max_entries() == Some(0) {
            return Err(self.contextualize_failure(self.failure(
                self.budget_error("copy entry limit was exceeded"),
                CopyFailureState::Unchanged,
                CopyStats::default(),
                None,
            )));
        }
        if self.filesystem.core().provider_supports(ProviderOperation::TryCopy) {
            self.execute_provider_attempt()
        } else {
            self.execute_stream_fallback()
                .map_err(|failure| self.contextualize_failure(failure))
        }
    }

    /// Dispatches the provider fast path and falls back only on a decline.
    #[allow(clippy::result_large_err)]
    fn execute_provider_attempt(&self) -> Result<CopyOutcome, CopyFailure> {
        match self.filesystem.spi().try_copy(CopyRequest::new(
            self.source,
            self.target,
            ResolvedCopyOptions::new(
                self.options.clone(),
                self.options
                    .symlink_policy_override()
                    .unwrap_or(self.filesystem.properties().symlink_policy()),
            ),
        )) {
            Ok(CopyAttempt::Completed(outcome)) => {
                if let Some(error) = self.deadline_error() {
                    return Err(self.contextualize_failure(self.failure(
                        error,
                        CopyFailureState::Published,
                        *outcome.stats(),
                        None,
                    )));
                }
                self.verify_completed_copy(outcome)
            }
            Err(failure) => {
                let (error, state, stats) = failure.into_parts();
                Err(self.contextualize_failure(self.failure(error, state, stats, None)))
            }
            Ok(CopyAttempt::Declined(_)) => self
                .execute_stream_fallback()
                .map_err(|failure| self.contextualize_failure(failure)),
        }
    }

    /// Verifies that provider success honors requested guarantees.
    #[allow(clippy::result_large_err)]
    fn verify_completed_copy(&self, outcome: CopyOutcome) -> Result<CopyOutcome, CopyFailure> {
        if let Some(message) = outcome.contract_violation(&self.options) {
            return Err(self.contextualize_failure(self.failure(
                FsError::new(FsErrorKind::ProviderContractViolation, FsOperation::Copy, message),
                CopyFailureState::Published,
                *outcome.stats(),
                None,
            )));
        }
        Ok(outcome)
    }

    /// Performs no-I/O validation before selecting a copy implementation.
    fn copy_preflight(&self) -> FsResult<()> {
        self.filesystem.core().validate_path(self.source, FsOperation::Copy)?;
        self.filesystem.core().validate_path(self.target, FsOperation::Copy)?;
        self.options
            .validate_against(self.filesystem.properties().capabilities())
            .map_err(|error| {
                self.filesystem
                    .core()
                    .enrich(error, Some(self.source), FsOperation::Copy)
                    .with_target(self.target.clone())
            })?;
        if self.source == self.target {
            return Err(FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::Copy,
                "copy source and target must differ",
            )
            .with_path(self.source.clone())
            .with_target(self.target.clone()));
        }
        Ok(())
    }

    /// Streams a copy through facade-owned handles when the provider declines.
    #[allow(clippy::result_large_err)]
    fn execute_stream_fallback(&self) -> Result<CopyOutcome, CopyFailure> {
        if !fallback_options_supported(&self.options) {
            return Err(self.failure(
                FsError::new(
                    FsErrorKind::RequirementNotMet,
                    FsOperation::Copy,
                    "declined copy cannot use the stream fallback for these options",
                ),
                CopyFailureState::Unchanged,
                CopyStats::default(),
                None,
            ));
        }
        if let Some(error) = self.deadline_error() {
            return Err(self.failure(error, CopyFailureState::Unchanged, CopyStats::default(), None));
        }
        if self.options.max_entries() == Some(0) {
            return Err(self.failure(
                self.budget_error("copy entry limit was exceeded"),
                CopyFailureState::Unchanged,
                CopyStats::default(),
                None,
            ));
        }
        self.filesystem
            .core()
            .require(FileSystemCapability::Read, FsOperation::Copy, Some(self.source))
            .and_then(|_| {
                self.filesystem
                    .core()
                    .require(FileSystemCapability::Write, FsOperation::Copy, Some(self.target))
            })
            .map_err(|error| self.failure(error, CopyFailureState::Unchanged, CopyStats::default(), None))?;
        let metadata = self
            .filesystem
            .stat(self.source)
            .map_err(|error| self.failure(error, CopyFailureState::Unchanged, CopyStats::default(), None))?;
        if !is_file_kind_supported(metadata.kind().clone()) {
            return Err(self.failure(
                FsError::new(
                    FsErrorKind::InvalidOptions,
                    FsOperation::Copy,
                    "stream fallback only supports regular files and objects",
                )
                .with_path(self.source.clone()),
                CopyFailureState::Unchanged,
                CopyStats::default(),
                None,
            ));
        }
        if let Some(length) = metadata.len()
            && let Err(error) = self.validate_fallback_length(length)
        {
            return Err(self.failure(error, CopyFailureState::Unchanged, CopyStats::default(), None));
        }
        let mut reader = self
            .filesystem
            .open_reader(self.source, ReadOptions::default())
            .map_err(|error| self.failure(error, CopyFailureState::Unchanged, CopyStats::default(), None))?;
        let writer_options = WriteOptions::default()
            .with_disposition(WriteDisposition::CreateNew)
            .with_atomicity(self.options.atomicity());
        let mut writer = match self.filesystem.open_writer(self.target, writer_options) {
            Ok(writer) => writer,
            Err(error)
                if error.kind() == FsErrorKind::AlreadyExists
                    && self.options.conflict() == CopyConflictPolicy::Skip =>
            {
                return Ok(CopyOutcome::streamed_fallback(
                    CopyStats {
                        skipped: 1,
                        ..CopyStats::default()
                    },
                    AchievedAtomicity::NonAtomic,
                ));
            }
            Err(error) => {
                return Err(self.failure(error, CopyFailureState::Unchanged, CopyStats::default(), None));
            }
        };
        let mut bytes = 0_u64;
        let mut buffer = [0_u8; 8192];
        loop {
            if let Some(error) = self.deadline_error() {
                return Err(self.failure(
                    error,
                    from_writer_state(writer.state()),
                    fallback_failure_stats(writer.written_bytes()),
                    Some(writer),
                ));
            }
            let read = match Input::read(&mut reader, &mut buffer) {
                Ok(read) => read,
                Err(error) => {
                    return Err(self.failure(
                        self.io_error(self.source, FsOperation::Read, error),
                        from_writer_state(writer.state()),
                        fallback_failure_stats(writer.written_bytes()),
                        Some(writer),
                    ));
                }
            };
            if read == 0 {
                break;
            }
            let next_bytes = match self.add_copied_bytes(bytes, read) {
                Ok(next_bytes) => next_bytes,
                Err(error) => {
                    return Err(self.failure(
                        error,
                        from_writer_state(writer.state()),
                        fallback_failure_stats(writer.written_bytes()),
                        Some(writer),
                    ));
                }
            };
            if self.options.max_bytes().is_some_and(|maximum| next_bytes > maximum) {
                return Err(self.failure(
                    self.budget_error("copy byte limit was exceeded"),
                    from_writer_state(writer.state()),
                    fallback_failure_stats(writer.written_bytes()),
                    Some(writer),
                ));
            }
            if let Err(error) = Output::write_fully(&mut writer, &buffer[..read]) {
                return Err(self.failure(
                    self.io_error(self.target, FsOperation::Write, error),
                    from_writer_state(writer.state()),
                    fallback_failure_stats(writer.written_bytes()),
                    Some(writer),
                ));
            }
            bytes = next_bytes;
        }
        if let Err(error) = Output::flush(&mut writer) {
            return Err(self.failure(
                self.io_error(self.target, FsOperation::Write, error),
                from_writer_state(writer.state()),
                fallback_failure_stats(writer.written_bytes()),
                Some(writer),
            ));
        }
        let write_outcome = match writer.commit() {
            Ok(outcome) => outcome,
            Err(failure) => {
                let (error, state) = failure.into_parts();
                let state = from_write_failure_state(state);
                if error.kind() == FsErrorKind::AlreadyExists
                    && self.options.conflict() == CopyConflictPolicy::Skip
                    && state == CopyFailureState::Unchanged
                {
                    if let Err(cleanup_error) = writer.abort() {
                        return Err(self.failure(
                            cleanup_error,
                            from_writer_state(writer.state()),
                            fallback_failure_stats(writer.written_bytes()),
                            Some(writer),
                        ));
                    }
                    return Ok(CopyOutcome::streamed_fallback(
                        CopyStats {
                            skipped: 1,
                            ..CopyStats::default()
                        },
                        AchievedAtomicity::NonAtomic,
                    ));
                }
                return Err(self.failure(
                    error,
                    state,
                    fallback_failure_stats(writer.written_bytes()),
                    Some(writer),
                ));
            }
        };
        Ok(CopyOutcome::streamed_fallback(
            CopyStats {
                files: 1,
                bytes,
                ..CopyStats::default()
            },
            write_outcome.atomicity(),
        ))
    }

    /// Builds a typed copy failure with operation context.
    fn failure(
        &self,
        error: FsError,
        state: CopyFailureState,
        stats: CopyStats,
        writer: Option<FileWriter>,
    ) -> CopyFailure {
        CopyFailure::new(error.with_operation(FsOperation::Copy), state, stats, writer)
    }

    /// Adds source, target, and provider facts to a copy failure.
    fn contextualize_failure(&self, failure: CopyFailure) -> CopyFailure {
        let (error, state, stats, writer) = failure.into_parts();
        CopyFailure::new(
            error.with_missing_context(
                self.source,
                Some(self.target),
                self.filesystem.properties().info().provider_id(),
            ),
            state,
            stats,
            writer,
        )
    }

    /// Creates a contextual stream I/O error.
    fn io_error(&self, path: &Path, operation: FsOperation, error: std::io::Error) -> FsError {
        FsError::from_stream_io(error, operation, path).with_provider(self.filesystem.properties().info().provider_id())
    }

    /// Adds a native read count to the public copy-statistics total.
    fn add_copied_bytes(&self, total: u64, count: usize) -> FsResult<u64> {
        let count = u64::try_from(count).map_err(|_| self.copy_byte_count_error())?;
        total.checked_add(count).ok_or_else(|| self.copy_byte_count_error())
    }

    /// Builds the error for an unrepresentable copy byte count.
    fn copy_byte_count_error(&self) -> FsError {
        FsError::new(
            FsErrorKind::ResourceLimitExceeded,
            FsOperation::Copy,
            "copy byte count exceeds the filesystem API reporting range",
        )
        .with_path(self.source.clone())
        .with_provider(self.filesystem.properties().info().provider_id())
    }

    /// Validates provider and caller size limits before opening fallback
    /// streams.
    fn validate_fallback_length(&self, length: u64) -> FsResult<()> {
        validate_stream_copy_length_limits(self.filesystem.properties().limits(), self.source, self.target, length)?;
        if self.options.max_bytes().is_some_and(|maximum| length > maximum) {
            return Err(self.budget_error("copy byte limit was exceeded"));
        }
        Ok(())
    }

    /// Returns a caller-budget error when the elapsed-time limit expired.
    fn deadline_error(&self) -> Option<FsError> {
        if self
            .options
            .deadline()
            .is_some_and(|deadline| self.started_at.elapsed() >= deadline)
        {
            return Some(self.budget_error("copy deadline was exceeded"));
        }
        None
    }

    /// Builds a caller-budget error with stable copy context.
    fn budget_error(&self, message: &str) -> FsError {
        FsError::new(FsErrorKind::ResourceLimitExceeded, FsOperation::Copy, message)
            .with_path(self.source.clone())
            .with_target(self.target.clone())
            .with_provider(self.filesystem.properties().info().provider_id())
    }
}
