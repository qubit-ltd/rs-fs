// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through public
// facade tests.
//! Concrete asynchronous file writer handle.

use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;
use std::io::Result as IoResult;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use qubit_io::AsyncOutput;

use crate::AchievedAtomicity;
use crate::AtomicityRequirement;
use crate::FsError;
use crate::FsErrorKind;
use crate::FsOperation;
use crate::OpenedFileInfo;
use crate::WriteAbortOutcome;
use crate::WriteFailure;
use crate::WriteFailureState;
use crate::WriteOutcome;
use crate::WriterState;
use crate::internal::facade::file_system_resource::ByteBudget;
use crate::internal::facade::file_system_resource::FileSystemResource;
use crate::internal::facade::file_system_resource::budget_error;
use crate::internal::facade::file_system_resource::byte_budget;
use crate::internal::facade::file_system_resource::quantity_from_usize;
use crate::spi::AsyncFileWriteSession;
use crate::spi::SpiFuture;

/// Type-erased asynchronous provider write session associated with a file.
pub struct AsyncFileWriter {
    /// Pinned provider write session.
    session: Pin<Box<dyn AsyncFileWriteSession>>,
    /// Stable identity and metadata captured at open time.
    info: OpenedFileInfo,
    /// Current publication lifecycle state.
    state: WriterState,
    /// Whether explicit provider cleanup has completed.
    abort_completed: bool,
    /// Atomicity required by the caller.
    atomicity: AtomicityRequirement,
    /// Provider identifier attached to facade-generated errors.
    provider: Box<str>,
    /// Optional inclusive byte limit for this write session.
    write_budget: Option<ByteBudget>,
    /// Bytes accepted by the provider session so far.
    written_bytes: u64,
}

impl AsyncFileWriter {
    /// Wraps an already-open asynchronous provider write session.
    ///
    /// # Parameters
    /// - `session`: Runtime-neutral asynchronous write session.
    /// - `info`: File identity and optional open-time metadata snapshot.
    ///
    /// # Returns
    /// A concrete writer in [`WriterState::Open`].
    #[inline]
    #[must_use]
    pub(crate) fn new(
        info: OpenedFileInfo,
        session: Box<dyn AsyncFileWriteSession>,
        atomicity: AtomicityRequirement,
        provider: &str,
        max_write_bytes: Option<u64>,
    ) -> Self {
        Self {
            session: Box::into_pin(session),
            info,
            state: WriterState::Open,
            abort_completed: false,
            atomicity,
            provider: provider.into(),
            write_budget: max_write_bytes.map(|maximum| {
                byte_budget(FileSystemResource::WriteBytes, maximum)
            }),
            written_bytes: 0,
        }
    }

    /// Returns the fixed identity and open-time metadata snapshot.
    ///
    /// # Returns
    /// Information captured when the writer was opened.
    #[inline(always)]
    #[must_use]
    pub fn info(&self) -> &OpenedFileInfo {
        &self.info
    }

    /// Returns the current lifecycle state.
    ///
    /// # Returns
    /// Current writer state.
    #[inline(always)]
    #[must_use]
    pub const fn state(&self) -> WriterState {
        self.state
    }

    /// Returns the bytes accepted by the underlying write session.
    #[inline(always)]
    #[must_use]
    pub(crate) const fn written_bytes(&self) -> u64 {
        self.written_bytes
    }

    /// Records that cancellation interrupted an operation using this writer.
    #[inline(always)]
    pub(crate) fn mark_indeterminate(&mut self) {
        self.state = WriterState::Indeterminate;
    }

    /// Asynchronously publishes bytes accepted by this session.
    ///
    /// A definite failure retains the open session for retry or abort. An
    /// indeterminate failure retains the session and changes its observable
    /// state to [`WriterState::Indeterminate`]. Once the returned future has
    /// been polled, dropping it before completion also makes the writer
    /// indeterminate because publication may already have started. Dropping an
    /// unpolled future leaves the writer open.
    ///
    /// # Returns
    /// A future resolving to the actual publication outcome.
    pub fn commit_async(
        &mut self,
    ) -> SpiFuture<'_, Result<WriteOutcome, WriteFailure>> {
        if self.state != WriterState::Open {
            let error = self.invalid_state(
                FsOperation::CommitWriter,
                "writer cannot be committed in its current state",
            );
            return Box::pin(async move {
                Err(WriteFailure::new(error, WriteFailureState::NotPublished))
            });
        }
        Box::pin(async move {
            self.state = WriterState::Indeterminate;
            let result = self.session.as_mut().commit_async().await;
            match result {
                Ok(outcome) => {
                    self.state = WriterState::Committed;
                    if self.atomicity == AtomicityRequirement::Required
                        && outcome.atomicity() != AchievedAtomicity::Atomic
                    {
                        self.state = WriterState::Published;
                        return Err(WriteFailure::new(
                            FsError::new(
                                FsErrorKind::ProviderContractViolation,
                                FsOperation::CommitWriter,
                                "provider reported non-atomic success for an atomic-required write",
                            )
                            .with_path(self.info.path().clone()).with_provider(&self.provider),
                            WriteFailureState::Published,
                        ));
                    }
                    if let Some(bytes_written) = outcome.bytes_written()
                        && bytes_written != self.written_bytes
                    {
                        self.state = WriterState::Published;
                        return Err(WriteFailure::new(
                            FsError::new(
                                FsErrorKind::ProviderContractViolation,
                                FsOperation::CommitWriter,
                                "provider reported a byte count different from the bytes accepted by the writer",
                            )
                            .with_path(self.info.path().clone()).with_provider(&self.provider),
                            WriteFailureState::Published,
                        ));
                    }
                    Ok(outcome)
                }
                Err(failure) => {
                    self.state = match failure.state() {
                        WriteFailureState::RetryableNotPublished => {
                            WriterState::Open
                        }
                        WriteFailureState::NotPublished => {
                            WriterState::NotPublished
                        }
                        WriteFailureState::Published => WriterState::Published,
                        WriteFailureState::Indeterminate => {
                            WriterState::Indeterminate
                        }
                    };
                    let (error, state) = failure.into_parts();
                    Err(WriteFailure::new(
                        self.contextual_error(error, FsOperation::CommitWriter),
                        state,
                    ))
                }
            }
        })
    }

    /// Asynchronously aborts this session and its provider staging resources.
    ///
    /// Once polled, cancellation before completion leaves the writer
    /// indeterminate. A definite provider failure restores the state from
    /// which abort was started; an indeterminate provider failure does not.
    /// Automatic drop cancellation is disabled for an indeterminate writer.
    ///
    /// # Returns
    /// A future resolving to the provider-confirmed destination publication
    /// state after cleanup.
    pub fn abort_async(
        &mut self,
    ) -> SpiFuture<'_, crate::FsResult<WriteAbortOutcome>> {
        if self.abort_completed
            || !matches!(
                self.state,
                WriterState::Open
                    | WriterState::NotPublished
                    | WriterState::Published
                    | WriterState::Indeterminate
            )
        {
            let error = self.invalid_state(
                FsOperation::AbortWriter,
                "writer cannot be aborted in its current state",
            );
            return Box::pin(async move { Err(error) });
        }
        Box::pin(async move {
            let previous_state = self.state;
            self.state = WriterState::Indeterminate;
            match self.session.as_mut().abort_async().await {
                Ok(outcome) => {
                    self.abort_completed = true;
                    self.state = match outcome {
                        WriteAbortOutcome::NotPublished => WriterState::Aborted,
                        WriteAbortOutcome::Published => WriterState::Published,
                        WriteAbortOutcome::Indeterminate => {
                            WriterState::Indeterminate
                        }
                    };
                    Ok(outcome)
                }
                Err(error) => {
                    if error.kind() != FsErrorKind::Indeterminate {
                        self.state = previous_state;
                    }
                    Err(self.contextual_error(error, FsOperation::AbortWriter))
                }
            }
        })
    }

    /// Builds a stable invalid-state error.
    fn invalid_state(&self, operation: FsOperation, message: &str) -> FsError {
        FsError::new(FsErrorKind::InvalidState, operation, message)
            .with_path(self.info.path().clone())
            .with_provider(&self.provider)
    }

    /// Builds a byte-transfer error after lifecycle completion.
    fn closed_io_error(&self) -> IoError {
        IoError::new(
            IoErrorKind::BrokenPipe,
            self.invalid_state(
                FsOperation::Write,
                "writer no longer accepts bytes",
            ),
        )
    }

    /// Checks whether a provider write can fit in the session budget.
    fn check_write_limit(&self, count: usize) -> IoResult<u64> {
        let count = quantity_from_usize(
            count,
            FsOperation::Write,
            self.info.path(),
            &self.provider,
        )
        .map_err(FsError::into_io_error)?;
        if let Some(budget) = &self.write_budget
            && let Err(error) = budget.check_available(count)
        {
            return Err(budget_error(
                error,
                FsOperation::Write,
                self.info.path(),
                &self.provider,
                "write session exceeds the provider byte limit",
            )
            .into_io_error());
        }
        Ok(count)
    }

    /// Records bytes accepted by the provider in the public `u64` accounting
    /// domain.
    ///
    /// Returns an I/O error when the native byte count or accumulated total
    /// cannot be represented by the filesystem API's `u64` byte counters.
    fn record_written_bytes(&mut self, count: usize) -> IoResult<()> {
        let count = quantity_from_usize(
            count,
            FsOperation::Write,
            self.info.path(),
            &self.provider,
        )
        .map_err(FsError::into_io_error)?;
        if let Some(error) = self
            .write_budget
            .as_mut()
            .and_then(|budget| budget.try_consume(count).err())
        {
            return Err(budget_error(
                error,
                FsOperation::Write,
                self.info.path(),
                &self.provider,
                "write session exceeds the provider byte limit",
            )
            .into_io_error());
        }
        self.written_bytes = self
            .written_bytes
            .checked_add(count)
            .ok_or_else(|| self.byte_count_error())?;
        Ok(())
    }

    /// Builds the error used when native byte accounting exceeds the public
    /// filesystem API's `u64` reporting range.
    fn byte_count_error(&self) -> IoError {
        FsError::new(
            FsErrorKind::ResourceLimitExceeded,
            FsOperation::Write,
            "write byte count exceeds the filesystem API reporting range",
        )
        .with_path(self.info.path().clone())
        .with_provider(&self.provider)
        .into_io_error()
    }

    /// Adds only missing facade context to a provider lifecycle error.
    fn contextual_error(
        &self,
        error: FsError,
        operation: FsOperation,
    ) -> FsError {
        error.with_operation(operation).with_missing_context(
            self.info.path(),
            None,
            &self.provider,
        )
    }
}

impl AsyncOutput for AsyncFileWriter {
    type Item = u8;

    #[inline(always)]
    fn is_buffered(&self) -> bool {
        self.session.is_buffered()
    }

    unsafe fn poll_write_unchecked(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        input: &[u8],
        index: usize,
        count: usize,
    ) -> Poll<IoResult<usize>> {
        let this = self.get_mut();
        if this.state != WriterState::Open {
            return Poll::Ready(Err(this.closed_io_error()));
        }
        if let Err(error) = this.check_write_limit(count) {
            return Poll::Ready(Err(error));
        }
        // SAFETY: The caller guarantees the same range contract required by
        // the wrapped asynchronous output session.
        match unsafe {
            this.session
                .as_mut()
                .poll_write_unchecked(cx, input, index, count)
        } {
            Poll::Ready(Ok(written)) => {
                if let Err(error) = this.record_written_bytes(written) {
                    this.state = WriterState::Indeterminate;
                    return Poll::Ready(Err(error));
                }
                Poll::Ready(Ok(written))
            }
            Poll::Ready(Err(error)) => {
                this.state = WriterState::Indeterminate;
                Poll::Ready(Err(error))
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<IoResult<()>> {
        let this = self.get_mut();
        if this.state != WriterState::Open {
            return Poll::Ready(Err(this.closed_io_error()));
        }
        match this.session.as_mut().poll_flush(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(error)) => {
                this.state = WriterState::Indeterminate;
                Poll::Ready(Err(error))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Debug for AsyncFileWriter {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("AsyncFileWriter")
            .field("info", &self.info)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl Drop for AsyncFileWriter {
    fn drop(&mut self) {
        if !self.abort_completed
            && matches!(
                self.state,
                WriterState::Open
                    | WriterState::NotPublished
                    | WriterState::Published
            )
        {
            self.session.as_mut().cancel_on_drop();
        }
    }
}
