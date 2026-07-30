// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Cancellation guard for an owning asynchronous copy operation.

use crate::{
    AsyncCopyFailure,
    AsyncCopyOperationState,
    AsyncFileWriter,
    CopyFailureState,
    CopyOutcome,
};

/// Marks a polled operation indeterminate if cancellation interrupts provider
/// I/O.
pub(in crate::copy) struct CopyCancellationGuard<'a> {
    state: &'a mut AsyncCopyOperationState,
    writer: &'a mut Option<AsyncFileWriter>,
    finished: bool,
}

impl<'a> CopyCancellationGuard<'a> {
    /// Starts tracking cancellation immediately before the first provider
    /// await.
    #[inline]
    pub(in crate::copy) fn start(
        state: &'a mut AsyncCopyOperationState,
        writer: &'a mut Option<AsyncFileWriter>,
    ) -> Self {
        *state = AsyncCopyOperationState::Running;
        Self {
            state,
            writer,
            finished: false,
        }
    }

    /// Borrows the recovery writer slot for the running operation.
    #[inline(always)]
    pub(in crate::copy) fn writer_mut(
        &mut self,
    ) -> &mut Option<AsyncFileWriter> {
        self.writer
    }

    /// Records the completed result and disables cancellation handling.
    #[inline]
    pub(in crate::copy) fn finish(
        &mut self,
        result: &Result<CopyOutcome, AsyncCopyFailure>,
    ) {
        *self.state = match result {
            Ok(_) => AsyncCopyOperationState::Completed,
            Err(failure) => AsyncCopyOperationState::Failed(failure.state()),
        };
        self.finished = true;
    }
}

impl Drop for CopyCancellationGuard<'_> {
    /// Records only local state; drop never calls a provider.
    fn drop(&mut self) {
        if !self.finished && *self.state == AsyncCopyOperationState::Running {
            *self.state = AsyncCopyOperationState::Failed(
                CopyFailureState::Indeterminate,
            );
            if let Some(writer) = self.writer.as_mut() {
                writer.mark_indeterminate();
            }
        }
    }
}
