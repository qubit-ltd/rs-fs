// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Owning asynchronous copy operation with cancellation-safe state tracking.

use std::time::Instant;

use qubit_io::AsyncInput;
use qubit_io::AsyncOutput;

use super::fallback_failure_stats;
use super::fallback_options_supported;
use super::from_writer_state;
use super::internal::CopyCancellationGuard;
use super::is_file_kind_supported;
use super::validate_stream_copy_length_limits;
use crate::AsyncFileSystem;
use crate::copy::AsyncCopyFailure;
use crate::copy::AsyncCopyOperationState;
use crate::copy::CopyConflictPolicy;
use crate::copy::CopyFailureState;
use crate::copy::CopyOptions;
use crate::copy::CopyOutcome;
use crate::copy::CopyStats;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::metadata::FileSystemCapability;
use crate::metadata::SymlinkPolicy;
use crate::path::Path;
use crate::read::ReadOptions;
use crate::spi::CopyAttempt;
use crate::spi::CopyRequest;
use crate::spi::ProviderOperation;
use crate::spi::ResolvedCopyOptions;
use crate::spi::SpiFuture;
use crate::write::AsyncFileWriter;
use crate::write::WriteDisposition;
use crate::write::WriteOptions;

/// An owning copy request whose recovery writer remains accessible after
/// failure.
pub struct AsyncCopyOperation {
    /// Facade used to validate and execute the pending operation.
    pub(crate) file_system: AsyncFileSystem,
    /// Immutable validated source path.
    source: Path,
    /// Immutable validated destination path.
    target: Path,
    /// Facade-resolved copy policy.
    options: ResolvedCopyOptions,
    /// Observable lifecycle state.
    state: AsyncCopyOperationState,
    /// Destination writer retained when recovery remains possible.
    writer: Option<Box<AsyncFileWriter>>,
    /// Monotonic start used to enforce caller elapsed-time budgets.
    started_at: Instant,
}

impl AsyncCopyOperation {
    /// Builds a preflight-validated operation without provider I/O.
    pub(crate) fn new(
        file_system: AsyncFileSystem,
        source: Path,
        target: Path,
        options: CopyOptions,
        symlink_policy: SymlinkPolicy,
    ) -> Self {
        Self {
            file_system,
            source,
            target,
            options: ResolvedCopyOptions::new(options, symlink_policy),
            state: AsyncCopyOperationState::Ready,
            writer: None,
            started_at: Instant::now(),
        }
    }

    /// Returns the immutable source path.
    ///
    /// # Returns
    /// The source path captured when the operation was created.
    #[inline(always)]
    #[must_use]
    pub const fn source(&self) -> &Path {
        &self.source
    }

    /// Returns the immutable destination path.
    #[inline(always)]
    #[must_use]
    pub const fn target(&self) -> &Path {
        &self.target
    }

    /// Returns the current operation lifecycle state.
    #[inline(always)]
    #[must_use]
    pub const fn state(&self) -> AsyncCopyOperationState {
        self.state
    }

    /// Returns whether a recovery writer is retained by this operation.
    #[inline(always)]
    #[must_use]
    pub const fn has_recovery_writer(&self) -> bool {
        self.writer.is_some()
    }

    /// Borrows the retained recovery writer, if one exists.
    #[inline(always)]
    pub fn recovery_writer(&mut self) -> Option<&mut AsyncFileWriter> {
        self.writer.as_deref_mut()
    }

    /// Takes ownership of the retained recovery writer, if one exists.
    #[inline(always)]
    pub fn take_recovery_writer(&mut self) -> Option<AsyncFileWriter> {
        self.writer.take().map(|writer| *writer)
    }

    /// Executes the operation exactly once.
    ///
    /// The operation becomes running only when this future is first polled.
    /// Dropping a polled pending future records indeterminate state and does
    /// not invoke provider cleanup or start any additional I/O.
    pub async fn execute(&mut self) -> Result<CopyOutcome, AsyncCopyFailure> {
        if self.state != AsyncCopyOperationState::Ready {
            return Err(invalid_state_failure(
                &self.source,
                &self.target,
                self.file_system.properties().info().provider_id(),
            ));
        }
        let Self {
            file_system,
            source,
            target,
            options,
            state,
            writer,
            started_at,
        } = self;
        let mut guard = CopyCancellationGuard::start(state, writer);
        let result = execute_copy(file_system, source, target, options, *started_at, guard.writer_mut()).await;
        guard.finish(&result);
        result
    }
}

/// Dispatches the asynchronous provider attempt and falls back only when the
/// provider explicitly declines it.
async fn execute_copy(
    filesystem: &AsyncFileSystem,
    source: &Path,
    target: &Path,
    options: &ResolvedCopyOptions,
    started_at: Instant,
    writer: &mut Option<Box<crate::write::AsyncFileWriter>>,
) -> Result<CopyOutcome, AsyncCopyFailure> {
    let caller_options = options.options();
    if caller_options.max_entries() == Some(0) {
        return Err(filesystem.contextual_copy_failure(
            budget_error(source, target, "copy entry limit was exceeded"),
            CopyFailureState::Unchanged,
            CopyStats::default(),
            source,
            target,
        ));
    }
    if deadline_expired(caller_options, started_at) {
        return Err(filesystem.contextual_copy_failure(
            budget_error(source, target, "copy deadline was exceeded"),
            CopyFailureState::Unchanged,
            CopyStats::default(),
            source,
            target,
        ));
    }
    if !filesystem.core().provider_supports(ProviderOperation::TryCopy) {
        return stream_copy_fallback(filesystem, source, target, options, started_at, writer).await;
    }
    match filesystem
        .spi()
        .try_copy(CopyRequest::new(source, target, options.clone()))
        .await
    {
        Ok(CopyAttempt::Completed(outcome)) => {
            if deadline_expired(caller_options, started_at) {
                return Err(filesystem.contextual_copy_failure(
                    budget_error(source, target, "copy deadline was exceeded"),
                    CopyFailureState::Published,
                    *outcome.stats(),
                    source,
                    target,
                ));
            }
            filesystem.verify_completed_copy(outcome, options.options(), source, target)
        }
        Ok(CopyAttempt::Declined(_)) => {
            stream_copy_fallback(filesystem, source, target, options, started_at, writer).await
        }
        Err(failure) => {
            let (error, state, stats) = failure.into_parts();
            Err(filesystem.contextual_copy_failure(error, state, stats, source, target))
        }
    }
}

/// Streams a declined asynchronous copy while retaining any recovery writer
/// in `writer_slot` until publication or cleanup completes.
fn stream_copy_fallback<'a>(
    filesystem: &'a AsyncFileSystem,
    source: &'a Path,
    target: &'a Path,
    options: &'a ResolvedCopyOptions,
    started_at: Instant,
    writer_slot: &'a mut Option<Box<crate::write::AsyncFileWriter>>,
) -> SpiFuture<'a, Result<CopyOutcome, AsyncCopyFailure>> {
    Box::pin(async move {
        let options = options.options();
        if !fallback_options_supported(options, filesystem.properties().symlink_policy()) {
            return Err(filesystem.contextual_copy_failure(
                FsError::new(
                    FsErrorKind::RequirementNotMet,
                    FsOperation::Copy,
                    "declined copy cannot use the stream fallback for these options",
                ),
                CopyFailureState::Unchanged,
                CopyStats::default(),
                source,
                target,
            ));
        }
        if options.max_entries() == Some(0) {
            return Err(filesystem.contextual_copy_failure(
                budget_error(source, target, "copy entry limit was exceeded"),
                CopyFailureState::Unchanged,
                CopyStats::default(),
                source,
                target,
            ));
        }
        if deadline_expired(options, started_at) {
            return Err(filesystem.contextual_copy_failure(
                budget_error(source, target, "copy deadline was exceeded"),
                CopyFailureState::Unchanged,
                CopyStats::default(),
                source,
                target,
            ));
        }
        filesystem
            .require(FileSystemCapability::Read, FsOperation::Copy, source)
            .and_then(|_| filesystem.require(FileSystemCapability::Write, FsOperation::Copy, target))
            .map_err(|error| {
                filesystem.contextual_copy_failure(
                    error,
                    CopyFailureState::Unchanged,
                    CopyStats::default(),
                    source,
                    target,
                )
            })?;
        let metadata = filesystem.stat(source).await.map_err(|error| {
            filesystem.contextual_copy_failure(error, CopyFailureState::Unchanged, CopyStats::default(), source, target)
        })?;
        if !is_file_kind_supported(metadata.kind().clone()) {
            return Err(filesystem.contextual_copy_failure(
                FsError::new(
                    FsErrorKind::InvalidOptions,
                    FsOperation::Copy,
                    "stream fallback only supports regular files and objects",
                ),
                CopyFailureState::Unchanged,
                CopyStats::default(),
                source,
                target,
            ));
        }
        if let Some(length) = metadata.len()
            && let Err(error) =
                validate_stream_copy_length_limits(filesystem.properties().limits(), source, target, length)
        {
            return Err(filesystem.contextual_copy_failure(
                error,
                CopyFailureState::Unchanged,
                CopyStats::default(),
                source,
                target,
            ));
        }
        if let Some(length) = metadata.len()
            && options.max_bytes().is_some_and(|maximum| length > maximum)
        {
            return Err(filesystem.contextual_copy_failure(
                budget_error(source, target, "copy byte limit was exceeded"),
                CopyFailureState::Unchanged,
                CopyStats::default(),
                source,
                target,
            ));
        }
        let mut reader = filesystem
            .open_reader(source, ReadOptions::default())
            .await
            .map_err(|error| {
                filesystem.contextual_copy_failure(
                    error,
                    CopyFailureState::Unchanged,
                    CopyStats::default(),
                    source,
                    target,
                )
            })?;
        let writer_options = WriteOptions::default()
            .with_disposition(WriteDisposition::CreateNew)
            .with_atomicity(options.atomicity());
        match filesystem.open_writer(target, writer_options).await {
            Ok(writer) => *writer_slot = Some(Box::new(writer)),
            Err(error)
                if error.kind() == FsErrorKind::AlreadyExists && options.conflict() == CopyConflictPolicy::Skip =>
            {
                return Ok(CopyOutcome::streamed_fallback(
                    CopyStats {
                        skipped: 1,
                        ..CopyStats::default()
                    },
                    crate::metadata::AchievedAtomicity::NonAtomic,
                ));
            }
            Err(error) => {
                return Err(filesystem.contextual_copy_failure(
                    error,
                    CopyFailureState::Unchanged,
                    CopyStats::default(),
                    source,
                    target,
                ));
            }
        }
        let mut bytes = 0_u64;
        let mut buffer = [0_u8; 8192];
        loop {
            if deadline_expired(options, started_at) {
                let writer = writer_slot.as_ref().expect("writer is retained before transfer");
                return Err(filesystem.contextual_copy_failure(
                    budget_error(source, target, "copy deadline was exceeded"),
                    from_writer_state(writer.state()),
                    fallback_failure_stats(writer.written_bytes()),
                    source,
                    target,
                ));
            }
            let read = reader.read_async(&mut buffer).await.map_err(|error| {
                filesystem.contextual_copy_failure(
                    FsError::from_stream_io(error, FsOperation::Read, source),
                    from_writer_state(
                        writer_slot
                            .as_ref()
                            .expect("writer is retained before transfer")
                            .state(),
                    ),
                    fallback_failure_stats(
                        writer_slot
                            .as_ref()
                            .expect("writer is retained before transfer")
                            .written_bytes(),
                    ),
                    source,
                    target,
                )
            })?;
            if read == 0 {
                break;
            }
            let writer = writer_slot.as_mut().expect("writer is retained before transfer");
            let next_bytes = filesystem.add_copied_bytes(bytes, read, source).map_err(|error| {
                filesystem.contextual_copy_failure(
                    error,
                    from_writer_state(writer.state()),
                    fallback_failure_stats(writer.written_bytes()),
                    source,
                    target,
                )
            })?;
            if options.max_bytes().is_some_and(|maximum| next_bytes > maximum) {
                return Err(filesystem.contextual_copy_failure(
                    budget_error(source, target, "copy byte limit was exceeded"),
                    from_writer_state(writer.state()),
                    fallback_failure_stats(writer.written_bytes()),
                    source,
                    target,
                ));
            }
            writer.write_fully_async(&buffer[..read]).await.map_err(|error| {
                filesystem.contextual_copy_failure(
                    FsError::from_stream_io(error, FsOperation::Write, target),
                    from_writer_state(writer.state()),
                    fallback_failure_stats(writer.written_bytes()),
                    source,
                    target,
                )
            })?;
            bytes = next_bytes;
        }
        let writer = writer_slot.as_mut().expect("writer is retained before flush");
        writer.flush_async().await.map_err(|error| {
            filesystem.contextual_copy_failure(
                FsError::from_stream_io(error, FsOperation::Write, target),
                from_writer_state(writer.state()),
                fallback_failure_stats(writer.written_bytes()),
                source,
                target,
            )
        })?;
        let writer = writer_slot.as_mut().expect("writer is retained before commit");
        let write_outcome = match writer.commit_async().await {
            Ok(outcome) => outcome,
            Err(failure)
                if failure.error().kind() == FsErrorKind::AlreadyExists
                    && options.conflict() == CopyConflictPolicy::Skip
                    && from_writer_state(writer.state()) == CopyFailureState::Unchanged =>
            {
                if let Err(cleanup_error) = writer.abort_async().await {
                    return Err(filesystem.contextual_copy_failure(
                        cleanup_error,
                        from_writer_state(writer.state()),
                        fallback_failure_stats(writer.written_bytes()),
                        source,
                        target,
                    ));
                }
                let _ = writer_slot.take();
                return Ok(CopyOutcome::streamed_fallback(
                    CopyStats {
                        skipped: 1,
                        ..CopyStats::default()
                    },
                    crate::metadata::AchievedAtomicity::NonAtomic,
                ));
            }
            Err(failure) => {
                return Err(filesystem.contextual_copy_failure(
                    failure.into_error(),
                    from_writer_state(writer.state()),
                    fallback_failure_stats(writer.written_bytes()),
                    source,
                    target,
                ));
            }
        };
        let _ = writer_slot.take();
        Ok(CopyOutcome::streamed_fallback(
            CopyStats {
                files: 1,
                bytes,
                ..CopyStats::default()
            },
            write_outcome.atomicity(),
        ))
    })
}

/// Returns whether the caller elapsed-time budget has expired.
fn deadline_expired(options: &CopyOptions, started_at: Instant) -> bool {
    options
        .deadline()
        .is_some_and(|deadline| started_at.elapsed() >= deadline)
}

/// Builds a caller-budget error for an asynchronous copy.
fn budget_error(source: &Path, target: &Path, message: &str) -> FsError {
    FsError::new(FsErrorKind::ResourceLimitExceeded, FsOperation::Copy, message)
        .with_path(source.clone())
        .with_target(target.clone())
}

/// Builds the stable failure used for an invalid execute retry.
fn invalid_state_failure(source: &Path, target: &Path, provider: &str) -> AsyncCopyFailure {
    AsyncCopyFailure::new(
        FsError::new(
            FsErrorKind::InvalidState,
            FsOperation::Copy,
            "copy operation cannot execute in its current state",
        )
        .with_path(source.clone())
        .with_target(target.clone())
        .with_provider(provider),
        CopyFailureState::Unchanged,
        CopyStats::default(),
    )
}
