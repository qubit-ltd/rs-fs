// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Transfer and publication stages for asynchronous stream-copy fallback.

use qubit_io::AsyncInput;
use qubit_io::AsyncOutput;

use crate::AsyncFileSystem;
use crate::copy::AsyncCopyFailure;
use crate::copy::CopyFailureState;
use crate::copy::CopyOutcome;
use crate::copy::CopyStats;
use crate::copy::fallback_failure_stats;
use crate::copy::from_write_failure_state;
use crate::copy::from_writer_state;
use crate::copy::internal::CopyDeadline;
use crate::copy::internal::StreamCopyPlan;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::metadata::FileKind;
use crate::path::Path;
use crate::read::AsyncFileReader;
use crate::write::AsyncFileWriter;
use crate::write::AsyncWriterRecovery;

/// Builds a failure from the currently observed opened-writer state and bytes.
///
/// This only reads the writer; it does not take ownership from the operation's
/// recovery slot or perform cleanup.
///
/// # Parameters
/// - `filesystem`: Filesystem facade used to attach copy context.
/// - `source`: Source path for the copy failure.
/// - `target`: Destination path for the copy failure.
/// - `error`: Primary read, write, or deadline error.
/// - `writer`: Open writer whose state and confirmed bytes are observed.
///
/// # Returns
/// A contextual failure that leaves recovery ownership with the caller.
fn opened_writer_failure(
    filesystem: &AsyncFileSystem,
    source: &Path,
    target: &Path,
    error: FsError,
    writer: &AsyncFileWriter,
) -> AsyncCopyFailure {
    filesystem.contextual_copy_failure(
        error,
        from_writer_state(writer.state()),
        fallback_failure_stats(writer.written_bytes()),
        source,
        target,
    )
}

/// Transfers source bytes into an already-open writer.
///
/// The caller retains ownership of the writer and its recovery slot while this
/// future is pending, so cancellation leaves recovery ownership with the
/// original operation. This function performs reads and writes but does not
/// flush, commit, abort, or remove the writer from its slot.
///
/// # Parameters
/// - `filesystem`: Filesystem facade used to contextualize failures.
/// - `source`: Source path to read.
/// - `target`: Destination path for write errors.
/// - `reader`: Open source reader borrowed for the duration of this future.
/// - `writer`: Open destination writer borrowed for the duration of this
///   future.
/// - `plan`: Validated plan used to account for copied bytes.
/// - `deadline`: Copy deadline checked around provider operations.
///
/// # Returns
/// The number of source bytes accepted by the writer.
///
/// # Errors
/// Returns a contextual copy failure for a read, write, byte-accounting, or
/// deadline error. The failure records the writer state and confirmed bytes;
/// the caller retains the writer for recovery.
pub(in crate::copy) async fn transfer_stream(
    filesystem: &AsyncFileSystem,
    source: &Path,
    target: &Path,
    reader: &mut AsyncFileReader,
    writer: &mut AsyncFileWriter,
    plan: &StreamCopyPlan<'_>,
    deadline: CopyDeadline,
) -> Result<u64, AsyncCopyFailure> {
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 8192];
    let mut writes_since_yield = 0_usize;
    loop {
        if deadline.expired() {
            return Err(opened_writer_failure(
                filesystem,
                source,
                target,
                budget_error(source, target),
                writer,
            ));
        }
        let read = reader.read_async(&mut buffer).await.map_err(|error| {
            opened_writer_failure(
                filesystem,
                source,
                target,
                FsError::from_stream_io(error, FsOperation::Read, source),
                writer,
            )
        })?;
        if deadline.expired() {
            return Err(opened_writer_failure(
                filesystem,
                source,
                target,
                budget_error(source, target),
                writer,
            ));
        }
        if read == 0 {
            break;
        }
        let next_bytes = plan
            .next_bytes(bytes, read)
            .map_err(|error| opened_writer_failure(filesystem, source, target, error, writer))?;
        let mut offset = 0;
        while offset < read {
            if deadline.expired() {
                return Err(opened_writer_failure(
                    filesystem,
                    source,
                    target,
                    budget_error(source, target),
                    writer,
                ));
            }
            let written = match writer.write_async(&buffer[offset..read]).await {
                Ok(written) => written,
                Err(error) => {
                    return Err(opened_writer_failure(
                        filesystem,
                        source,
                        target,
                        FsError::from_stream_io(error, FsOperation::Write, target),
                        writer,
                    ));
                }
            };
            if written == 0 {
                return Err(opened_writer_failure(
                    filesystem,
                    source,
                    target,
                    FsError::from_stream_io(
                        std::io::Error::new(
                            std::io::ErrorKind::WriteZero,
                            "failed to write the complete copy buffer",
                        ),
                        FsOperation::Write,
                        target,
                    ),
                    writer,
                ));
            }
            offset += written;
            if deadline.expired() {
                return Err(opened_writer_failure(
                    filesystem,
                    source,
                    target,
                    budget_error(source, target),
                    writer,
                ));
            }
            writes_since_yield += 1;
            if writes_since_yield == 64 {
                let mut yielded = false;
                std::future::poll_fn(|cx| {
                    if yielded {
                        std::task::Poll::Ready(())
                    } else {
                        yielded = true;
                        cx.waker().wake_by_ref();
                        std::task::Poll::Pending
                    }
                })
                .await;
                writes_since_yield = 0;
            }
        }
        bytes = next_bytes;
    }
    Ok(bytes)
}

/// Flushes and publishes the copied bytes, retaining provider commit facts.
///
/// The writer remains in `writer_slot` while flush and commit futures are
/// pending, so cancellation leaves the slot available to the operation's
/// cancellation guard. The slot is cleared only after successful abort or
/// commit, matching the original fallback behavior.
///
/// # Parameters
/// - `filesystem`: Filesystem facade used to contextualize failures.
/// - `source`: Source path to include in failures and statistics context.
/// - `target`: Destination path to include in failures.
/// - `writer_slot`: Operation-owned recovery slot containing the opened writer.
/// - `plan`: Validated plan used for conflict handling and statistics.
/// - `deadline`: Copy deadline checked around provider operations.
/// - `kind`: Source resource kind used in completed statistics.
/// - `bytes`: Number of source bytes accepted by the writer.
///
/// # Returns
/// The completed or skipped copy outcome.
///
/// # Errors
/// Returns a contextual copy failure if flush, commit, skip cleanup, or the
/// deadline check fails. Commit failures retain the provider's typed write
/// state; failures before commit record the currently observed writer state
/// and confirmed byte count. A successful commit followed by an expired
/// deadline reports the copy as published with completed statistics.
// Keep the private boundary explicit to match the sync helper and design.
#[allow(clippy::too_many_arguments)]
pub(in crate::copy) async fn finish_stream_copy(
    filesystem: &AsyncFileSystem,
    source: &Path,
    target: &Path,
    writer_slot: &mut Option<AsyncWriterRecovery>,
    plan: &StreamCopyPlan<'_>,
    deadline: CopyDeadline,
    kind: &FileKind,
    bytes: u64,
) -> Result<CopyOutcome, AsyncCopyFailure> {
    let writer = writer_slot
        .as_mut()
        .and_then(AsyncWriterRecovery::opened_mut)
        .expect("writer is retained before flush");
    if deadline.expired() {
        return Err(opened_writer_failure(
            filesystem,
            source,
            target,
            budget_error(source, target),
            writer,
        ));
    }
    writer.flush_async().await.map_err(|error| {
        opened_writer_failure(
            filesystem,
            source,
            target,
            FsError::from_stream_io(error, FsOperation::Write, target),
            writer,
        )
    })?;
    if deadline.expired() {
        return Err(opened_writer_failure(
            filesystem,
            source,
            target,
            budget_error(source, target),
            writer,
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
                && plan.may_skip_conflict(from_write_failure_state(failure.state())) =>
        {
            let error = failure.into_error();
            if let Err(cleanup_error) = writer.abort_async().await {
                return Err(opened_writer_failure(filesystem, source, target, cleanup_error, writer));
            }
            let state = from_writer_state(writer.state());
            let _ = writer_slot.take();
            let stats = CopyStats {
                skipped: 1,
                ..CopyStats::default()
            };
            if deadline.expired() {
                return Err(filesystem.contextual_copy_failure(
                    budget_error(source, target),
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
            let (error, state) = failure.into_parts();
            return Err(filesystem.contextual_copy_failure(
                error,
                from_write_failure_state(state),
                fallback_failure_stats(writer.written_bytes()),
                source,
                target,
            ));
        }
    };
    if deadline.expired() {
        let _ = writer_slot.take();
        return Err(filesystem.contextual_copy_failure(
            budget_error(source, target),
            CopyFailureState::Published,
            StreamCopyPlan::completed_stats(kind, bytes),
            source,
            target,
        ));
    }
    let _ = writer_slot.take();
    let mut outcome = CopyOutcome::streamed_fallback(
        StreamCopyPlan::completed_stats(kind, bytes),
        write_outcome.atomicity(),
        write_outcome.durable(),
    );
    if let Some(version) = write_outcome.version() {
        outcome = outcome.with_target_version(version.clone());
    }
    Ok(outcome)
}

/// Creates the contextual deadline error used by stream-copy stages.
///
/// # Parameters
/// - `source`: Source path for the failure.
/// - `target`: Destination path for the failure.
///
/// # Returns
/// A resource-limit copy error carrying both paths.
fn budget_error(source: &Path, target: &Path) -> FsError {
    FsError::new(
        FsErrorKind::ResourceLimitExceeded,
        FsOperation::Copy,
        "copy deadline was exceeded",
    )
    .with_path(source.clone())
    .with_target(target.clone())
}
