//! Owning asynchronous whole-file write operation.
use qubit_io::AsyncOutput;

use crate::AsyncFileSystem;
use crate::error::FsError;
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
/// Owning asynchronous whole-file write that survives cancellation.
pub struct AsyncWriteAllOperation<'a> {
    filesystem: &'a AsyncFileSystem,
    path: Path,
    bytes: &'a [u8],
    options: WriteOptions,
    state: AsyncWriteAllOperationState,
    writer: Option<AsyncFileWriter>,
    recovery: WriteAllRecoverySnapshot,
}
impl<'a> AsyncWriteAllOperation<'a> {
    /// Creates an operation after facade preflight has succeeded.
    pub(crate) fn new(filesystem: &'a AsyncFileSystem, path: Path, bytes: &'a [u8], options: WriteOptions) -> Self {
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
    /// Returns the operation state.
    #[must_use]
    pub const fn state(&self) -> AsyncWriteAllOperationState {
        self.state
    }
    /// Reports whether an opened writer is retained for recovery.
    #[must_use]
    pub const fn has_recovery_writer(&self) -> bool {
        self.writer.is_some()
    }
    /// Returns mutable access to the retained recovery writer.
    #[must_use]
    pub fn recovery_writer(&mut self) -> Option<&mut AsyncFileWriter> {
        self.writer.as_mut()
    }
    /// Takes ownership of the retained recovery writer.
    #[must_use]
    pub fn take_recovery_writer(&mut self) -> Option<AsyncFileWriter> {
        self.writer.take()
    }
    /// Returns bytes accepted before the operation stopped.
    #[must_use]
    pub const fn written_bytes(&self) -> u64 {
        self.recovery.written_bytes
    }
    /// Executes the operation once, retaining all recovery state on failure.
    pub async fn execute(&mut self) -> Result<WriteOutcome, AsyncWriteAllOperationFailure> {
        if self.state != AsyncWriteAllOperationState::Ready {
            return Err(AsyncWriteAllOperationFailure::new(
                invalid_state(&self.path, self.filesystem),
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
        result
    }
}
async fn execute_write(
    filesystem: &AsyncFileSystem,
    path: &Path,
    bytes: &[u8],
    options: &WriteOptions,
    slot: &mut Option<AsyncFileWriter>,
) -> Result<WriteOutcome, AsyncWriteAllOperationFailure> {
    if slot.is_none() {
        *slot = Some(
            filesystem
                .open_writer(path, options.clone())
                .await
                .map_err(|error| AsyncWriteAllOperationFailure::new(error, WriteFailureState::NotPublished, 0))?,
        );
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
fn contextual(filesystem: &AsyncFileSystem, error: std::io::Error, path: &Path) -> FsError {
    filesystem.core().enrich(
        FsError::from_stream_io(error, FsOperation::Write, path),
        Some(path),
        FsOperation::Write,
    )
}
fn state_for(indeterminate: bool, state: WriterState) -> WriteFailureState {
    if indeterminate {
        WriteFailureState::Indeterminate
    } else {
        state.publication_failure_state()
    }
}
fn invalid_state(path: &Path, filesystem: &AsyncFileSystem) -> FsError {
    FsError::new(
        crate::error::FsErrorKind::InvalidState,
        FsOperation::Write,
        "async whole-file write cannot execute in its current state",
    )
    .with_path(path.clone())
    .with_provider(filesystem.properties().info().provider_id())
}
