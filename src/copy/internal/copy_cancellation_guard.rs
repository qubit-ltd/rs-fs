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

use crate::AsyncCopyFailure;
use crate::AsyncCopyOperationState;
use crate::AsyncFileWriter;
use crate::CopyFailureState;
use crate::CopyOutcome;

/// Marks a polled operation indeterminate if cancellation interrupts provider
/// I/O.
pub(in crate::copy) struct CopyCancellationGuard<'a> {
    /// Borrowed operation state updated when execution finishes or is dropped.
    state: &'a mut AsyncCopyOperationState,
    /// Borrowed slot retaining an opened destination writer.
    writer: &'a mut Option<Box<AsyncFileWriter>>,
    /// Whether normal completion disarmed cancellation handling.
    finished: bool,
}

impl<'a> CopyCancellationGuard<'a> {
    /// Starts tracking cancellation immediately before the first provider
    /// await.
    ///
    /// # Parameters
    /// - `state`: Operation lifecycle state updated on cancellation.
    /// - `writer`: Recovery-writer slot retained across provider I/O.
    ///
    /// # Returns
    /// An armed cancellation guard borrowing both state locations.
    #[inline]
    pub(in crate::copy) fn start(
        state: &'a mut AsyncCopyOperationState,
        writer: &'a mut Option<Box<AsyncFileWriter>>,
    ) -> Self {
        *state = AsyncCopyOperationState::Running;
        Self {
            state,
            writer,
            finished: false,
        }
    }

    /// Borrows the recovery writer slot for the running operation.
    ///
    /// # Returns
    /// The mutable slot used to retain an opened recovery writer.
    #[inline(always)]
    pub(in crate::copy) fn writer_mut(
        &mut self,
    ) -> &mut Option<Box<AsyncFileWriter>> {
        self.writer
    }

    /// Records the completed result and disables cancellation handling.
    ///
    /// # Parameters
    /// - `result`: Final copy outcome or typed failure.
    ///
    /// # Returns
    /// The unchanged result after updating operation state and recovery facts.
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
