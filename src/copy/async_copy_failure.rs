// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow all -- facade integration tests exercise this API group.
//! Recoverable failure returned by an asynchronous copy operation.

use std::fmt::{
    Debug,
    Formatter,
    Result as FmtResult,
};

use crate::{
    CopyFailureState,
    CopyStats,
    FsError,
};

/// Copy failure facts retained after an asynchronous copy operation.
pub struct AsyncCopyFailure {
    error: FsError,
    state: CopyFailureState,
    partial_stats: CopyStats,
}

impl AsyncCopyFailure {
    /// Creates a failure from facade-confirmed facts.
    pub(crate) fn new(
        error: FsError,
        state: CopyFailureState,
        partial_stats: CopyStats,
    ) -> Self {
        Self {
            error,
            state,
            partial_stats,
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

    /// Splits the failure into owned error, state, and progress facts.
    #[must_use]
    pub fn into_parts(self) -> (FsError, CopyFailureState, CopyStats) {
        (self.error, self.state, self.partial_stats)
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
            .finish()
    }
}
