// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Failure returned by the convenience whole-file write operation.

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

use crate::{FileWriter, FsError};

/// A whole-file write failure retaining the recoverable writer when available.
pub struct WriteAllFailure {
    error: FsError,
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
    #[must_use]
    pub const fn error(&self) -> &FsError {
        &self.error
    }
    /// Returns the retained writer, if opening had completed.
    #[must_use]
    pub fn writer(&self) -> Option<&FileWriter> {
        self.writer.as_deref()
    }
    /// Returns a mutable retained writer for explicit recovery.
    #[must_use]
    pub fn writer_mut(&mut self) -> Option<&mut FileWriter> {
        self.writer.as_deref_mut()
    }
    /// Returns the causal error and optional writer.
    #[must_use]
    pub fn into_parts(self) -> (FsError, Option<FileWriter>) {
        (self.error, self.writer.map(|writer| *writer))
    }
}

impl Display for WriteAllFailure {
    /// Formats the causal failure without exposing writer internals.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        self.error.fmt(formatter)
    }
}

impl std::fmt::Debug for WriteAllFailure {
    /// Formats the causal error and whether recovery is available.
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
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}
