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

use crate::copy::AsyncCopyFailure;
use crate::copy::AsyncCopyOperationState;
use crate::copy::CopyFailureState;
use crate::copy::CopyOutcome;
use crate::copy::CopyStats;
use crate::copy::internal::CopyRecoverySnapshot;
use crate::write::AsyncFileWriter;

/// Marks a polled operation indeterminate if cancellation interrupts provider
/// I/O.
pub(in crate::copy) struct CopyCancellationGuard<'a> {
    /// Borrowed operation state updated when execution finishes or is dropped.
    state: &'a mut AsyncCopyOperationState,
    /// Borrowed slot retaining an opened destination writer.
    writer: &'a mut Option<Box<AsyncFileWriter>>,
    recovery: &'a mut CopyRecoverySnapshot,
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
        recovery: &'a mut CopyRecoverySnapshot,
    ) -> Self {
        *state = AsyncCopyOperationState::Running;
        Self {
            state,
            writer,
            recovery,
            finished: false,
        }
    }

    /// Borrows the recovery writer slot for the running operation.
    ///
    /// # Returns
    /// The mutable slot used to retain an opened recovery writer.
    #[inline(always)]
    pub(in crate::copy) fn writer_mut(&mut self) -> &mut Option<Box<AsyncFileWriter>> {
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
    pub(in crate::copy) fn finish(&mut self, result: &Result<CopyOutcome, AsyncCopyFailure>) {
        *self.state = match result {
            Ok(outcome) => {
                self.recovery.state = CopyFailureState::Published;
                self.recovery.stats = *outcome.stats();
                AsyncCopyOperationState::Completed
            }
            Err(failure) => {
                self.recovery.state = failure.state();
                self.recovery.stats = *failure.partial_stats();
                AsyncCopyOperationState::Failed(failure.state())
            }
        };
        self.finished = true;
    }
}

impl Drop for CopyCancellationGuard<'_> {
    /// Records only local state; drop never calls a provider.
    fn drop(&mut self) {
        if !self.finished && *self.state == AsyncCopyOperationState::Running {
            *self.state = AsyncCopyOperationState::Failed(CopyFailureState::Indeterminate);
            self.recovery.state = CopyFailureState::Indeterminate;
            self.recovery.stats = self.writer.as_ref().map_or(CopyStats::default(), |writer| {
                crate::copy::internal::fallback_failure_stats(writer.written_bytes())
            });
            if let Some(writer) = self.writer.as_mut() {
                writer.mark_indeterminate();
            }
        }
    }
}
