// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Owning asynchronous whole-file write operation.
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::io::Error as IoError;

use qubit_io::AsyncOutput;

use crate::AsyncFileSystem;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::metadata::WriteOutcome;
use crate::path::Path;
use crate::write::AsyncFileWriter;
use crate::write::AsyncWriteAllOperationFailure;
use crate::write::AsyncWriteAllOperationState;
use crate::write::WriteFailureState;
use crate::write::WriteOptions;
use crate::write::WriterState;
use crate::write::internal::WriteAllCancellationGuard;
use crate::write::internal::WriteAllRecoverySnapshot;
use crate::write::internal::open_failure_state;
/// Owning asynchronous whole-file write that survives cancellation.
///
/// # Examples
///
/// The example runs against an isolated in-memory fixture. Applications obtain
/// their configured facade from a provider or registry integration.
///
/// ```rust
/// # mod support { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/common/rustdoc_support.rs")); }
/// # use support::*;
/// # let (filesystem, _) = async_recording_spi::async_recording_file_system(Default::default());
/// # poll_support::ready(async {
/// use qubit_fs::Path;
/// use qubit_fs::write::WriteOptions;
/// use qubit_fs::write::AsyncWriteAllOperationState;
///
/// let mut operation = filesystem.begin_write_all(
///     Path::parse("/report")?, b"bytes".to_vec(), WriteOptions::default(),
/// )?;
/// operation.execute().await?;
/// assert_eq!(AsyncWriteAllOperationState::Completed, operation.state());
/// assert_eq!(5, operation.written_bytes());
/// assert!(!operation.has_recovery_writer());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// # }).unwrap();
/// ```
#[must_use]
pub struct AsyncWriteAllOperation {
    /// Filesystem retained independently of the caller.
    filesystem: AsyncFileSystem,
    /// Target retained for recovery inspection.
    path: Path,
    /// Request payload, held only until execution ends or is cancelled.
    bytes: Vec<u8>,
    /// Validated request options.
    options: WriteOptions,
    /// Historical execution state.
    state: AsyncWriteAllOperationState,
    /// Opened writer retained while recovery may be required.
    writer: Option<AsyncFileWriter>,
    /// Frozen publication and confirmed-progress snapshot.
    recovery: WriteAllRecoverySnapshot,
}
impl AsyncWriteAllOperation {
    /// Creates an operation after facade preflight has succeeded.
    pub(crate) fn new(filesystem: AsyncFileSystem, path: Path, bytes: Vec<u8>, options: WriteOptions) -> Self {
        Self {
            filesystem,
            path,
            bytes,
            options,
            state: AsyncWriteAllOperationState::Ready,
            writer: None,
            recovery: WriteAllRecoverySnapshot::new(),
        }
    }
    /// Returns the filesystem retained for execution and recovery inspection.
    #[inline]
    #[must_use]
    pub const fn filesystem(&self) -> &AsyncFileSystem {
        &self.filesystem
    }
    /// Returns the requested target path.
    #[inline]
    #[must_use]
    pub const fn path(&self) -> &Path {
        &self.path
    }
    /// Returns the operation state.
    #[inline]
    #[must_use = "inspect publication state before choosing a recovery action"]
    pub const fn state(&self) -> AsyncWriteAllOperationState {
        self.state
    }
    /// Reports whether an opened writer is retained for recovery.
    #[inline]
    #[must_use]
    pub const fn has_recovery_writer(&self) -> bool {
        self.writer.is_some()
    }
    /// Returns mutable access to the retained recovery writer.
    #[inline]
    #[must_use]
    pub fn recovery_writer(&mut self) -> Option<&mut AsyncFileWriter> {
        self.writer.as_mut()
    }
    /// Transfers recovery responsibility to the caller.
    ///
    /// Subsequent writer recovery does not change this operation's historical
    /// publication state or byte count.
    #[inline]
    #[must_use]
    pub fn take_recovery_writer(&mut self) -> Option<AsyncFileWriter> {
        self.writer.take()
    }
    /// Returns the frozen count of bytes confirmed before execution stopped.
    #[inline]
    #[must_use]
    pub const fn written_bytes(&self) -> u64 {
        self.recovery.written_bytes
    }
    /// Executes the operation once, retaining recovery state on failure.
    ///
    /// # Cancellation
    ///
    /// Keep this operation outside the cancellation scope and cancel only this
    /// future. Dropping an unpolled future leaves the operation ready. After
    /// execution starts, cancellation records indeterminate publication and
    /// retains any opened writer. A missing writer does not prove no effects.
    /// The payload is released when this future finishes or is dropped.
    ///
    /// # Returns
    ///
    /// The confirmed write outcome. Success freezes the acknowledged byte count
    /// and releases the committed writer.
    ///
    /// # Errors
    ///
    /// Returns the provider failure with confirmed progress, or `InvalidState`
    /// if execution has already started. Repeated execution preserves the
    /// historical state and does not invoke the provider again.
    pub async fn execute(&mut self) -> Result<WriteOutcome, AsyncWriteAllOperationFailure> {
        if self.state != AsyncWriteAllOperationState::Ready {
            return Err(AsyncWriteAllOperationFailure::new(
                invalid_state(&self.path, &self.filesystem),
                self.recovery.state,
                self.recovery.written_bytes,
            ));
        }
        let Self {
            filesystem,
            path,
            bytes,
            options,
            state,
            writer,
            recovery,
        } = self;
        let bytes = std::mem::take(bytes);
        let mut guard = WriteAllCancellationGuard::start(state, writer, recovery);
        let result = execute_write(filesystem, path, &bytes, options, guard.writer_mut()).await;
        guard.finish(&result);
        result
    }
}
/// Runs provider stages while keeping the session in the recovery slot.
async fn execute_write(
    filesystem: &AsyncFileSystem,
    path: &Path,
    bytes: &[u8],
    options: &WriteOptions,
    slot: &mut Option<AsyncFileWriter>,
) -> Result<WriteOutcome, AsyncWriteAllOperationFailure> {
    if slot.is_none() {
        *slot = Some(filesystem.open_writer(path, options.clone()).await.map_err(|error| {
            let state = open_failure_state(&error);
            AsyncWriteAllOperationFailure::new(error, state, 0)
        })?);
    }
    let writer = slot.as_mut().expect("writer is retained after open");
    if let Err(error) = writer.write_fully_async(bytes).await {
        let error = contextual(filesystem, error, path);
        let state = state_for(error.has_indeterminate_effect(), writer.state());
        return Err(AsyncWriteAllOperationFailure::new(error, state, writer.written_bytes()));
    }
    if let Err(error) = writer.flush_async().await {
        let error = contextual(filesystem, error, path);
        let state = state_for(error.has_indeterminate_effect(), writer.state());
        return Err(AsyncWriteAllOperationFailure::new(error, state, writer.written_bytes()));
    }
    match writer.commit_async().await {
        Ok(outcome) => Ok(outcome),
        Err(failure) => {
            let (error, state) = failure.into_parts();
            Err(AsyncWriteAllOperationFailure::new(error, state, writer.written_bytes()))
        }
    }
}
/// Restores filesystem and target context to a stream failure.
fn contextual(filesystem: &AsyncFileSystem, error: IoError, path: &Path) -> FsError {
    filesystem.core().enrich(
        FsError::from_stream_io(error, FsOperation::Write, path),
        Some(path),
        FsOperation::Write,
    )
}
/// Classifies a post-open stream failure using writer publication facts.
fn state_for(indeterminate: bool, state: WriterState) -> WriteFailureState {
    if indeterminate {
        WriteFailureState::Indeterminate
    } else {
        state.publication_failure_state()
    }
}
/// Builds a repeated-execution error without changing historical facts.
fn invalid_state(path: &Path, filesystem: &AsyncFileSystem) -> FsError {
    FsError::new(
        FsErrorKind::InvalidState,
        FsOperation::Write,
        "async whole-file write cannot execute in its current state",
    )
    .with_path(path.clone())
    .with_provider(filesystem.properties().info().provider_id())
}

impl Debug for AsyncWriteAllOperation {
    /// Formats lifecycle facts without payload or provider session contents.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("AsyncWriteAllOperation")
            .field("path", &self.path)
            .field("state", &self.state)
            .field("written_bytes", &self.recovery.written_bytes)
            .field("has_recovery_writer", &self.writer.is_some())
            .finish()
    }
}
