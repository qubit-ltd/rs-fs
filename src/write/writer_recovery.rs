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

use crate::write::FileWriter;
use crate::write::RejectedWriter;

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
/// use qubit_fs::error::FsResult;
/// use qubit_fs::write::WriteAllFailure;
///
/// fn clean_up(failure: &mut WriteAllFailure) -> FsResult<()> {
///     if let Some(recovery) = failure.recovery_mut() {
///         match recovery {
///             qubit_fs::write::WriterRecovery::Opened(writer) => {
///                 writer.abort()?;
///             }
///             qubit_fs::write::WriterRecovery::Rejected(writer) => {
///                 writer.abort()?;
///             }
///         }
///     }
///     Ok(())
/// }
/// ```
#[must_use]
pub enum WriterRecovery {
    /// A validated writer retained for explicit recovery.
    Opened(Box<FileWriter>),
    /// A provider session whose returned identity failed validation.
    Rejected(RejectedWriter),
}

impl Debug for WriterRecovery {
    /// Formats the variant without exposing provider session internals.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(match self {
            Self::Opened(_) => "WriterRecovery::Opened",
            Self::Rejected(_) => "WriterRecovery::Rejected",
        })
    }
}
