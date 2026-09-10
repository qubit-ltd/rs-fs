// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Cancellation guard for async whole-file writes.
use crate::metadata::WriteOutcome;
use crate::write::AsyncWriteAllOperationFailure;
use crate::write::AsyncWriteAllOperationState;
use crate::write::AsyncWriterRecovery;
use crate::write::WriteFailureState;
use crate::write::internal::WriteAllRecoverySnapshot;
/// Keeps operation facts outside the borrowed execution future.
pub(crate) struct WriteAllCancellationGuard<'a> {
    /// Owning operation state updated on completion or cancellation.
    state: &'a mut AsyncWriteAllOperationState,
    /// Recovery slot that survives cancellation of the execution future.
    writer: &'a mut Option<AsyncWriterRecovery>,
    /// Historical publication and acknowledged-progress snapshot.
    recovery: &'a mut WriteAllRecoverySnapshot,
    /// Whether an explicit result has already been recorded.
    finished: bool,
}
impl<'a> WriteAllCancellationGuard<'a> {
    /// Arms cancellation tracking before the first provider await.
    pub(crate) fn start(
        state: &'a mut AsyncWriteAllOperationState,
        writer: &'a mut Option<AsyncWriterRecovery>,
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
    /// Borrows the retained writer slot for provider execution.
    #[inline]
    pub(crate) fn writer_mut(&mut self) -> &mut Option<AsyncWriterRecovery> {
        self.writer
    }
    /// Records a terminal result and releases a successfully committed writer.
    ///
    /// # Panics
    /// Panics if successful execution violated the internal retained-writer
    /// invariant.
    pub(crate) fn finish(&mut self, result: &Result<WriteOutcome, AsyncWriteAllOperationFailure>) {
        *self.state = match result {
            Ok(_) => {
                self.recovery.written_bytes = self
                    .writer
                    .as_ref()
                    .and_then(AsyncWriterRecovery::opened)
                    .expect("successful write retains writer")
                    .written_bytes();
                self.recovery.state = WriteFailureState::Published;
                self.writer.take();
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
    /// Records cancellation without performing provider I/O.
    fn drop(&mut self) {
        if !self.finished && *self.state == AsyncWriteAllOperationState::Running {
            *self.state = AsyncWriteAllOperationState::Failed(WriteFailureState::Indeterminate);
            if let Some(writer) = self.writer.as_mut().and_then(AsyncWriterRecovery::opened_mut) {
                self.recovery.written_bytes = writer.written_bytes();
                writer.mark_indeterminate();
            }
            self.recovery.state = WriteFailureState::Indeterminate;
        }
    }
}
