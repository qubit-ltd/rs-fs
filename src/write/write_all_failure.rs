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
use crate::write::FileWriter;

/// A whole-file write failure retaining the recoverable writer when available.
pub struct WriteAllFailure {
    /// Contextual filesystem error that interrupted the whole-file write.
    error: FsError,
    /// Opened writer retained for explicit recovery when available.
    writer: Option<Box<FileWriter>>,
}

impl WriteAllFailure {
    /// Builds a failure within the facade after a write or commit error.
    pub(crate) fn new(error: FsError, writer: Option<FileWriter>) -> Self {
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
    pub fn writer(&self) -> Option<&FileWriter> {
        self.writer.as_deref()
    }
    /// Returns a mutable retained writer for explicit recovery.
    #[inline(always)]
    #[must_use]
    pub fn writer_mut(&mut self) -> Option<&mut FileWriter> {
        self.writer.as_deref_mut()
    }
    /// Returns the causal error and optional writer.
    #[inline(always)]
    #[must_use]
    pub fn into_parts(self) -> (FsError, Option<FileWriter>) {
        (self.error, self.writer.map(|writer| *writer))
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
            .field("has_writer", &self.writer.is_some())
            .finish()
    }
}

impl Error for WriteAllFailure {
    /// Returns the underlying filesystem error.
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}
