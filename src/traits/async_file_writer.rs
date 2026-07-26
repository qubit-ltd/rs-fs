// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Concrete asynchronous file writer handle.

use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Result as IoResult};
use std::pin::Pin;
use std::task::{Context, Poll};

use qubit_io::AsyncOutput;

use crate::{
    AsyncFileWriteSession, FileLocation, FsError, FsErrorKind, FsFuture, FsOperation,
    OpenedFileInfo, WriteOutcome, WriterState,
};

/// Type-erased asynchronous provider write session associated with a file.
pub struct AsyncFileWriter {
    session: Pin<Box<dyn AsyncFileWriteSession>>,
    info: OpenedFileInfo,
    state: WriterState,
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
    pub fn new<S>(session: S, info: OpenedFileInfo) -> Self
    where
        S: AsyncFileWriteSession + 'static,
    {
        Self {
            session: Box::pin(session),
            info,
            state: WriterState::Open,
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
    pub fn commit_async(&mut self) -> FsFuture<'_, WriteOutcome> {
        if self.state != WriterState::Open {
            let error = self.invalid_state(
                FsOperation::CommitWriter,
                "writer cannot be committed in its current state",
            );
            return Box::pin(async move { Err(error) });
        }
        Box::pin(async move {
            self.state = WriterState::Indeterminate;
            let result = self.session.as_mut().commit_async().await;
            match result {
                Ok(outcome) => {
                    self.state = WriterState::Committed;
                    Ok(outcome)
                }
                Err(error) => {
                    if error.kind() != FsErrorKind::Indeterminate {
                        self.state = WriterState::Open;
                    }
                    Err(error)
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
    /// A future resolving when cleanup is confirmed.
    pub fn abort_async(&mut self) -> FsFuture<'_, ()> {
        if !matches!(self.state, WriterState::Open | WriterState::Indeterminate) {
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
                Ok(()) => {
                    self.state = WriterState::Aborted;
                    Ok(())
                }
                Err(error) => {
                    if error.kind() != FsErrorKind::Indeterminate {
                        self.state = previous_state;
                    }
                    Err(error)
                }
            }
        })
    }

    /// Rebinds the handle to registry-resolved canonical identity.
    #[inline(always)]
    pub(crate) fn bind_location(&mut self, location: FileLocation) {
        self.info.replace_location(location);
    }

    /// Builds a stable invalid-state error.
    fn invalid_state(&self, operation: FsOperation, message: &str) -> FsError {
        FsError::new(FsErrorKind::InvalidState, operation, message)
            .with_path(self.info.location().path().clone())
    }

    /// Builds a byte-transfer error after lifecycle completion.
    fn closed_io_error(&self) -> IoError {
        IoError::new(
            IoErrorKind::BrokenPipe,
            self.invalid_state(FsOperation::Write, "writer no longer accepts bytes"),
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
        // SAFETY: The caller guarantees the same range contract required by
        // the wrapped asynchronous output session.
        unsafe {
            this.session
                .as_mut()
                .poll_write_unchecked(cx, input, index, count)
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        let this = self.get_mut();
        if this.state != WriterState::Open {
            return Poll::Ready(Err(this.closed_io_error()));
        }
        this.session.as_mut().poll_flush(cx)
    }
}

impl Debug for AsyncFileWriter {
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
        if self.state == WriterState::Open {
            self.session.as_mut().cancel_on_drop();
        }
    }
}
