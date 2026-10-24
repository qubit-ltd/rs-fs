// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// facade tests.
//! Recoverable facade copy failure.

use std::error::Error;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use crate::copy::CopyFailureState;
use crate::copy::CopyStats;
use crate::copy::internal::CopyFailureParts;
use crate::error::FsError;
use crate::write::WriterRecovery;

/// A copy error with publication state, partial statistics, and optional writer
/// recovery.
///
/// # Examples
///
/// The example runs against an isolated in-memory fixture. Applications obtain
/// their configured facade from a provider or registry integration.
///
/// ```rust
/// # mod support { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/common/rustdoc_support.rs")); }
/// # let filesystem = support::rustdoc_provider::filesystem();
/// use qubit_fs::Path;
/// use qubit_fs::copy::CopyOptions;
/// use qubit_fs::copy::CopyFailureState;
///
/// let failure = filesystem.copy(
///     &Path::parse("/missing")?, &Path::parse("/copy")?, CopyOptions::default(),
/// ).expect_err("the fixture has no source");
/// assert_eq!(CopyFailureState::Unchanged, failure.state());
/// assert_eq!(0, failure.partial_stats().bytes);
/// assert!(!failure.has_recovery());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use = "inspect or explicitly recover from the copy failure"]
pub struct CopyFailure {
    /// Heap-owned error, publication state, progress, and recovery writer.
    parts: Box<CopyFailureParts>,
}
impl CopyFailure {
    /// Creates a typed copy failure from validated facade facts.
    ///
    /// # Parameters
    /// - `error`: Contextual filesystem error that caused the copy to fail.
    /// - `state`: Confirmed destination publication state.
    /// - `partial_stats`: Transfer progress confirmed before failure.
    /// - `writer`: Optional writer retained for recovery.
    ///
    /// # Returns
    /// A copy failure retaining the supplied facts.
    pub(crate) fn new(
        error: FsError,
        state: CopyFailureState,
        partial_stats: CopyStats,
        writer: Option<WriterRecovery>,
    ) -> Self {
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
    ///
    /// # Returns
    /// The error that caused the copy operation to fail.
    #[inline]
    #[must_use = "inspect the filesystem error instead of silently discarding it"]
    pub const fn error(&self) -> &FsError {
        &self.parts.error
    }
    /// Returns the publication state at failure.
    ///
    /// # Returns
    /// The state confirmed when the operation failed.
    #[inline]
    pub const fn state(&self) -> CopyFailureState {
        self.parts.state
    }
    /// Returns statistics accumulated before failure.
    ///
    /// # Returns
    /// A reference to the progress counters accumulated before failure.
    #[inline]
    #[must_use]
    pub const fn partial_stats(&self) -> &CopyStats {
        &self.parts.partial_stats
    }
    /// Returns whether a writer is available for recovery.
    ///
    /// # Returns
    /// `true` when a recovery writer is retained, or `false` otherwise.
    #[inline]
    #[must_use]
    pub const fn has_recovery(&self) -> bool {
        self.parts.writer.is_some()
    }

    /// Returns the recovery writer if retained.
    ///
    /// # Returns
    /// `Some` with a borrowed recovery writer when one is retained, or `None`
    /// otherwise.
    #[inline]
    #[must_use = "inspect the recovery state before abandoning the retained writer"]
    pub fn recovery(&self) -> Option<&WriterRecovery> {
        self.parts.writer.as_ref()
    }

    /// Returns a mutable recovery writer if retained.
    ///
    /// # Returns
    /// `Some` with mutable access to a retained recovery writer, or `None`
    /// otherwise.
    #[inline]
    #[must_use = "inspect the recovery state before abandoning the retained writer"]
    pub fn recovery_mut(&mut self) -> Option<&mut WriterRecovery> {
        self.parts.writer.as_mut()
    }

    /// Takes ownership of the recovery writer when recovery responsibility
    /// remains with the caller.
    ///
    /// # Returns
    /// `Some` with the retained recovery writer, or `None` if it was absent or
    /// already taken.
    #[inline]
    pub fn take_recovery(&mut self) -> Option<WriterRecovery> {
        self.parts.writer.take()
    }

    /// Splits the failure into error, state, statistics, and writer recovery.
    ///
    /// # Returns
    /// The filesystem error, confirmed publication state, partial statistics,
    /// and optional writer retained for recovery.
    #[inline]
    pub fn into_parts(self) -> (FsError, CopyFailureState, CopyStats, Option<WriterRecovery>) {
        let mut parts = self.parts;
        (parts.error, parts.state, parts.partial_stats, parts.writer.take())
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
            .field("has_recovery", &self.parts.writer.is_some())
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
