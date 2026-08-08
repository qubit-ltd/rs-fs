// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Concrete synchronous file writer handle.

use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;
use std::io::Result as IoResult;

use qubit_io::Output;

use crate::AchievedAtomicity;
use crate::AtomicityRequirement;
use crate::FsError;
use crate::FsErrorKind;
use crate::FsOperation;
use crate::FsResult;
use crate::OpenedFileInfo;
use crate::WriteAbortOutcome;
use crate::WriteFailure;
use crate::WriteFailureState;
use crate::WriteOutcome;
use crate::WriterState;
use crate::spi::FileWriterSpi;

/// Type-erased provider write session explicitly associated with a file.
pub struct FileWriter {
    /// Provider write session.
    session: Box<dyn FileWriterSpi>,
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
    max_write_bytes: Option<u64>,
    /// Bytes accepted by the provider session so far.
    written_bytes: u64,
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
        provider: &str,
        max_write_bytes: Option<u64>,
    ) -> Self {
        Self {
            session,
            info,
            state: WriterState::Open,
            abort_completed: false,
            atomicity,
            provider: provider.into(),
            max_write_bytes,
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
                        .with_path(self.info.path().clone()),
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
                        .with_path(self.info.path().clone()),
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
    }

    /// Aborts this writer and releases provider staging resources.
    ///
    /// Abort is allowed while open and after every failed commit. Successful
    /// cleanup does not imply that an already-published target was rolled back.
    /// An indeterminate abort disables automatic drop cleanup so the caller can
    /// inspect provider state before choosing a recovery action.
    ///
    /// # Returns
    /// Provider-confirmed destination publication state after cleanup.
    ///
    /// # Errors
    /// Returns [`FsErrorKind::InvalidState`] after commit or a previous abort,
    /// or returns the provider cleanup error while retaining the session.
    pub fn abort(&mut self) -> FsResult<WriteAbortOutcome> {
        if self.abort_completed
            || !matches!(
                self.state,
                WriterState::Open
                    | WriterState::NotPublished
                    | WriterState::Published
                    | WriterState::Indeterminate
            )
        {
            return Err(self.invalid_state(
                FsOperation::AbortWriter,
                "writer cannot be aborted in its current state",
            ));
        }
        match self.session.abort() {
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
                if error.kind() == FsErrorKind::Indeterminate {
                    self.state = WriterState::Indeterminate;
                }
                Err(self.contextual_error(error, FsOperation::AbortWriter))
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
            self.invalid_state(
                FsOperation::Write,
                "writer no longer accepts bytes",
            ),
        )
    }

    /// Builds a typed I/O error when a write would exceed the session limit.
    fn write_limit_error(&self) -> IoError {
        FsError::new(
            FsErrorKind::ResourceLimitExceeded,
            FsOperation::Write,
            "write session exceeds the provider byte limit",
        )
        .with_path(self.info.path().clone())
        .into_io_error()
    }

    /// Returns whether accepting `count` more bytes exceeds the finite limit.
    fn exceeds_write_limit(&self, count: usize) -> bool {
        let Some(maximum) = self.max_write_bytes else {
            return false;
        };
        self.written_bytes.saturating_add(count as u64) > maximum
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
        if self.exceeds_write_limit(count) {
            return Err(self.write_limit_error());
        }
        // SAFETY: The caller guarantees the same range contract required by
        // the wrapped output session.
        match unsafe { self.session.write_unchecked(input, index, count) } {
            Ok(value) => {
                self.written_bytes =
                    self.written_bytes.saturating_add(value as u64);
                Ok(value)
            }
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
    #[inline]
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
        if !self.abort_completed
            && matches!(
                self.state,
                WriterState::Open
                    | WriterState::NotPublished
                    | WriterState::Published
            )
        {
            let _ = self.session.abort();
        }
    }
}
