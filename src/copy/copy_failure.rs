// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through public
// facade tests.
//! Recoverable facade copy failure.

use std::error::Error;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use crate::copy::CopyFailureState;
use crate::copy::CopyStats;
use crate::error::FsError;
use crate::write::FileWriter;

/// A copy error with publication state, partial statistics, and optional writer
/// recovery.
struct CopyFailureParts {
    /// Contextual filesystem error that caused the copy to fail.
    error: FsError,
    /// Confirmed destination publication state at failure time.
    state: CopyFailureState,
    /// Transfer progress confirmed before the failure.
    partial_stats: CopyStats,
    /// Destination writer retained when explicit recovery remains possible.
    writer: Option<Box<FileWriter>>,
}

/// A copy error with publication state, partial statistics, and optional writer
/// recovery.
pub struct CopyFailure {
    parts: Box<CopyFailureParts>,
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
        let writer = writer.map(Box::new);
        Self {
            parts: Box::new(CopyFailureParts {
                error,
                state,
                partial_stats,
                writer,
            }),
        }
    }
    /// Returns the contextual filesystem error.
    #[inline(always)]
    #[must_use]
    pub const fn error(&self) -> &FsError {
        &self.parts.error
    }
    /// Returns the publication state at failure.
    #[inline(always)]
    #[must_use]
    pub const fn state(&self) -> CopyFailureState {
        self.parts.state
    }
    /// Returns statistics accumulated before failure.
    #[inline(always)]
    #[must_use]
    pub const fn partial_stats(&self) -> &CopyStats {
        &self.parts.partial_stats
    }
    /// Returns whether a writer is available for recovery.
    #[inline(always)]
    #[must_use]
    pub const fn has_writer(&self) -> bool {
        self.parts.writer.is_some()
    }

    /// Returns the recovery writer if retained.
    #[inline(always)]
    #[must_use]
    pub fn writer(&self) -> Option<&FileWriter> {
        self.parts.writer.as_deref()
    }

    /// Returns a mutable recovery writer if retained.
    #[inline(always)]
    #[must_use]
    pub fn writer_mut(&mut self) -> Option<&mut FileWriter> {
        self.parts.writer.as_deref_mut()
    }

    /// Takes ownership of the recovery writer when recovery responsibility
    /// remains with the caller.
    #[inline(always)]
    #[must_use]
    pub fn take_writer(&mut self) -> Option<FileWriter> {
        self.parts.writer.take().map(|writer| *writer)
    }

    /// Splits the failure into error, state, statistics, and writer recovery.
    #[inline(always)]
    #[must_use]
    pub fn into_parts(self) -> (FsError, CopyFailureState, CopyStats, Option<FileWriter>) {
        let mut parts = self.parts;
        (
            parts.error,
            parts.state,
            parts.partial_stats,
            parts.writer.take().map(|writer| *writer),
        )
    }
}
impl Debug for CopyFailure {
    /// Formats safe failure facts without exposing a provider writer session.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("CopyFailure")
            .field("error", &self.parts.error)
            .field("state", &self.parts.state)
            .field("partial_stats", &self.parts.partial_stats)
            .field("has_writer", &self.parts.writer.is_some())
            .finish()
    }
}

impl Display for CopyFailure {
    /// Formats the wrapped file-system error while keeping the recovery state
    /// intentionally separate.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self.error(), formatter)
    }
}

impl Error for CopyFailure {
    /// Returns the underlying file-system error.
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.error())
    }
}
