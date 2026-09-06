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

use crate::error::FsEffectState;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::facade::facade_core::ByteBudget;
use crate::facade::facade_core::FacadeCore;
use crate::facade::facade_core::FileSystemResource;
use crate::metadata::AchievedAtomicity;
use crate::metadata::AtomicityRequirement;
use crate::metadata::DurabilityRequirement;
use crate::metadata::OpenedFileInfo;
use crate::metadata::WriteOutcome;
use crate::spi::FileWriterSpi;
use crate::write::WriteAbortOutcome;
use crate::write::WriteFailure;
use crate::write::WriteFailureState;
use crate::write::WriterState;

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
    /// Durability required by the caller.
    durability: DurabilityRequirement,
    /// Provider identifier attached to facade-generated errors.
    provider: Box<str>,
    /// Optional inclusive byte limit for this write session.
    write_budget: Option<ByteBudget>,
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
        durability: DurabilityRequirement,
        provider: &str,
        max_write_bytes: Option<u64>,
    ) -> Self {
        Self {
            session,
            info,
            state: WriterState::Open,
            abort_completed: false,
            atomicity,
            durability,
            provider: provider.into(),
            write_budget: max_write_bytes
                .map(|maximum| FacadeCore::byte_budget(FileSystemResource::WriteBytes, maximum)),
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
            let publication_state = self.state.publication_failure_state();
            return Err(WriteFailure::new(
                self.invalid_state(
                    FsOperation::CommitWriter,
                    "writer cannot be committed in its current state",
                ),
                publication_state,
            ));
        }
        let outcome = self.session.commit();
        match outcome {
            Ok(outcome) => {
                self.state = WriterState::Committed;
                if self.atomicity == AtomicityRequirement::Required && outcome.atomicity() != AchievedAtomicity::Atomic
                {
                    self.state = WriterState::Published;
                    return Err(WriteFailure::new(
                        FsError::new(
                            FsErrorKind::ProviderContractViolation,
                            FsOperation::CommitWriter,
                            "provider reported non-atomic success for an atomic-required write",
                        )
                        .with_path(self.info.path().clone())
                        .with_provider(&self.provider)
                        .with_effect_state(FsEffectState::Applied),
                        WriteFailureState::Published,
                    ));
                }
                if self.durability == DurabilityRequirement::Required && !outcome.durable() {
                    self.state = WriterState::Published;
                    return Err(WriteFailure::new(
                        FsError::new(
                            FsErrorKind::ProviderContractViolation,
                            FsOperation::CommitWriter,
                            "provider reported non-durable success for a durability-required write",
                        )
                        .with_path(self.info.path().clone())
                        .with_provider(&self.provider)
                        .with_effect_state(FsEffectState::Applied),
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
                        .with_path(self.info.path().clone())
                        .with_provider(&self.provider)
                        .with_effect_state(FsEffectState::Applied),
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
                WriterState::Open | WriterState::NotPublished | WriterState::Published | WriterState::Indeterminate
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
                    WriteAbortOutcome::Indeterminate => WriterState::Indeterminate,
                };
                Ok(outcome)
            }
            Err(error) => {
                if error.has_indeterminate_effect() {
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
            .with_provider(&self.provider)
    }

    /// Builds a stream error for byte transfer after lifecycle completion.
    fn closed_io_error(&self) -> IoError {
        IoError::new(
            IoErrorKind::BrokenPipe,
            self.invalid_state(FsOperation::Write, "writer no longer accepts bytes"),
        )
    }

    /// Checks whether a provider write can fit in the session budget.
    fn check_write_limit(&self, count: usize) -> IoResult<u64> {
        let count = FacadeCore::quantity_from_usize(count, FsOperation::Write, self.info.path(), &self.provider)
            .map_err(FsError::into_io_error)?;
        if let Some(budget) = &self.write_budget
            && let Err(error) = budget.check_available(count)
        {
            return Err(FacadeCore::budget_error(
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
        let count = FacadeCore::quantity_from_usize(count, FsOperation::Write, self.info.path(), &self.provider)
            .map_err(FsError::into_io_error)?;
        if let Some(error) = self
            .write_budget
            .as_mut()
            .and_then(|budget| budget.try_consume(count).err())
        {
            return Err(FacadeCore::budget_error(
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
    fn contextual_error(&self, error: FsError, operation: FsOperation) -> FsError {
        error
            .with_operation(operation)
            .with_missing_context(self.info.path(), None, &self.provider)
    }
}

impl Output for FileWriter {
    type Item = u8;

    #[inline(always)]
    fn is_buffered(&self) -> bool {
        self.session.is_buffered()
    }

    unsafe fn write_unchecked(&mut self, input: &[u8], index: usize, count: usize) -> IoResult<usize> {
        if self.state != WriterState::Open {
            return Err(self.closed_io_error());
        }
        self.check_write_limit(count)?;
        // SAFETY: The caller guarantees the same range contract required by
        // the wrapped output session.
        match unsafe { self.session.write_unchecked(input, index, count) } {
            Ok(value) => {
                if let Err(error) = self.record_written_bytes(value) {
                    self.state = WriterState::Indeterminate;
                    return Err(error);
                }
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
                WriterState::Open | WriterState::NotPublished | WriterState::Published
            )
        {
            let _ = self.session.abort();
        }
    }
}
