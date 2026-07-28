// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Concrete synchronous file writer handle.

use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Result as IoResult};

use qubit_io::Output;

use crate::spi::FileWriterSpi;
use crate::{
    AchievedAtomicity, AtomicityRequirement, FsError, FsErrorKind, FsOperation, FsResult,
    OpenedFileInfo, WriteFailure, WriteFailureState, WriteOutcome, WriterState,
};

/// Type-erased provider write session explicitly associated with a file.
pub struct FileWriter {
    session: Box<dyn FileWriterSpi>,
    info: OpenedFileInfo,
    state: WriterState,
    atomicity: AtomicityRequirement,
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
    pub(crate) fn new(
        info: OpenedFileInfo,
        session: Box<dyn FileWriterSpi>,
        atomicity: AtomicityRequirement,
    ) -> Self {
        Self {
            session,
            info,
            state: WriterState::Open,
            atomicity,
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
    pub fn commit(&mut self) -> Result<WriteOutcome, WriteFailure> {
        if self.state != WriterState::Open {
            return Err(WriteFailure::new(
                self.invalid_state(
                    FsOperation::CommitWriter,
                    "writer cannot be committed in its current state",
                ),
                WriteFailureState::NotPublished,
            ));
        }
        let outcome = self.session.commit();
        match outcome {
            Ok(outcome) => {
                self.state = if outcome.atomicity == AchievedAtomicity::Atomic {
                    WriterState::Committed
                } else {
                    WriterState::Published
                };
                if self.atomicity == AtomicityRequirement::Required
                    && outcome.atomicity != AchievedAtomicity::Atomic
                {
                    return Err(WriteFailure::new(
                        FsError::new(
                            FsErrorKind::ProviderContractViolation,
                            FsOperation::CommitWriter,
                            "provider reported non-atomic success for an atomic-required write",
                        )
                        .with_path(self.info.path().clone()),
                        WriteFailureState::Published,
                    ));
                }
                Ok(outcome)
            }
            Err(failure) => {
                self.state = match failure.state() {
                    WriteFailureState::RetryableNotPublished => WriterState::Open,
                    WriteFailureState::NotPublished => WriterState::NotPublished,
                    WriteFailureState::Published => WriterState::Published,
                    WriteFailureState::Indeterminate => WriterState::Indeterminate,
                };
                let (error, state) = failure.into_parts();
                Err(WriteFailure::new(error, state))
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
                if self.state != WriterState::Published {
                    self.state = WriterState::Aborted;
                }
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

    /// Builds a stable invalid-state error for this writer.
    fn invalid_state(&self, operation: FsOperation, message: &str) -> FsError {
        FsError::new(FsErrorKind::InvalidState, operation, message)
            .with_path(self.info.path().clone())
    }

    /// Builds a stream error for byte transfer after lifecycle completion.
    fn closed_io_error(&self) -> IoError {
        IoError::new(
            IoErrorKind::BrokenPipe,
            self.invalid_state(FsOperation::Write, "writer no longer accepts bytes"),
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
        match unsafe { self.session.write_unchecked(input, index, count) } {
            Ok(value) => Ok(value),
            Err(error) => {
                self.state = WriterState::Indeterminate;
                Err(error)
            }
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        if self.state != WriterState::Open {
            return Err(self.closed_io_error());
        }
        match self.session.flush() {
            Ok(()) => Ok(()),
            Err(error) => {
                self.state = WriterState::Indeterminate;
                Err(error)
            }
        }
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
            WriterState::Open | WriterState::NotPublished | WriterState::Published
        ) {
            let _ = self.session.abort();
        }
    }
}
