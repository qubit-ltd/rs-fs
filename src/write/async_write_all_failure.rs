// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Failure returned by the asynchronous convenience whole-file write operation.

use std::error::Error;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use crate::error::FsError;
use crate::write::AsyncFileWriter;

/// An asynchronous whole-file write failure retaining a recoverable writer.
pub struct AsyncWriteAllFailure {
    /// Contextual filesystem error that interrupted the whole-file write.
    error: FsError,
    /// Opened writer retained for explicit recovery when available.
    writer: Option<Box<AsyncFileWriter>>,
}

impl AsyncWriteAllFailure {
    /// Builds a failure within the facade after a write or commit error.
    pub(crate) fn new(error: FsError, writer: Option<AsyncFileWriter>) -> Self {
        Self {
            error,
            writer: writer.map(Box::new),
        }
    }

    /// Returns the causal filesystem error.
    #[inline(always)]
    #[must_use]
    pub const fn error(&self) -> &FsError {
        &self.error
    }

    /// Returns the retained writer, if opening had completed.
    #[inline(always)]
    #[must_use]
    pub fn writer(&self) -> Option<&AsyncFileWriter> {
        self.writer.as_deref()
    }

    /// Returns a mutable retained writer for explicit recovery.
    #[inline(always)]
    #[must_use]
    pub fn writer_mut(&mut self) -> Option<&mut AsyncFileWriter> {
        self.writer.as_deref_mut()
    }

    /// Returns the causal error and optional writer.
    #[inline(always)]
    #[must_use]
    pub fn into_parts(self) -> (FsError, Option<AsyncFileWriter>) {
        (self.error, self.writer.map(|writer| *writer))
    }
}

impl Display for AsyncWriteAllFailure {
    /// Formats the causal failure without exposing writer internals.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        self.error.fmt(formatter)
    }
}

impl std::fmt::Debug for AsyncWriteAllFailure {
    /// Formats the causal error and whether recovery is available.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("AsyncWriteAllFailure")
            .field("error", &self.error)
            .field("has_writer", &self.writer.is_some())
            .finish()
    }
}

impl Error for AsyncWriteAllFailure {
    /// Returns the underlying filesystem error.
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}
