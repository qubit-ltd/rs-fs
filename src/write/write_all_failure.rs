// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Failure returned by the convenience whole-file write operation.

use std::error::Error;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use crate::error::FsError;
use crate::write::WriteFailureState;
use crate::write::WriterRecovery;

/// A whole-file write failure retaining the recoverable writer when available.
pub struct WriteAllFailure {
    /// Contextual filesystem error that interrupted the whole-file write.
    error: Box<FsError>,
    /// Immutable publication certainty captured at failure.
    state: WriteFailureState,
    /// Bytes acknowledged before the failure, independent of later recovery.
    written_bytes: u64,
    /// Opened writer retained for explicit recovery when available.
    writer: Option<WriterRecovery>,
}

impl WriteAllFailure {
    /// Builds a failure within the facade after a write or commit error.
    pub(crate) fn new(
        error: FsError,
        state: WriteFailureState,
        written_bytes: u64,
        writer: Option<WriterRecovery>,
    ) -> Self {
        Self {
            error: Box::new(error),
            state,
            written_bytes,
            writer,
        }
    }
    /// Returns the causal filesystem error.
    #[inline(always)]
    #[must_use]
    pub const fn error(&self) -> &FsError {
        &self.error
    }
    /// Returns publication certainty at the original failure, even after
    /// recovery.
    #[must_use]
    pub const fn state(&self) -> WriteFailureState {
        self.state
    }
    /// Returns the original acknowledged byte count, excluding uncertain
    /// writes.
    #[must_use]
    pub const fn written_bytes(&self) -> u64 {
        self.written_bytes
    }
    /// Transfers the retained session without changing the failure snapshot.
    #[must_use]
    pub fn take_recovery(&mut self) -> Option<WriterRecovery> {
        self.writer.take()
    }
    /// Returns the retained writer, if opening had completed.
    #[inline(always)]
    #[must_use]
    pub fn recovery(&self) -> Option<&WriterRecovery> {
        self.writer.as_ref()
    }
    /// Returns a mutable retained writer for explicit recovery.
    #[inline(always)]
    #[must_use]
    pub fn recovery_mut(&mut self) -> Option<&mut WriterRecovery> {
        self.writer.as_mut()
    }
    /// Returns the causal error and optional writer.
    #[inline(always)]
    #[must_use]
    pub fn into_parts(self) -> (FsError, WriteFailureState, u64, Option<WriterRecovery>) {
        (*self.error, self.state, self.written_bytes, self.writer)
    }
}

impl Display for WriteAllFailure {
    /// Formats the causal failure without exposing writer internals.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        self.error.fmt(formatter)
    }
}

impl std::fmt::Debug for WriteAllFailure {
    /// Formats the causal error and whether recovery is available.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("WriteAllFailure")
            .field("error", &self.error)
            .field("state", &self.state)
            .field("written_bytes", &self.written_bytes)
            .field("has_recovery", &self.writer.is_some())
            .finish()
    }
}

impl Error for WriteAllFailure {
    /// Returns the underlying filesystem error.
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.error.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use std::hint::black_box;

    use super::WriteAllFailure;
    use crate::error::FsError;
    use crate::error::FsErrorKind;
    use crate::error::FsOperation;
    use crate::write::WriteFailureState;

    #[test]
    fn failure_accessors_are_executed_at_runtime() {
        let error: fn(&WriteAllFailure) -> &FsError = black_box(WriteAllFailure::error);
        let recovery: fn(&WriteAllFailure) -> Option<&crate::write::WriterRecovery> =
            black_box(WriteAllFailure::recovery);
        let failure = WriteAllFailure::new(
            FsError::new(FsErrorKind::NotFound, FsOperation::OpenWriter, "missing target"),
            WriteFailureState::NotPublished,
            4,
            None,
        );

        assert_eq!(FsErrorKind::NotFound, error(&failure).kind());
        assert!(recovery(&failure).is_none());
    }
}
