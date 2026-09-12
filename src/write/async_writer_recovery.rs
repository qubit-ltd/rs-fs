// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Ownership retained after an aggregate write or copy failure.

use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use crate::write::AsyncFileWriter;
use crate::write::RejectedAsyncWriter;

/// A retained session that is either validated or restricted to explicit
/// cleanup.
///
/// Matching the variant is required before performing recovery. A rejected
/// session cannot write or publish data. This value does not describe the
/// historical publication state; use the failure snapshot for that fact.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::write::AsyncWriterRecovery;
///
/// fn observe(_recovery: AsyncWriterRecovery) {}
/// ```
#[must_use]
pub enum AsyncWriterRecovery {
    /// A validated writer retained for explicit recovery.
    Opened(Box<AsyncFileWriter>),
    /// A provider session whose returned identity failed validation.
    Rejected(RejectedAsyncWriter),
}

impl AsyncWriterRecovery {
    /// Borrows a validated writer, or returns None for a rejected session.
    pub(crate) fn opened(&self) -> Option<&AsyncFileWriter> {
        match self {
            Self::Opened(writer) => Some(writer),
            Self::Rejected(_) => None,
        }
    }

    /// Mutably borrows a validated writer, or returns None for a rejected
    /// session.
    pub(crate) fn opened_mut(&mut self) -> Option<&mut AsyncFileWriter> {
        match self {
            Self::Opened(writer) => Some(writer),
            Self::Rejected(_) => None,
        }
    }
}

impl Debug for AsyncWriterRecovery {
    /// Formats the variant without exposing provider session internals.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(match self {
            Self::Opened(_) => "AsyncWriterRecovery::Opened",
            Self::Rejected(_) => "AsyncWriterRecovery::Rejected",
        })
    }
}
