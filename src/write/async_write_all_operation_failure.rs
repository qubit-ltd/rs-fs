// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Failure for an owning asynchronous whole-file write.
use std::error::Error;
use std::fmt::Display;
use std::fmt::Formatter;

use crate::error::FsError;
use crate::write::WriteFailureState;
/// Error returned by an owning asynchronous whole-file write.
///
/// Recovery ownership remains in the operation even when this error is
/// consumed.
///
/// # Examples
/// ```rust
/// # mod support { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/common/rustdoc_support.rs")); }
/// # use support::*;
/// # let (filesystem, _) = async_recording_spi::async_recording_file_system(async_recording_spi::AsyncRecordingConfig {
/// #     writer_commit_failure: Some(qubit_fs::write::WriteFailureState::NotPublished),
/// #     ..Default::default()
/// # });
/// # poll_support::ready(async {
/// use qubit_fs::Path;
/// use qubit_fs::write::WriteOptions;
/// use qubit_fs::write::WriteFailureState;
/// let mut operation = filesystem.begin_write_all(
///     Path::parse("/report")?, b"bytes".to_vec(), WriteOptions::default(),
/// )?;
/// let failure = operation.execute().await.expect_err("fixture commit failure");
/// assert_eq!(WriteFailureState::NotPublished, failure.state());
/// assert_eq!(5, failure.written_bytes());
/// assert!(operation.has_recovery_writer());
/// let cleanup = operation.recovery_writer().unwrap().abort_async().await;
/// assert!(cleanup.is_ok());
/// // Application error handling can retain `failure`, `cleanup`, and `operation`.
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// # }).unwrap();
/// ```
#[must_use]
pub struct AsyncWriteAllOperationFailure {
    /// Original contextual error.
    error: FsError,
    /// Publication facts at failure.
    state: WriteFailureState,
    /// Bytes acknowledged by completed writes.
    written_bytes: u64,
}
impl AsyncWriteAllOperationFailure {
    /// Creates a failure with its recovery facts.
    pub(crate) fn new(error: FsError, state: WriteFailureState, written_bytes: u64) -> Self {
        Self {
            error,
            state,
            written_bytes,
        }
    }
    /// Returns the contextual filesystem error.
    #[must_use]
    pub const fn error(&self) -> &FsError {
        &self.error
    }
    /// Returns the publication facts recorded when execution stopped.
    #[must_use]
    pub const fn state(&self) -> WriteFailureState {
        self.state
    }
    /// Returns bytes acknowledged by completed writes before execution stopped.
    #[must_use]
    pub const fn written_bytes(&self) -> u64 {
        self.written_bytes
    }
    /// Consumes the failure and returns its filesystem error.
    ///
    /// This discards publication state and confirmed byte count. The operation
    /// still owns any recovery writer; prefer `into_parts` when recovering.
    #[must_use]
    pub fn into_error(self) -> FsError {
        self.error
    }
    /// Consumes the failure and returns all recovery facts.
    #[must_use]
    pub fn into_parts(self) -> (FsError, WriteFailureState, u64) {
        (self.error, self.state, self.written_bytes)
    }
}
impl Display for AsyncWriteAllOperationFailure {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl std::fmt::Debug for AsyncWriteAllOperationFailure {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncWriteAllOperationFailure")
            .field("error", &self.error)
            .field("state", &self.state)
            .field("written_bytes", &self.written_bytes)
            .finish()
    }
}
impl Error for AsyncWriteAllOperationFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}
