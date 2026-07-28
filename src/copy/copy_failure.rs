// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Recoverable facade copy failure.

use crate::{CopyFailureState, CopyStats, FileWriter, FsError};
use std::fmt::{Debug, Formatter, Result as FmtResult};

/// A copy error with publication state, partial statistics, and optional writer recovery.
pub struct CopyFailure {
    error: FsError,
    state: CopyFailureState,
    partial_stats: CopyStats,
    writer: Option<FileWriter>,
}
impl CopyFailure {
    /// Creates a typed copy failure from validated facade facts.
    #[must_use]
    pub(crate) fn new(
        error: FsError,
        state: CopyFailureState,
        partial_stats: CopyStats,
        writer: Option<FileWriter>,
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
    /// Returns the publication state at failure.
    #[must_use]
    pub const fn state(&self) -> CopyFailureState {
        self.state
    }
    /// Returns statistics accumulated before failure.
    #[must_use]
    pub const fn partial_stats(&self) -> &CopyStats {
        &self.partial_stats
    }
    /// Returns whether a writer is available for recovery.
    #[must_use]
    pub const fn has_writer(&self) -> bool {
        self.writer.is_some()
    }
    /// Splits the failure into error, state, statistics, and writer recovery.
    #[must_use]
    pub fn into_parts(self) -> (FsError, CopyFailureState, CopyStats, Option<FileWriter>) {
        (self.error, self.state, self.partial_stats, self.writer)
    }
}
impl Debug for CopyFailure {
    /// Formats safe failure facts without exposing a provider writer session.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("CopyFailure")
            .field("error", &self.error)
            .field("state", &self.state)
            .field("partial_stats", &self.partial_stats)
            .field("has_writer", &self.writer.is_some())
            .finish()
    }
}
