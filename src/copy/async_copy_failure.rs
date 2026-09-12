// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through public
// facade tests.
//! Recoverable failure returned by an asynchronous copy operation.

use std::error::Error;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use crate::copy::CopyFailureState;
use crate::copy::CopyStats;
use crate::error::FsError;

/// Copy failure facts retained after an asynchronous copy operation.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::copy::{AsyncCopyFailure, CopyFailureState, CopyStats};
/// use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
///
/// assert!(std::any::type_name::<AsyncCopyFailure>().contains("AsyncCopyFailure"));
/// let error = FsError::new(FsErrorKind::Io, FsOperation::Copy, "interrupted");
/// assert_eq!(CopyFailureState::Unchanged, CopyFailureState::Unchanged);
/// assert_eq!(0, CopyStats::default().bytes);
/// assert_eq!(FsOperation::Copy, error.operation());
/// ```
pub struct AsyncCopyFailure {
    /// Contextual filesystem error that caused the copy to fail.
    error: FsError,
    /// Confirmed destination publication state at failure time.
    state: CopyFailureState,
    /// Transfer progress confirmed before the failure.
    partial_stats: CopyStats,
}

impl AsyncCopyFailure {
    /// Creates a failure from facade-confirmed facts.
    pub(crate) fn new(error: FsError, state: CopyFailureState, partial_stats: CopyStats) -> Self {
        Self {
            error,
            state,
            partial_stats,
        }
    }

    /// Returns the contextual filesystem error.
    #[inline]
    #[must_use]
    pub const fn error(&self) -> &FsError {
        &self.error
    }

    /// Returns the confirmed publication state.
    #[inline]
    #[must_use]
    pub const fn state(&self) -> CopyFailureState {
        self.state
    }

    /// Returns partial transfer statistics.
    #[inline]
    #[must_use]
    pub const fn partial_stats(&self) -> &CopyStats {
        &self.partial_stats
    }

    /// Splits the failure into owned error, state, and progress facts.
    #[inline]
    #[must_use]
    pub fn into_parts(self) -> (FsError, CopyFailureState, CopyStats) {
        (self.error, self.state, self.partial_stats)
    }
}

impl Debug for AsyncCopyFailure {
    /// Formats failure facts without exposing a provider session.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("AsyncCopyFailure")
            .field("error", &self.error)
            .field("state", &self.state)
            .field("partial_stats", &self.partial_stats)
            .finish()
    }
}

impl Display for AsyncCopyFailure {
    /// Formats the wrapped file-system error.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self.error(), formatter)
    }
}

impl Error for AsyncCopyFailure {
    /// Returns the underlying file-system error.
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.error())
    }
}

#[cfg(test)]
mod tests {
    use super::AsyncCopyFailure;
    use crate::copy::CopyFailureState;
    use crate::copy::CopyStats;
    use crate::error::FsError;
    use crate::error::FsErrorKind;
    use crate::error::FsOperation;

    #[test]
    fn owned_parts_are_executed_at_runtime() {
        let failure = AsyncCopyFailure::new(
            FsError::new(FsErrorKind::NotFound, FsOperation::Copy, "missing source"),
            CopyFailureState::Unchanged,
            CopyStats {
                files: 1,
                ..CopyStats::default()
            },
        );

        let (error, state, stats) = failure.into_parts();
        assert_eq!(error.kind(), FsErrorKind::NotFound);
        assert_eq!(state, CopyFailureState::Unchanged);
        assert_eq!(stats.files, 1);
    }
}
