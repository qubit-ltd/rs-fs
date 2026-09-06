//! Cancellation guard for async whole-file writes.
use crate::write::AsyncFileWriter;
use crate::write::AsyncWriteAllOperationFailure;
use crate::write::AsyncWriteAllOperationState;
use crate::write::WriteFailureState;
use crate::write::internal::WriteAllRecoverySnapshot;
pub(crate) struct WriteAllCancellationGuard<'a> {
    state: &'a mut AsyncWriteAllOperationState,
    writer: &'a mut Option<AsyncFileWriter>,
    recovery: &'a mut WriteAllRecoverySnapshot,
    finished: bool,
}
impl<'a> WriteAllCancellationGuard<'a> {
    pub(crate) fn start(
        state: &'a mut AsyncWriteAllOperationState,
        writer: &'a mut Option<AsyncFileWriter>,
        recovery: &'a mut WriteAllRecoverySnapshot,
    ) -> Self {
        *state = AsyncWriteAllOperationState::Running;
        Self {
            state,
            writer,
            recovery,
            finished: false,
        }
    }
    pub(crate) fn writer_mut(&mut self) -> &mut Option<AsyncFileWriter> {
        self.writer
    }
    pub(crate) fn finish(&mut self, result: &Result<crate::metadata::WriteOutcome, AsyncWriteAllOperationFailure>) {
        *self.state = match result {
            Ok(_) => {
                self.recovery.state = WriteFailureState::Published;
                AsyncWriteAllOperationState::Completed
            }
            Err(failure) => {
                self.recovery.state = failure.state();
                self.recovery.written_bytes = failure.written_bytes();
                AsyncWriteAllOperationState::Failed(failure.state())
            }
        };
        self.finished = true;
    }
}
impl Drop for WriteAllCancellationGuard<'_> {
    fn drop(&mut self) {
        if !self.finished && *self.state == AsyncWriteAllOperationState::Running {
            *self.state = AsyncWriteAllOperationState::Failed(WriteFailureState::Indeterminate);
            if let Some(writer) = self.writer.as_mut() {
                self.recovery.written_bytes = writer.written_bytes();
                writer.mark_indeterminate();
            }
            self.recovery.state = WriteFailureState::Indeterminate;
        }
    }
}
