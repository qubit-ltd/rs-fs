// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// facade tests.
//! Recoverable failure returned by an asynchronous copy operation.

use std::error::Error;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use crate::copy::CopyFailureState;
use crate::copy::CopyStats;
use crate::error::FsError;

/// Copy failure facts retained after an asynchronous copy operation.
///
/// # Examples
///
/// ```rust
/// # mod support { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/common/rustdoc_support.rs")); }
/// # use support::*;
/// # let (filesystem, _) = async_recording_spi::async_recording_file_system(
/// #     AsyncRecordingConfig { copy_failure: true, ..Default::default() },
/// # );
/// # poll_support::ready(async {
/// use qubit_fs::Path;
/// use qubit_fs::copy::CopyOptions;
/// use qubit_fs::error::FsErrorKind;
///
/// let mut operation = filesystem.begin_copy(
///     Path::parse("/source")?, Path::parse("/target")?, CopyOptions::default(),
/// )?;
/// let failure = operation.execute().await.expect_err("provider copy should fail");
/// let (error, state, stats) = failure.into_parts();
/// assert_eq!(FsErrorKind::Io, error.kind());
/// assert_eq!(0, stats.bytes);
/// assert!(matches!(state, qubit_fs::copy::CopyFailureState::Indeterminate));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// # }).unwrap();
/// ```
pub struct AsyncCopyFailure {
    /// Contextual filesystem error that caused the copy to fail.
    error: FsError,
    /// Confirmed destination publication state at failure time.
    state: CopyFailureState,
    /// Transfer progress confirmed before the failure.
    partial_stats: CopyStats,
}

impl AsyncCopyFailure {
    /// Creates a failure from facade-confirmed facts.
    pub(crate) fn new(error: FsError, state: CopyFailureState, partial_stats: CopyStats) -> Self {
        Self {
            error,
            state,
            partial_stats,
        }
    }

    /// Returns the contextual filesystem error.
    #[inline]
    pub const fn error(&self) -> &FsError {
        &self.error
    }

    /// Returns the confirmed publication state.
    #[inline]
    #[must_use]
    pub const fn state(&self) -> CopyFailureState {
        self.state
    }

    /// Returns partial transfer statistics.
    #[inline]
    #[must_use]
    pub const fn partial_stats(&self) -> &CopyStats {
        &self.partial_stats
    }

    /// Splits the failure into owned error, state, and progress facts.
    #[inline]
    pub fn into_parts(self) -> (FsError, CopyFailureState, CopyStats) {
        (self.error, self.state, self.partial_stats)
    }
}

impl Debug for AsyncCopyFailure {
    /// Formats failure facts without exposing a provider session.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("AsyncCopyFailure")
            .field("error", &self.error)
            .field("state", &self.state)
            .field("partial_stats", &self.partial_stats)
            .finish()
    }
}

impl Display for AsyncCopyFailure {
    /// Formats the wrapped file-system error.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self.error(), formatter)
    }
}

impl Error for AsyncCopyFailure {
    /// Returns the underlying file-system error.
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.error())
    }
}
