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
use crate::error::OpenFailureStage;
use crate::metadata::WriteOutcome;
use crate::path::Path;
use crate::write::AsyncWriteAllOperationFailure;
use crate::write::AsyncWriteAllOperationState;
use crate::write::AsyncWriterRecovery;
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
/// assert!(!operation.has_recovery());
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
    writer: Option<AsyncWriterRecovery>,
    /// Frozen publication and confirmed-progress snapshot.
    recovery: WriteAllRecoverySnapshot,
}
impl AsyncWriteAllOperation {
    /// Creates an operation after facade preflight has succeeded.
    ///
    /// # Parameters
    /// - `filesystem`: Filesystem whose provider stages execute this write.
    /// - `path`: Target path used for error context and provider requests.
    /// - `bytes`: Payload retained for the whole-file write.
    /// - `options`: Write policy applied to the provider request.
    ///
    /// # Returns
    /// A ready operation that owns the payload until completion or
    /// cancellation.
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
    ///
    /// # Returns
    /// The filesystem used for provider calls and error context.
    #[inline]
    #[must_use]
    pub const fn filesystem(&self) -> &AsyncFileSystem {
        &self.filesystem
    }
    /// Returns the requested target path.
    ///
    /// # Returns
    /// The path retained for execution and recovery context.
    #[inline]
    #[must_use]
    pub const fn path(&self) -> &Path {
        &self.path
    }
    /// Returns the operation state.
    ///
    /// # Returns
    /// The current historical execution state.
    #[inline]
    pub const fn state(&self) -> AsyncWriteAllOperationState {
        self.state
    }
    /// Reports whether an opened writer is retained for recovery.
    ///
    /// # Returns
    /// `true` if this operation retains a writer for recovery, otherwise
    /// `false`.
    #[inline]
    #[must_use]
    pub const fn has_recovery(&self) -> bool {
        self.writer.is_some()
    }
    /// Returns mutable access to the retained recovery writer.
    ///
    /// # Returns
    /// `Some` with mutable access when recovery is retained, otherwise `None`.
    #[inline]
    pub fn recovery(&mut self) -> Option<&mut AsyncWriterRecovery> {
        self.writer.as_mut()
    }
    /// Transfers recovery responsibility to the caller.
    ///
    /// Subsequent writer recovery does not change this operation's historical
    /// publication state or byte count.
    ///
    /// # Returns
    /// The retained recovery writer, or `None` when recovery is not required.
    #[inline]
    pub fn take_recovery(&mut self) -> Option<AsyncWriterRecovery> {
        self.writer.take()
    }
    /// Returns the frozen count of bytes confirmed before execution stopped.
    ///
    /// # Returns
    /// The number of bytes the writer confirmed accepting before execution
    /// ended.
    #[inline]
    #[must_use]
    pub const fn written_bytes(&self) -> u64 {
        self.recovery.written_bytes
    }
    /// Returns the input suffix after the bytes confirmed accepted by the
    /// writer.
    ///
    /// The suffix remains owned by this operation when its execution future is
    /// cancelled. If publication is indeterminate, this is an inspection aid;
    /// it does not establish that retrying the suffix is safe.
    ///
    /// # Returns
    /// The unconfirmed payload suffix, which may be empty after full
    /// acceptance.
    #[inline]
    #[must_use]
    pub fn remaining_bytes(&self) -> &[u8] {
        let offset = usize::try_from(self.recovery.written_bytes)
            .unwrap_or(self.bytes.len())
            .min(self.bytes.len());
        &self.bytes[offset..]
    }
    /// Executes the operation once, retaining recovery state on failure.
    ///
    /// # Cancellation
    ///
    /// Keep this operation outside the cancellation scope and cancel only this
    /// future. Dropping an unpolled future leaves the operation ready. After
    /// execution starts, cancellation records indeterminate publication and
    /// retains any opened writer. A missing writer does not prove no effects.
    /// The payload remains owned by the operation if this future is cancelled.
    /// [`Self::remaining_bytes`] identifies the suffix after confirmed writes;
    /// uncertain publication still requires provider-specific recovery before
    /// any retry.
    ///
    /// # Returns
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
        let mut guard = WriteAllCancellationGuard::start(state, writer, recovery);
        let result = execute_write(filesystem, path, bytes, options, guard.writer_mut()).await;
        guard.finish(&result);
        if result.is_ok() {
            bytes.clear();
        }
        result
    }
}
/// Runs provider stages while keeping the session in the recovery slot.
///
/// # Parameters
/// - `filesystem`: Filesystem whose provider stages execute this write.
/// - `path`: Target path used for error context and provider requests.
/// - `bytes`: Payload retained for the whole-file write.
/// - `options`: Write policy applied to the provider request.
/// - `slot`: Recovery slot retaining the opened or rejected writer.
///
/// # Returns
/// The provider-confirmed outcome or a failure retaining confirmed progress
/// and any recovery writer.
///
/// # Errors
/// Returns open, write, flush, or commit failures with their publication facts.
///
/// # Panics
/// Panics if a successful open does not leave an opened writer in the recovery
/// slot, violating the helper's internal slot invariant.
async fn execute_write(
    filesystem: &AsyncFileSystem,
    path: &Path,
    bytes: &[u8],
    options: &WriteOptions,
    slot: &mut Option<AsyncWriterRecovery>,
) -> Result<WriteOutcome, AsyncWriteAllOperationFailure> {
    if slot.is_none() {
        match filesystem.open_writer(path, options.clone()).await {
            Ok(writer) => *slot = Some(AsyncWriterRecovery::Opened(Box::new(writer))),
            Err(failure) => {
                let (error, stage, recovery) = failure.into_parts();
                *slot = recovery.map(AsyncWriterRecovery::Rejected);
                let state = match stage {
                    OpenFailureStage::Preflight => WriteFailureState::NotPublished,
                    OpenFailureStage::ProviderOpen => open_failure_state(&error),
                    OpenFailureStage::OutcomeValidation => WriteFailureState::Indeterminate,
                };
                return Err(AsyncWriteAllOperationFailure::new(error, state, 0));
            }
        }
    }
    let writer = slot
        .as_mut()
        .and_then(AsyncWriterRecovery::opened_mut)
        .expect("writer is retained after open");
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
///
/// # Parameters
/// - `filesystem`: Filesystem whose provider stages execute this write.
/// - `error`: Failure whose trusted filesystem context is being restored.
/// - `path`: Target path used for error context and provider requests.
///
/// # Returns
/// The filesystem error with provider and target context restored.
fn contextual(filesystem: &AsyncFileSystem, error: IoError, path: &Path) -> FsError {
    filesystem.core().enrich(
        FsError::from_stream_io(error, FsOperation::Write, path),
        Some(path),
        FsOperation::Write,
    )
}
/// Classifies a post-open stream failure using writer publication facts.
///
/// # Parameters
/// - `indeterminate`: Whether a completed stage left publication uncertain.
/// - `state`: Publication state used to classify or retain this outcome.
///
/// # Returns
/// `Indeterminate` when effects are uncertain; otherwise the writer state
/// mapped to its corresponding write failure state.
fn state_for(indeterminate: bool, state: WriterState) -> WriteFailureState {
    if indeterminate {
        WriteFailureState::Indeterminate
    } else {
        state.publication_failure_state()
    }
}
/// Builds a repeated-execution error without changing historical facts.
///
/// # Parameters
/// - `path`: Target path used for error context and provider requests.
/// - `filesystem`: Filesystem whose provider stages execute this write.
///
/// # Returns
/// An invalid-state error enriched with the target path and provider identity.
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
    ///
    /// # Parameters
    /// - `formatter`: Formatter receiving the diagnostic representation.
    ///
    /// # Returns
    /// The formatter's result after writing the lifecycle facts.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("AsyncWriteAllOperation")
            .field("path", &self.path)
            .field("state", &self.state)
            .field("written_bytes", &self.recovery.written_bytes)
            .field("has_recovery", &self.writer.is_some())
            .finish()
    }
}
