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
/// # mod support { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/common/rustdoc_support.rs")); }
/// # use support::*;
/// # let (filesystem, _) = async_recording_spi::async_recording_file_system(async_recording_spi::AsyncRecordingConfig {
/// #     writer_commit_failure: Some(qubit_fs::write::WriteFailureState::NotPublished),
/// #     ..Default::default()
/// # });
/// # poll_support::ready(async {
/// use qubit_fs::Path;
/// use qubit_fs::write::AsyncWriterRecovery;
/// use qubit_fs::write::WriteFailureState;
/// use qubit_fs::write::WriteOptions;
///
/// let mut operation = filesystem.begin_write_all(
///     Path::parse("/report")?, b"bytes".to_vec(), WriteOptions::default(),
/// )?;
/// let failure = operation.execute().await.expect_err("fixture commit failure");
/// assert_eq!(WriteFailureState::NotPublished, failure.state());
/// let mut recovery = operation.take_recovery().expect("retained session");
/// let cleanup = match &mut recovery {
///     AsyncWriterRecovery::Opened(writer) => writer.abort_async().await,
///     AsyncWriterRecovery::Rejected(writer) => writer.abort_async().await,
/// };
/// assert!(cleanup.is_ok());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// # }).unwrap();
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
    #[inline]
    pub(crate) fn opened(&self) -> Option<&AsyncFileWriter> {
        match self {
            Self::Opened(writer) => Some(writer),
            Self::Rejected(_) => None,
        }
    }

    /// Mutably borrows a validated writer, or returns None for a rejected
    /// session.
    #[inline]
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
