// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Synchronous copy operation implementation.

use qubit_io::Output;

use super::CopyConflictPolicy;
use super::CopyFailure;
use super::CopyFailureState;
use super::CopyOptions;
use super::CopyOutcome;
use super::CopyStats;
use super::fallback_failure_stats;
use super::from_write_failure_state;
use super::from_writer_state;
use super::internal::CopyDeadline;
use super::internal::StreamCopyPlan;
use super::internal::from_completed_stats;
use crate::FileSystem;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::error::OpenFailureStage;
use crate::metadata::AchievedAtomicity;
use crate::metadata::FileSystemCapability;
use crate::path::Path;
use crate::read::ReadOptions;
use crate::spi::CopyAttempt;
use crate::spi::CopyRequest;
use crate::spi::ProviderOperation;
use crate::spi::ResolvedCopyOptions;
use crate::write::FileWriter;
use crate::write::WriterRecovery;
use crate::write::internal::is_unchanged_open_failure;
use crate::write::internal::open_failure_state;

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
    deadline: CopyDeadline,
}

impl<'a> CopyOperation<'a> {
    /// Creates a pending synchronous copy operation.
    #[inline]
    pub(crate) fn new(filesystem: &'a FileSystem, source: &'a Path, target: &'a Path, options: CopyOptions) -> Self {
        let deadline = CopyDeadline::new(options.deadline(), std::time::Instant::now);
        Self {
            filesystem,
            source,
            target,
            options,
            deadline,
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
                let outcome = self.verify_completed_copy(outcome)?;
                if let Some(error) = self.deadline_error() {
                    return Err(self.contextualize_failure(self.failure(
                        error,
                        from_completed_stats(outcome.stats()),
                        *outcome.stats(),
                        None,
                    )));
                }
                Ok(outcome)
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
                outcome.violation_failure_state(&self.options),
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
        let plan = StreamCopyPlan::new(
            &self.options,
            self.filesystem.properties().limits(),
            self.source,
            self.target,
        );
        plan.validate_options(self.filesystem.properties().symlink_policy())
            .map_err(|error| self.failure(error, CopyFailureState::Unchanged, CopyStats::default(), None))?;
        if let Some(error) = self.deadline_error() {
            return Err(self.failure(error, CopyFailureState::Unchanged, CopyStats::default(), None));
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
        if let Some(error) = self.deadline_error() {
            return Err(self.failure(error, CopyFailureState::Unchanged, CopyStats::default(), None));
        }
        plan.validate_metadata(&metadata)
            .map_err(|error| self.failure(error, CopyFailureState::Unchanged, CopyStats::default(), None))?;
        let mut reader = self
            .filesystem
            .open_reader(self.source, ReadOptions::default())
            .map_err(|error| self.failure(error, CopyFailureState::Unchanged, CopyStats::default(), None))?;
        if let Some(error) = self.deadline_error() {
            return Err(self.failure(error, CopyFailureState::Unchanged, CopyStats::default(), None));
        }
        let writer_options = plan.writer_options();
        let mut writer = match self.filesystem.open_writer(self.target, writer_options) {
            Ok(writer) => writer,
            Err(error)
                if error.stage() == OpenFailureStage::ProviderOpen
                    && error.recovery().is_none()
                    && error.error().kind() == FsErrorKind::AlreadyExists
                    && is_unchanged_open_failure(error.error())
                    && self.options.conflict() == CopyConflictPolicy::Skip =>
            {
                let stats = CopyStats {
                    skipped: 1,
                    ..CopyStats::default()
                };
                if let Some(error) = self.deadline_error() {
                    return Err(self.failure(error, CopyFailureState::Unchanged, stats, None));
                }
                return Ok(CopyOutcome::streamed_fallback(
                    stats,
                    AchievedAtomicity::NonAtomic,
                    false,
                ));
            }
            Err(error) => {
                let (error, stage, recovery) = error.into_parts();
                let state = match stage {
                    OpenFailureStage::Preflight => CopyFailureState::Unchanged,
                    OpenFailureStage::ProviderOpen => from_write_failure_state(open_failure_state(&error)),
                    OpenFailureStage::OutcomeValidation => CopyFailureState::Indeterminate,
                };
                return Err(CopyFailure::new(
                    error.with_operation(FsOperation::Copy),
                    state,
                    CopyStats::default(),
                    recovery.map(WriterRecovery::Rejected),
                ));
            }
        };
        if let Some(error) = self.deadline_error() {
            return Err(self.failure(
                error,
                from_writer_state(writer.state()),
                fallback_failure_stats(writer.written_bytes()),
                Some(writer),
            ));
        }
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
            let read =
                match crate::read::read_retry_interrupted(&mut reader, &mut buffer, || match self.deadline_error() {
                    Some(error) => Err(error.into_io_error()),
                    None => Ok(()),
                }) {
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
            if let Some(error) = self.deadline_error() {
                return Err(self.failure(
                    error,
                    from_writer_state(writer.state()),
                    fallback_failure_stats(writer.written_bytes()),
                    Some(writer),
                ));
            }
            if read == 0 {
                break;
            }
            let next_bytes = match plan.next_bytes(bytes, read) {
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
            if let Err(error) = Output::write_fully(&mut writer, &buffer[..read]) {
                return Err(self.failure(
                    self.io_error(self.target, FsOperation::Write, error),
                    from_writer_state(writer.state()),
                    fallback_failure_stats(writer.written_bytes()),
                    Some(writer),
                ));
            }
            if let Some(error) = self.deadline_error() {
                return Err(self.failure(
                    error,
                    from_writer_state(writer.state()),
                    fallback_failure_stats(writer.written_bytes()),
                    Some(writer),
                ));
            }
            bytes = next_bytes;
        }
        if let Some(error) = self.deadline_error() {
            return Err(self.failure(
                error,
                from_writer_state(writer.state()),
                fallback_failure_stats(writer.written_bytes()),
                Some(writer),
            ));
        }
        if let Err(error) = Output::flush(&mut writer) {
            return Err(self.failure(
                self.io_error(self.target, FsOperation::Write, error),
                from_writer_state(writer.state()),
                fallback_failure_stats(writer.written_bytes()),
                Some(writer),
            ));
        }
        if let Some(error) = self.deadline_error() {
            return Err(self.failure(
                error,
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
                if error.kind() == FsErrorKind::AlreadyExists && plan.may_skip_conflict(state) {
                    if let Err(cleanup_error) = writer.abort() {
                        return Err(self.failure(
                            cleanup_error,
                            from_writer_state(writer.state()),
                            fallback_failure_stats(writer.written_bytes()),
                            Some(writer),
                        ));
                    }
                    let stats = CopyStats {
                        skipped: 1,
                        ..CopyStats::default()
                    };
                    let abort_state = from_writer_state(writer.state());
                    if let Some(error) = self.deadline_error() {
                        return Err(self.failure(error, abort_state, stats, None));
                    }
                    if abort_state != CopyFailureState::Unchanged {
                        return Err(self.failure(error, abort_state, stats, None));
                    }
                    return Ok(CopyOutcome::streamed_fallback(
                        stats,
                        AchievedAtomicity::NonAtomic,
                        false,
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
        if let Some(error) = self.deadline_error() {
            return Err(self.failure(
                error,
                CopyFailureState::Published,
                StreamCopyPlan::completed_stats(metadata.kind(), bytes),
                None,
            ));
        }
        Ok(CopyOutcome::streamed_fallback(
            StreamCopyPlan::completed_stats(metadata.kind(), bytes),
            write_outcome.atomicity(),
            write_outcome.durable(),
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
        CopyFailure::new(
            error.with_operation(FsOperation::Copy),
            state,
            stats,
            writer.map(|writer| WriterRecovery::Opened(Box::new(writer))),
        )
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

    /// Returns a caller-budget error when the elapsed-time limit expired.
    fn deadline_error(&self) -> Option<FsError> {
        if self.deadline.expired() {
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

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::io::Cursor;
    use std::io::Result as IoResult;
    use std::time::Duration;
    use std::time::Instant;

    use qubit_io::Output;

    use super::CopyOperation;
    use crate::copy::CopyFailureState;
    use crate::copy::CopyOptions;
    use crate::error::FsError;
    use crate::error::FsErrorKind;
    use crate::error::FsOperation;
    use crate::error::FsResult;
    use crate::metadata::AchievedAtomicity;
    use crate::metadata::FileKind;
    use crate::metadata::FileMetadata;
    use crate::metadata::FileSystemCapabilities;
    use crate::metadata::FileSystemCapability;
    use crate::metadata::FileSystemId;
    use crate::metadata::FileSystemInfo;
    use crate::metadata::FileSystemLimits;
    use crate::metadata::OpenedFileInfo;
    use crate::metadata::PublicationMethod;
    use crate::metadata::SymlinkPolicy;
    use crate::metadata::WriteOutcome;
    use crate::path::Path;
    use crate::path::PathConstraints;
    use crate::path::PathSemantics;
    use crate::spi::FileSystemSpi;
    use crate::spi::FileWriterSpi;
    use crate::spi::OpenReaderRequest;
    use crate::spi::OpenWriterRequest;
    use crate::spi::OpenedReader;
    use crate::spi::OpenedWriter;
    use crate::spi::ProviderOperation;
    use crate::spi::ProviderOperations;
    use crate::spi::ProviderProperties;
    use crate::spi::SpiWriteFailure;
    use crate::spi::StatRequest;
    use crate::spi::StatResponse;
    use crate::write::WriteAbortOutcome;
    use crate::write::WriteFailureState;

    thread_local! {
        static NOW: Cell<Option<Instant>> = const { Cell::new(None) };
        static ADVANCE_ON_FLUSH: Cell<bool> = const { Cell::new(false) };
        static ADVANCE_ON_COMMIT: Cell<bool> = const { Cell::new(false) };
    }

    fn test_now() -> Instant {
        NOW.with(|now| now.get().expect("test clock should be initialized"))
    }

    fn advance_clock() {
        NOW.with(|now| {
            now.set(Some(
                now.get().expect("test clock should be initialized") + Duration::from_secs(1),
            ))
        });
    }

    struct StageSpi {
        late_commit_error: bool,
    }
    impl FileSystemSpi for StageSpi {
        fn properties(&self) -> ProviderProperties {
            ProviderProperties::new(
                FileSystemInfo::new(
                    FileSystemId::new("deadline-test").expect("valid id"),
                    "deadline-test",
                    PathSemantics::Hierarchical,
                ),
                ProviderOperations::new()
                    .with(ProviderOperation::Stat)
                    .with(ProviderOperation::OpenReader)
                    .with(ProviderOperation::OpenWriter),
                FileSystemCapabilities::new()
                    .with_guaranteed(FileSystemCapability::Read)
                    .with_guaranteed(FileSystemCapability::Write),
                FileSystemLimits::unknown(),
                PathConstraints::absolute(),
                SymlinkPolicy::Reject,
            )
            .expect("valid provider properties")
        }

        fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
            Ok(StatResponse::new(
                request.path().clone(),
                FileMetadata::new(FileKind::File).with_len(Some(1)),
            ))
        }

        fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
            let info = OpenedFileInfo::new(
                FileSystemId::new("deadline-test").expect("valid id"),
                request.path().clone(),
            )
            .with_metadata(FileMetadata::new(FileKind::File).with_len(Some(1)));
            Ok(OpenedReader::new(info, Box::new(Cursor::new(vec![b'x']))))
        }

        fn open_writer(&self, request: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
            let info = OpenedFileInfo::new(
                FileSystemId::new("deadline-test").expect("valid id"),
                request.path().clone(),
            )
            .with_metadata(FileMetadata::new(FileKind::File));
            Ok(OpenedWriter::new(
                info,
                Box::new(StageWriter {
                    late_commit_error: self.late_commit_error,
                }),
            ))
        }
    }

    struct StageWriter {
        late_commit_error: bool,
    }
    impl Output for StageWriter {
        type Item = u8;
        unsafe fn write_unchecked(&mut self, _: &[u8], _: usize, count: usize) -> IoResult<usize> {
            Ok(count)
        }
        fn flush(&mut self) -> IoResult<()> {
            if ADVANCE_ON_FLUSH.with(Cell::get) {
                advance_clock();
            }
            Ok(())
        }
    }
    impl FileWriterSpi for StageWriter {
        fn commit(&mut self) -> Result<WriteOutcome, SpiWriteFailure> {
            if ADVANCE_ON_COMMIT.with(Cell::get) {
                advance_clock();
            }
            if self.late_commit_error {
                return Err(SpiWriteFailure::new(
                    FsError::new(FsErrorKind::Io, FsOperation::CommitWriter, "late commit failure"),
                    WriteFailureState::RetryableNotPublished,
                ));
            }
            Ok(WriteOutcome::new(
                AchievedAtomicity::NonAtomic,
                PublicationMethod::Direct,
            ))
        }
        fn abort(&mut self) -> FsResult<WriteAbortOutcome> {
            Ok(WriteAbortOutcome::NotPublished)
        }
    }

    fn operation(late_commit_error: bool) -> CopyOperation<'static> {
        let filesystem = Box::leak(Box::new(
            crate::FileSystem::from_spi(StageSpi { late_commit_error }).expect("valid test filesystem"),
        ));
        let source = Box::leak(Box::new(Path::parse("/source").expect("valid source path")));
        let target = Box::leak(Box::new(Path::parse("/target").expect("valid target path")));
        let start = Instant::now();
        NOW.with(|now| now.set(Some(start)));
        let mut operation = CopyOperation::new(
            filesystem,
            source,
            target,
            CopyOptions::file().with_deadline(Some(Duration::from_secs(1))),
        );
        operation.deadline = super::CopyDeadline::new(Some(Duration::from_secs(1)), test_now);
        operation
    }

    #[test]
    fn test_copy_deadline_after_flush_retains_unpublished_writer_and_bytes() {
        ADVANCE_ON_FLUSH.with(|advance| advance.set(true));
        let failure = operation(false).execute().expect_err("deadline after flush must fail");
        ADVANCE_ON_FLUSH.with(|advance| advance.set(false));
        assert_eq!(CopyFailureState::Unchanged, failure.state());
        assert_eq!(1, failure.partial_stats().bytes);
        assert!(failure.has_recovery());
    }

    #[test]
    fn test_copy_deadline_after_commit_reports_published_without_recovery() {
        ADVANCE_ON_COMMIT.with(|advance| advance.set(true));
        let failure = operation(false).execute().expect_err("deadline after commit must fail");
        ADVANCE_ON_COMMIT.with(|advance| advance.set(false));
        assert_eq!(CopyFailureState::Published, failure.state());
        assert_eq!(1, failure.partial_stats().bytes);
        assert!(!failure.has_recovery());
    }

    #[test]
    fn test_copy_deadline_preserves_late_provider_commit_error() {
        ADVANCE_ON_COMMIT.with(|advance| advance.set(true));
        let failure = operation(true)
            .execute()
            .expect_err("provider commit error must remain primary");
        ADVANCE_ON_COMMIT.with(|advance| advance.set(false));
        assert_eq!(FsErrorKind::Io, failure.error().kind());
        assert_eq!(CopyFailureState::Unchanged, failure.state());
    }
}
