// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Owning asynchronous copy operation with cancellation-safe state tracking.

use qubit_io::AsyncInput;
use qubit_io::AsyncOutput;

use super::fallback_failure_stats;
use super::from_writer_state;
use super::internal::CopyCancellationGuard;
use super::internal::CopyDeadline;
use super::internal::CopyRecoverySnapshot;
use super::internal::StreamCopyPlan;
use super::internal::from_completed_stats;
use crate::AsyncFileSystem;
use crate::copy::AsyncCopyFailure;
use crate::copy::AsyncCopyOperationState;
use crate::copy::CopyConflictPolicy;
use crate::copy::CopyFailureState;
use crate::copy::CopyOptions;
use crate::copy::CopyOutcome;
use crate::copy::CopyStats;
use crate::copy::internal::from_write_failure_state;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::OpenFailureStage;
use crate::metadata::FileSystemCapability;
use crate::metadata::SymlinkPolicy;
use crate::path::Path;
use crate::read::ReadOptions;
use crate::spi::CopyAttempt;
use crate::spi::CopyRequest;
use crate::spi::ProviderOperation;
use crate::spi::ResolvedCopyOptions;
use crate::spi::SpiFuture;
use crate::write::AsyncWriterRecovery;
use crate::write::internal::is_unchanged_open_failure;
use crate::write::internal::open_failure_state;

/// An owning copy request whose recovery writer remains accessible after
/// failure.
///
/// # Examples
///
/// This example uses an isolated in-memory provider fixture.
/// The operation remains owned by the caller after an execution error, so the
/// caller can inspect its state and take any retained writer recovery.
///
/// ```rust
/// # mod support { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/common/rustdoc_support.rs")); }
/// # use support::*;
/// # let (filesystem, _) = async_recording_spi::async_recording_file_system(Default::default());
/// # poll_support::ready(async {
/// use qubit_fs::Path;
/// use qubit_fs::copy::CopyOptions;
///
/// let mut operation = filesystem.begin_copy(
///     Path::parse("/source")?, Path::parse("/target")?, CopyOptions::default(),
/// )?;
/// let outcome = operation.execute().await?;
/// assert_eq!(5, outcome.stats().bytes);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// # }).unwrap();
/// ```
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
    writer: Option<AsyncWriterRecovery>,
    /// Monotonic start used to enforce caller elapsed-time budgets.
    deadline: CopyDeadline,
    /// Publication state and progress retained for cancellation recovery.
    recovery: CopyRecoverySnapshot,
}

impl AsyncCopyOperation {
    /// Builds a preflight-validated operation without provider I/O.
    ///
    /// # Parameters
    /// - `file_system`: Facade that will validate and execute the operation.
    /// - `source`: Validated logical source path.
    /// - `target`: Validated logical destination path.
    /// - `options`: Copy options resolved by the facade.
    /// - `symlink_policy`: Policy applied when traversing symbolic links.
    ///
    /// # Returns
    /// A ready operation that retains the resolved copy request.
    #[must_use]
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
            deadline: CopyDeadline::new(options.deadline(), std::time::Instant::now),
            options: ResolvedCopyOptions::new(options, symlink_policy),
            state: AsyncCopyOperationState::Ready,
            writer: None,
            recovery: CopyRecoverySnapshot::unchanged(),
        }
    }

    /// Returns the immutable source path.
    ///
    /// # Returns
    /// The source path captured when the operation was created.
    #[inline]
    #[must_use]
    pub const fn source(&self) -> &Path {
        &self.source
    }

    /// Returns the immutable destination path.
    ///
    /// # Returns
    /// The destination path captured when the operation was created.
    #[inline]
    #[must_use]
    pub const fn target(&self) -> &Path {
        &self.target
    }

    /// Returns the current operation lifecycle state.
    ///
    /// # Returns
    /// The state most recently recorded by this operation.
    #[inline]
    #[must_use]
    pub const fn state(&self) -> AsyncCopyOperationState {
        self.state
    }

    /// Returns whether a recovery writer is retained by this operation.
    ///
    /// # Returns
    /// `true` when a recovery writer is retained, or `false` otherwise.
    #[inline]
    #[must_use]
    pub const fn has_recovery(&self) -> bool {
        self.writer.is_some()
    }

    /// Borrows the retained recovery writer, if one exists.
    ///
    /// # Returns
    /// `Some` with the retained writer, or `None` when no writer is retained.
    #[inline]
    pub fn recovery(&mut self) -> Option<&mut AsyncWriterRecovery> {
        self.writer.as_mut()
    }

    /// Takes ownership of the retained recovery writer, if one exists.
    ///
    /// # Returns
    /// `Some` with the removed writer, or `None` when no writer was retained.
    #[inline]
    pub fn take_recovery(&mut self) -> Option<AsyncWriterRecovery> {
        self.writer.take()
    }

    /// Executes the operation exactly once.
    ///
    /// The operation becomes running only when this future is first polled.
    /// Dropping a polled pending future records indeterminate state and does
    /// not invoke provider cleanup or start any additional I/O. The deadline
    /// includes time spent waiting before execution and is cooperative: a
    /// pending provider future is not forcibly woken by this operation, and
    /// external cancellation does not roll back provider publication.
    ///
    /// The returned [`AsyncCopyFailure`] owns the error, publication state, and
    /// copy statistics. This operation retains any recovery writer; inspect it
    /// with [`Self::recovery`] or transfer it with [`Self::take_recovery`].
    /// Keep the operation outside the cancellation scope and cancel only
    /// this `execute` future so those facts and recovery ownership remain
    /// available.
    ///
    /// # Returns
    /// The provider-confirmed copy outcome.
    ///
    /// # Errors
    /// Returns [`AsyncCopyFailure`] with the confirmed copy state and any
    /// retained writer when preflight, provider execution, fallback, or
    /// outcome validation fails.
    pub async fn execute(&mut self) -> Result<CopyOutcome, AsyncCopyFailure> {
        if self.state != AsyncCopyOperationState::Ready {
            return Err(invalid_state_failure(
                &self.source,
                &self.target,
                self.file_system.properties().info().provider_id(),
                self.recovery,
            ));
        }
        let Self {
            file_system,
            source,
            target,
            options,
            state,
            writer,
            deadline,
            recovery,
        } = self;
        let mut guard = CopyCancellationGuard::start(state, writer, recovery);
        let result = execute_copy(file_system, source, target, options, *deadline, guard.writer_mut()).await;
        guard.finish(&result);
        result
    }
}

/// Dispatches the asynchronous provider attempt and falls back only when the
/// provider explicitly declines it.
///
/// # Parameters
/// - `filesystem`: Facade used for provider execution and fallback.
/// - `source`: Validated source path.
/// - `target`: Destination path associated with the copy.
/// - `options`: Copy policy resolved by the facade.
/// - `deadline`: Caller budget shared by provider and fallback work.
/// - `writer`: Retained recovery writer, if fallback has opened one.
///
/// # Returns
/// The completed copy outcome when the provider or fallback succeeds.
///
/// # Errors
/// The contextual failure when limits, provider execution, or outcome
/// validation fails.
async fn execute_copy(
    filesystem: &AsyncFileSystem,
    source: &Path,
    target: &Path,
    options: &ResolvedCopyOptions,
    deadline: CopyDeadline,
    writer: &mut Option<AsyncWriterRecovery>,
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
    if deadline.expired() {
        return Err(filesystem.contextual_copy_failure(
            budget_error(source, target, "copy deadline was exceeded"),
            CopyFailureState::Unchanged,
            CopyStats::default(),
            source,
            target,
        ));
    }
    if !filesystem.core().provider_supports(ProviderOperation::TryCopy) {
        return stream_copy_fallback(filesystem, source, target, options, deadline, writer).await;
    }
    match filesystem
        .spi()
        .try_copy(CopyRequest::new(source, target, options.clone()))
        .await
    {
        Ok(CopyAttempt::Completed(outcome)) => {
            let outcome = filesystem.verify_completed_copy(outcome, options.options(), source, target)?;
            if deadline.expired() {
                return Err(filesystem.contextual_copy_failure(
                    budget_error(source, target, "copy deadline was exceeded"),
                    from_completed_stats(outcome.stats()),
                    *outcome.stats(),
                    source,
                    target,
                ));
            }
            Ok(outcome)
        }
        Ok(CopyAttempt::Declined(_)) => {
            stream_copy_fallback(filesystem, source, target, options, deadline, writer).await
        }
        Err(failure) => {
            let (error, state, stats) = failure.into_parts();
            Err(filesystem.contextual_copy_failure(error, state, stats, source, target))
        }
    }
}

/// Streams a declined asynchronous copy while retaining any recovery writer
/// in `writer_slot` until publication or cleanup completes.
///
/// # Parameters
/// - `filesystem`: Facade used to read and write the copied resource.
/// - `source`: Validated source path.
/// - `target`: Validated destination path.
/// - `options`: Copy policy resolved by the facade.
/// - `deadline`: Caller budget checked throughout the transfer.
/// - `writer_slot`: Slot that retains the writer while recovery is possible.
///
/// # Returns
/// A future that resolves to the streamed copy outcome on success.
///
/// # Errors
/// The contextual failure when preflight, streaming, publication, or cleanup
/// fails.
#[inline]
fn stream_copy_fallback<'a>(
    filesystem: &'a AsyncFileSystem,
    source: &'a Path,
    target: &'a Path,
    options: &'a ResolvedCopyOptions,
    deadline: CopyDeadline,
    writer_slot: &'a mut Option<AsyncWriterRecovery>,
) -> SpiFuture<'a, Result<CopyOutcome, AsyncCopyFailure>> {
    Box::pin(async move {
        let options = options.options();
        let plan = StreamCopyPlan::new(options, filesystem.properties().limits(), source, target);
        if let Err(error) = plan.validate_options(filesystem.properties().symlink_policy()) {
            return Err(filesystem.contextual_copy_failure(
                error,
                CopyFailureState::Unchanged,
                CopyStats::default(),
                source,
                target,
            ));
        }
        if deadline.expired() {
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
        if deadline.expired() {
            return Err(filesystem.contextual_copy_failure(
                budget_error(source, target, "copy deadline was exceeded"),
                CopyFailureState::Unchanged,
                CopyStats::default(),
                source,
                target,
            ));
        }
        if let Err(error) = plan.validate_metadata(&metadata) {
            return Err(filesystem.contextual_copy_failure(
                error,
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
        if deadline.expired() {
            return Err(filesystem.contextual_copy_failure(
                budget_error(source, target, "copy deadline was exceeded"),
                CopyFailureState::Unchanged,
                CopyStats::default(),
                source,
                target,
            ));
        }
        let writer_options = plan.writer_options();
        match filesystem.open_writer(target, writer_options).await {
            Ok(writer) => *writer_slot = Some(AsyncWriterRecovery::Opened(Box::new(writer))),
            Err(error)
                if error.stage() == OpenFailureStage::ProviderOpen
                    && error.recovery().is_none()
                    && error.error().kind() == FsErrorKind::AlreadyExists
                    && is_unchanged_open_failure(error.error())
                    && options.conflict() == CopyConflictPolicy::Skip =>
            {
                let stats = CopyStats {
                    skipped: 1,
                    ..CopyStats::default()
                };
                if deadline.expired() {
                    return Err(filesystem.contextual_copy_failure(
                        budget_error(source, target, "copy deadline was exceeded"),
                        CopyFailureState::Unchanged,
                        stats,
                        source,
                        target,
                    ));
                }
                return Ok(CopyOutcome::streamed_fallback(
                    stats,
                    crate::metadata::AchievedAtomicity::NonAtomic,
                    false,
                ));
            }
            Err(error) => {
                let (error, stage, recovery) = error.into_parts();
                *writer_slot = recovery.map(AsyncWriterRecovery::Rejected);
                let state = match stage {
                    OpenFailureStage::Preflight => CopyFailureState::Unchanged,
                    OpenFailureStage::ProviderOpen => from_write_failure_state(open_failure_state(&error)),
                    OpenFailureStage::OutcomeValidation => CopyFailureState::Indeterminate,
                };
                return Err(filesystem.contextual_copy_failure(error, state, CopyStats::default(), source, target));
            }
        }
        if deadline.expired() {
            let writer = writer_slot
                .as_ref()
                .and_then(AsyncWriterRecovery::opened)
                .expect("writer is retained before transfer");
            return Err(filesystem.contextual_copy_failure(
                budget_error(source, target, "copy deadline was exceeded"),
                from_writer_state(writer.state()),
                fallback_failure_stats(writer.written_bytes()),
                source,
                target,
            ));
        }
        let mut bytes = 0_u64;
        let mut buffer = [0_u8; 8192];
        loop {
            if deadline.expired() {
                let writer = writer_slot
                    .as_ref()
                    .and_then(AsyncWriterRecovery::opened)
                    .expect("writer is retained before transfer");
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
                            .and_then(AsyncWriterRecovery::opened)
                            .expect("writer is retained before transfer")
                            .state(),
                    ),
                    fallback_failure_stats(
                        writer_slot
                            .as_ref()
                            .and_then(AsyncWriterRecovery::opened)
                            .expect("writer is retained before transfer")
                            .written_bytes(),
                    ),
                    source,
                    target,
                )
            })?;
            if deadline.expired() {
                let writer = writer_slot
                    .as_ref()
                    .and_then(AsyncWriterRecovery::opened)
                    .expect("writer is retained before transfer");
                return Err(filesystem.contextual_copy_failure(
                    budget_error(source, target, "copy deadline was exceeded"),
                    from_writer_state(writer.state()),
                    fallback_failure_stats(writer.written_bytes()),
                    source,
                    target,
                ));
            }
            if read == 0 {
                break;
            }
            let writer = writer_slot
                .as_mut()
                .and_then(AsyncWriterRecovery::opened_mut)
                .expect("writer is retained before transfer");
            let next_bytes = plan.next_bytes(bytes, read).map_err(|error| {
                filesystem.contextual_copy_failure(
                    error,
                    from_writer_state(writer.state()),
                    fallback_failure_stats(writer.written_bytes()),
                    source,
                    target,
                )
            })?;
            writer.write_fully_async(&buffer[..read]).await.map_err(|error| {
                filesystem.contextual_copy_failure(
                    FsError::from_stream_io(error, FsOperation::Write, target),
                    from_writer_state(writer.state()),
                    fallback_failure_stats(writer.written_bytes()),
                    source,
                    target,
                )
            })?;
            if deadline.expired() {
                return Err(filesystem.contextual_copy_failure(
                    budget_error(source, target, "copy deadline was exceeded"),
                    from_writer_state(writer.state()),
                    fallback_failure_stats(writer.written_bytes()),
                    source,
                    target,
                ));
            }
            bytes = next_bytes;
        }
        let writer = writer_slot
            .as_mut()
            .and_then(AsyncWriterRecovery::opened_mut)
            .expect("writer is retained before flush");
        if deadline.expired() {
            return Err(filesystem.contextual_copy_failure(
                budget_error(source, target, "copy deadline was exceeded"),
                from_writer_state(writer.state()),
                fallback_failure_stats(writer.written_bytes()),
                source,
                target,
            ));
        }
        writer.flush_async().await.map_err(|error| {
            filesystem.contextual_copy_failure(
                FsError::from_stream_io(error, FsOperation::Write, target),
                from_writer_state(writer.state()),
                fallback_failure_stats(writer.written_bytes()),
                source,
                target,
            )
        })?;
        if deadline.expired() {
            return Err(filesystem.contextual_copy_failure(
                budget_error(source, target, "copy deadline was exceeded"),
                from_writer_state(writer.state()),
                fallback_failure_stats(writer.written_bytes()),
                source,
                target,
            ));
        }
        let writer = writer_slot
            .as_mut()
            .and_then(AsyncWriterRecovery::opened_mut)
            .expect("writer is retained before commit");
        let write_outcome = match writer.commit_async().await {
            Ok(outcome) => outcome,
            Err(failure)
                if failure.error().kind() == FsErrorKind::AlreadyExists
                    && plan.may_skip_conflict(from_writer_state(writer.state())) =>
            {
                let error = failure.into_error();
                if let Err(cleanup_error) = writer.abort_async().await {
                    return Err(filesystem.contextual_copy_failure(
                        cleanup_error,
                        from_writer_state(writer.state()),
                        fallback_failure_stats(writer.written_bytes()),
                        source,
                        target,
                    ));
                }
                let state = from_writer_state(writer.state());
                let _ = writer_slot.take();
                let stats = CopyStats {
                    skipped: 1,
                    ..CopyStats::default()
                };
                if deadline.expired() {
                    return Err(filesystem.contextual_copy_failure(
                        budget_error(source, target, "copy deadline was exceeded"),
                        state,
                        stats,
                        source,
                        target,
                    ));
                }
                if state != CopyFailureState::Unchanged {
                    return Err(filesystem.contextual_copy_failure(error, state, stats, source, target));
                }
                return Ok(CopyOutcome::streamed_fallback(
                    stats,
                    crate::metadata::AchievedAtomicity::NonAtomic,
                    false,
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
        if deadline.expired() {
            let _ = writer_slot.take();
            return Err(filesystem.contextual_copy_failure(
                budget_error(source, target, "copy deadline was exceeded"),
                CopyFailureState::Published,
                StreamCopyPlan::completed_stats(metadata.kind(), bytes),
                source,
                target,
            ));
        }
        let _ = writer_slot.take();
        Ok(CopyOutcome::streamed_fallback(
            StreamCopyPlan::completed_stats(metadata.kind(), bytes),
            write_outcome.atomicity(),
            write_outcome.durable(),
        ))
    })
}

/// Builds a caller-budget error for an asynchronous copy.
///
/// # Parameters
/// - `source`: Source path associated with the copy.
/// - `target`: Destination path associated with the copy.
/// - `message`: Human-readable reason the caller budget was exceeded.
///
/// # Returns
/// A resource-limit error associated with the source and destination paths.
fn budget_error(source: &Path, target: &Path, message: &str) -> FsError {
    FsError::new(FsErrorKind::ResourceLimitExceeded, FsOperation::Copy, message)
        .with_path(source.clone())
        .with_target(target.clone())
}

/// Builds the stable failure used for an invalid execute retry.
///
/// # Parameters
/// - `source`: Source path captured by the operation.
/// - `target`: Destination path captured by the operation.
/// - `provider`: Provider identifier associated with the operation.
/// - `snapshot`: Last confirmed publication state and transfer statistics.
///
/// # Returns
/// An invalid-state failure containing the saved publication state and stats.
fn invalid_state_failure(
    source: &Path,
    target: &Path,
    provider: &str,
    snapshot: CopyRecoverySnapshot,
) -> AsyncCopyFailure {
    AsyncCopyFailure::new(
        FsError::new(
            FsErrorKind::InvalidState,
            FsOperation::Copy,
            "copy operation cannot execute in its current state",
        )
        .with_path(source.clone())
        .with_target(target.clone())
        .with_provider(provider),
        snapshot.state,
        snapshot.stats,
    )
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::io::Result as IoResult;
    use std::pin::Pin;
    use std::task::Context;
    use std::task::Poll;
    use std::task::Waker;
    use std::time::Duration;
    use std::time::Instant;

    use qubit_io::AsyncInput;
    use qubit_io::AsyncOutput;

    use super::AsyncCopyOperation;
    use super::CopyDeadline;
    use crate::copy::AsyncCopyOperationState;
    use crate::copy::CopyConflictPolicy;
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
    use crate::spi::AsyncFileSystemSpi;
    use crate::spi::AsyncFileWriteSession;
    use crate::spi::OpenReaderRequest;
    use crate::spi::OpenWriterRequest;
    use crate::spi::OpenedAsyncReader;
    use crate::spi::OpenedAsyncWriter;
    use crate::spi::ProviderOperation;
    use crate::spi::ProviderOperations;
    use crate::spi::ProviderProperties;
    use crate::spi::SpiFuture;
    use crate::spi::StatRequest;
    use crate::spi::StatResponse;
    use crate::write::WriteAbortOutcome;
    use crate::write::WriteFailure;
    use crate::write::WriteFailureState;

    thread_local! { static NOW: Cell<Option<Instant>> = const { Cell::new(None) }; }
    fn test_now() -> Instant {
        NOW.with(|now| now.get().expect("test clock initialized"))
    }
    fn advance_clock() {
        NOW.with(|now| {
            now.set(Some(
                now.get().expect("test clock initialized") + Duration::from_secs(1),
            ))
        });
    }

    #[derive(Clone, Copy)]
    enum SuspendAt {
        Flush,
        Commit,
    }
    struct StageSpi {
        suspend_at: SuspendAt,
        fail_commit: bool,
        skip_open: bool,
        skip_commit: bool,
        abort_outcome: WriteAbortOutcome,
    }
    impl StageSpi {
        fn info(&self, path: &Path) -> OpenedFileInfo {
            OpenedFileInfo::new(
                FileSystemId::new("async-deadline-test").expect("valid id"),
                path.clone(),
            )
            .with_metadata(FileMetadata::new(FileKind::File).with_len(Some(1)))
        }
    }
    impl AsyncFileSystemSpi for StageSpi {
        fn properties(&self) -> ProviderProperties {
            ProviderProperties::new(
                FileSystemInfo::new(
                    FileSystemId::new("async-deadline-test").expect("valid id"),
                    "async-deadline-test",
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
        fn stat<'a>(&'a self, request: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
            Box::pin(async move {
                Ok(StatResponse::new(
                    request.path().clone(),
                    FileMetadata::new(FileKind::File).with_len(Some(1)),
                ))
            })
        }
        fn open_reader<'a>(&'a self, request: OpenReaderRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncReader>> {
            let info = self.info(request.path());
            Box::pin(async move { Ok(OpenedAsyncReader::new(info, Box::new(StageReader { read: false }))) })
        }
        fn open_writer<'a>(&'a self, request: OpenWriterRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncWriter>> {
            let info = self.info(request.path());
            let suspend_at = self.suspend_at;
            let fail_commit = self.fail_commit;
            let skip_open = self.skip_open;
            let skip_commit = self.skip_commit;
            let abort_outcome = self.abort_outcome;
            if skip_open {
                let mut yielded = false;
                return Box::pin(std::future::poll_fn(move |_| {
                    if !yielded {
                        yielded = true;
                        return Poll::Pending;
                    }
                    Poll::Ready(Err(FsError::new(
                        FsErrorKind::AlreadyExists,
                        FsOperation::OpenWriter,
                        "skip conflict",
                    )
                    .with_effect_state(crate::error::FsEffectState::Unchanged)))
                }));
            }
            Box::pin(async move {
                Ok(OpenedAsyncWriter::new(
                    info,
                    Box::new(StageWriter {
                        suspend_at,
                        fail_commit,
                        skip_commit,
                        abort_outcome,
                        flush_pending: false,
                    }),
                ))
            })
        }
    }

    struct StageReader {
        read: bool,
    }
    impl AsyncInput for StageReader {
        type Item = u8;
        unsafe fn poll_read_unchecked(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            output: &mut [u8],
            index: usize,
            count: usize,
        ) -> Poll<IoResult<usize>> {
            let this = self.as_mut().get_mut();
            if this.read || count == 0 {
                return Poll::Ready(Ok(0));
            }
            output[index] = b'x';
            this.read = true;
            Poll::Ready(Ok(1))
        }
    }
    struct StageWriter {
        suspend_at: SuspendAt,
        fail_commit: bool,
        skip_commit: bool,
        abort_outcome: WriteAbortOutcome,
        flush_pending: bool,
    }
    impl AsyncOutput for StageWriter {
        type Item = u8;
        unsafe fn poll_write_unchecked(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            _: &[u8],
            _: usize,
            count: usize,
        ) -> Poll<IoResult<usize>> {
            Poll::Ready(Ok(count))
        }
        fn poll_flush(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<IoResult<()>> {
            let this = self.as_mut().get_mut();
            if matches!(this.suspend_at, SuspendAt::Flush) && !this.flush_pending {
                this.flush_pending = true;
                Poll::Pending
            } else {
                Poll::Ready(Ok(()))
            }
        }
    }
    impl AsyncFileWriteSession for StageWriter {
        fn commit_async<'a>(mut self: Pin<&'a mut Self>) -> SpiFuture<'a, Result<WriteOutcome, WriteFailure>> {
            let this = self.as_mut().get_mut();
            let fail = this.fail_commit;
            let skip = this.skip_commit;
            let suspend = matches!(this.suspend_at, SuspendAt::Commit) || skip;
            let mut yielded = false;
            Box::pin(std::future::poll_fn(move |_| {
                if suspend && !yielded {
                    yielded = true;
                    return Poll::Pending;
                }
                advance_clock();
                Poll::Ready(if skip {
                    Err(WriteFailure::new(
                        FsError::new(FsErrorKind::AlreadyExists, FsOperation::CommitWriter, "skip conflict"),
                        WriteFailureState::RetryableNotPublished,
                    ))
                } else if fail {
                    Err(WriteFailure::new(
                        FsError::new(FsErrorKind::Io, FsOperation::CommitWriter, "late async commit failure"),
                        WriteFailureState::RetryableNotPublished,
                    ))
                } else {
                    Ok(WriteOutcome::new(
                        AchievedAtomicity::NonAtomic,
                        PublicationMethod::Direct,
                    ))
                })
            }))
        }
        fn abort_async<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, FsResult<WriteAbortOutcome>> {
            let outcome = self.get_mut().abort_outcome;
            Box::pin(async move {
                advance_clock();
                Ok(outcome)
            })
        }
    }

    fn operation(suspend_at: SuspendAt, fail_commit: bool) -> AsyncCopyOperation {
        let file_system = crate::AsyncFileSystem::from_spi(StageSpi {
            suspend_at,
            fail_commit,
            skip_open: false,
            skip_commit: false,
            abort_outcome: WriteAbortOutcome::NotPublished,
        })
        .expect("valid async filesystem");
        let start = Instant::now();
        NOW.with(|now| now.set(Some(start)));
        let mut operation = AsyncCopyOperation::new(
            file_system,
            Path::parse("/source").expect("source"),
            Path::parse("/target").expect("target"),
            CopyOptions::file().with_deadline(Some(Duration::from_secs(1))),
            SymlinkPolicy::Reject,
        );
        operation.deadline = CopyDeadline::new(Some(Duration::from_secs(1)), test_now);
        operation
    }

    fn skip_operation(skip_open: bool, abort_outcome: WriteAbortOutcome, with_deadline: bool) -> AsyncCopyOperation {
        let file_system = crate::AsyncFileSystem::from_spi(StageSpi {
            suspend_at: SuspendAt::Commit,
            fail_commit: false,
            skip_open,
            skip_commit: !skip_open,
            abort_outcome,
        })
        .expect("valid async filesystem");
        let start = Instant::now();
        NOW.with(|now| now.set(Some(start)));
        let mut options = CopyOptions::default().with_conflict(CopyConflictPolicy::Skip);
        if with_deadline {
            options = options.with_deadline(Some(Duration::from_secs(1)));
        }
        let mut operation = AsyncCopyOperation::new(
            file_system,
            Path::parse("/source").expect("source"),
            Path::parse("/target").expect("target"),
            options,
            SymlinkPolicy::Reject,
        );
        if with_deadline {
            operation.deadline = CopyDeadline::new(Some(Duration::from_secs(1)), test_now);
        }
        operation
    }

    #[test]
    fn test_async_copy_deadline_after_flush_preserves_unpublished_recovery() {
        let mut operation = operation(SuspendAt::Flush, false);
        let mut future = Box::pin(operation.execute());
        let mut context = Context::from_waker(Waker::noop());
        assert!(
            future.as_mut().poll(&mut context).is_pending(),
            "flush must suspend on first poll"
        );
        advance_clock();
        let failure = match future.as_mut().poll(&mut context) {
            Poll::Ready(Err(failure)) => failure,
            _ => panic!("expired flush must fail"),
        };
        drop(future);
        assert_eq!(CopyFailureState::Unchanged, failure.state());
        assert_eq!(1, failure.partial_stats().bytes);
        assert!(operation.has_recovery());
        assert_eq!(
            AsyncCopyOperationState::Failed(CopyFailureState::Unchanged),
            operation.state()
        );
    }

    #[test]
    fn test_async_copy_deadline_after_commit_is_published_without_recovery() {
        let mut operation = operation(SuspendAt::Commit, false);
        let mut future = Box::pin(operation.execute());
        let mut context = Context::from_waker(Waker::noop());
        assert!(
            future.as_mut().poll(&mut context).is_pending(),
            "commit must suspend on first poll"
        );
        advance_clock();
        let failure = match future.as_mut().poll(&mut context) {
            Poll::Ready(Err(failure)) => failure,
            _ => panic!("expired commit must fail"),
        };
        drop(future);
        assert_eq!(CopyFailureState::Published, failure.state());
        assert_eq!(1, failure.partial_stats().bytes);
        assert!(!operation.has_recovery());
    }

    #[test]
    fn test_async_copy_deadline_preserves_late_provider_commit_error() {
        let mut operation = operation(SuspendAt::Commit, true);
        let mut future = Box::pin(operation.execute());
        let mut context = Context::from_waker(Waker::noop());
        assert!(future.as_mut().poll(&mut context).is_pending());
        advance_clock();
        let failure = match future.as_mut().poll(&mut context) {
            Poll::Ready(Err(failure)) => failure,
            _ => panic!("provider error must win"),
        };
        drop(future);
        assert_eq!(FsErrorKind::Io, failure.error().kind());
        assert_eq!(CopyFailureState::Unchanged, failure.state());
    }

    #[test]
    fn test_async_copy_deadline_after_open_skip_preserves_skipped_stats() {
        let mut operation = skip_operation(true, WriteAbortOutcome::NotPublished, true);
        let mut future = Box::pin(operation.execute());
        let mut context = Context::from_waker(Waker::noop());
        assert!(
            future.as_mut().poll(&mut context).is_pending(),
            "provider open must suspend first"
        );
        advance_clock();
        let failure = match future.as_mut().poll(&mut context) {
            Poll::Ready(Err(failure)) => failure,
            _ => panic!("expired skipped open must fail"),
        };
        drop(future);
        assert_eq!(CopyFailureState::Unchanged, failure.state());
        assert_eq!(1, failure.partial_stats().skipped);
        assert!(!operation.has_recovery());
    }

    #[test]
    fn test_async_copy_deadline_after_skip_abort_keeps_abort_publication_state() {
        for abort_outcome in [WriteAbortOutcome::Published, WriteAbortOutcome::Indeterminate] {
            let mut operation = skip_operation(false, abort_outcome, true);
            let mut future = Box::pin(operation.execute());
            let mut context = Context::from_waker(Waker::noop());
            assert!(
                future.as_mut().poll(&mut context).is_pending(),
                "commit must suspend first"
            );
            advance_clock();
            let failure = match future.as_mut().poll(&mut context) {
                Poll::Ready(Err(failure)) => failure,
                _ => panic!("expired skipped commit must fail"),
            };
            drop(future);
            assert_eq!(
                if abort_outcome == WriteAbortOutcome::Published {
                    CopyFailureState::Published
                } else {
                    CopyFailureState::Indeterminate
                },
                failure.state(),
            );
            assert_eq!(1, failure.partial_stats().skipped);
            assert!(!operation.has_recovery());
        }
    }

    #[test]
    fn test_async_copy_skip_without_deadline_remains_successful() {
        let mut operation = skip_operation(true, WriteAbortOutcome::NotPublished, false);
        let mut future = Box::pin(operation.execute());
        let mut context = Context::from_waker(Waker::noop());
        assert!(
            future.as_mut().poll(&mut context).is_pending(),
            "provider open must suspend first"
        );
        let outcome = match future.as_mut().poll(&mut context) {
            Poll::Ready(Ok(outcome)) => outcome,
            _ => panic!("unbounded skip should succeed"),
        };
        drop(future);
        assert_eq!(1, outcome.stats().skipped);
        assert_eq!(AsyncCopyOperationState::Completed, operation.state());
    }
}
