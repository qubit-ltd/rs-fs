// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Concrete synchronous file writer handle.

use std::fmt::{
    Debug,
    Formatter,
    Result as FmtResult,
};
use std::io::{
    Error as IoError,
    ErrorKind as IoErrorKind,
    Result as IoResult,
};

use qubit_io::Output;

use crate::{
    FileLocation,
    FileWriteSession,
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
    OpenedFileInfo,
    WriteFailureState,
    WriteOutcome,
    WriterState,
};

/// Type-erased provider write session explicitly associated with a file.
pub struct FileWriter {
    session: Box<dyn FileWriteSession>,
    info: OpenedFileInfo,
    state: WriterState,
}

impl FileWriter {
    /// Wraps an already-open provider write session.
    ///
    /// # Parameters
    /// - `session`: Provider session accepting file bytes.
    /// - `info`: File identity and optional open-time metadata snapshot.
    ///
    /// # Returns
    /// A concrete writer in [`WriterState::Open`].
    #[inline]
    #[must_use]
    pub fn new<S>(session: S, info: OpenedFileInfo) -> Self
    where
        S: FileWriteSession + 'static,
    {
        Self {
            session: Box::new(session),
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

    /// Publishes bytes accepted by this session.
    ///
    /// This method borrows rather than consumes the writer. The provider's
    /// typed failure determines whether retry remains safe or only explicit
    /// cleanup is available.
    ///
    /// # Returns
    /// Actual publication method and atomicity on success.
    ///
    /// # Errors
    /// Returns [`FsErrorKind::InvalidState`] after commit or abort, or the
    /// provider publication error.
    pub fn commit(&mut self) -> FsResult<WriteOutcome> {
        if self.state != WriterState::Open {
            return Err(self.invalid_state(
                FsOperation::CommitWriter,
                "writer cannot be committed in its current state",
            ));
        }
        let outcome = self.session.commit();
        match outcome {
            Ok(outcome) => {
                self.state = WriterState::Committed;
                Ok(outcome)
            }
            Err(failure) => {
                self.state = match failure.state() {
                    WriteFailureState::Retryable => WriterState::Open,
                    WriteFailureState::NotPublished => {
                        WriterState::NotPublished
                    }
                    WriteFailureState::Published => WriterState::Published,
                    WriteFailureState::Indeterminate => {
                        WriterState::Indeterminate
                    }
                };
                Err(failure.into_error())
            }
        }
    }

    /// Aborts this writer and releases provider staging resources.
    ///
    /// Abort is allowed while open and after every failed commit. Successful
    /// cleanup does not imply that an already-published target was rolled back.
    /// An indeterminate abort disables automatic drop cleanup so the caller can
    /// inspect provider state before choosing a recovery action.
    ///
    /// # Errors
    /// Returns [`FsErrorKind::InvalidState`] after commit or a previous abort,
    /// or returns the provider cleanup error while retaining the session.
    pub fn abort(&mut self) -> FsResult<()> {
        if !matches!(
            self.state,
            WriterState::Open
                | WriterState::NotPublished
                | WriterState::Published
                | WriterState::Indeterminate
        ) {
            return Err(self.invalid_state(
                FsOperation::AbortWriter,
                "writer cannot be aborted in its current state",
            ));
        }
        match self.session.abort() {
            Ok(()) => {
                self.state = WriterState::Aborted;
                Ok(())
            }
            Err(error) => {
                if error.kind() == FsErrorKind::Indeterminate {
                    self.state = WriterState::Indeterminate;
                }
                Err(error)
            }
        }
    }

    /// Rebinds the handle to registry-resolved canonical identity.
    #[inline(always)]
    pub(crate) fn bind_location(&mut self, location: FileLocation) {
        self.info.replace_location(location);
    }

    /// Builds a stable invalid-state error for this writer.
    fn invalid_state(&self, operation: FsOperation, message: &str) -> FsError {
        FsError::new(FsErrorKind::InvalidState, operation, message)
            .with_path(self.info.location().path().clone())
    }

    /// Builds a stream error for byte transfer after lifecycle completion.
    fn closed_io_error(&self) -> IoError {
        IoError::new(
            IoErrorKind::BrokenPipe,
            self.invalid_state(
                FsOperation::Write,
                "writer no longer accepts bytes",
            ),
        )
    }
}

impl Output for FileWriter {
    type Item = u8;

    #[inline(always)]
    fn is_buffered(&self) -> bool {
        self.session.is_buffered()
    }

    unsafe fn write_unchecked(
        &mut self,
        input: &[u8],
        index: usize,
        count: usize,
    ) -> IoResult<usize> {
        if self.state != WriterState::Open {
            return Err(self.closed_io_error());
        }
        // SAFETY: The caller guarantees the same range contract required by
        // the wrapped output session.
        unsafe { self.session.write_unchecked(input, index, count) }
    }

    fn flush(&mut self) -> IoResult<()> {
        if self.state != WriterState::Open {
            return Err(self.closed_io_error());
        }
        self.session.flush()
    }
}

impl Debug for FileWriter {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("FileWriter")
            .field("info", &self.info)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl Drop for FileWriter {
    fn drop(&mut self) {
        if matches!(
            self.state,
            WriterState::Open
                | WriterState::NotPublished
                | WriterState::Published
        ) {
            let _ = self.session.abort();
        }
    }
}
