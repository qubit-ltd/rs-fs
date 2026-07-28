// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Recoverable failure returned by an asynchronous copy operation.

use std::fmt::{
    Debug,
    Formatter,
    Result as FmtResult,
};

use crate::{
    AsyncFileWriter,
    CopyFailureState,
    CopyStats,
    FsError,
};

/// Copy failure facts and an optional writer retained by the operation.
pub struct AsyncCopyFailure {
    error: FsError,
    state: CopyFailureState,
    partial_stats: CopyStats,
    writer: Option<AsyncFileWriter>,
}

impl AsyncCopyFailure {
    /// Creates a failure from facade-confirmed facts.
    pub(crate) fn new(
        error: FsError,
        state: CopyFailureState,
        partial_stats: CopyStats,
        writer: Option<AsyncFileWriter>,
    ) -> Self {
        Self {
            error,
            state,
            partial_stats,
            writer,
        }
    }

    /// Returns the contextual filesystem error.
    #[must_use]
    pub const fn error(&self) -> &FsError {
        &self.error
    }

    /// Returns the confirmed publication state.
    #[must_use]
    pub const fn state(&self) -> CopyFailureState {
        self.state
    }

    /// Returns partial transfer statistics.
    #[must_use]
    pub const fn partial_stats(&self) -> &CopyStats {
        &self.partial_stats
    }

    /// Splits the failure into owned facts and optional writer recovery.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        FsError,
        CopyFailureState,
        CopyStats,
        Option<AsyncFileWriter>,
    ) {
        (self.error, self.state, self.partial_stats, self.writer)
    }
}

impl Debug for AsyncCopyFailure {
    /// Formats failure facts without exposing a provider session.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("AsyncCopyFailure")
            .field("error", &self.error)
            .field("state", &self.state)
            .field("partial_stats", &self.partial_stats)
            .field("has_writer", &self.writer.is_some())
            .finish()
    }
}
