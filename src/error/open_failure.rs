// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Opening failures that preserve isolated recovery ownership.

use std::error::Error;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use super::FsError;
use super::OpenFailureStage;

/// A failed open with its causal error and optional isolated recovery session.
///
/// `R` owns only explicit cleanup authority. Keep this error or take its
/// recovery session before reporting failure. Dropping the error does not
/// confirm cleanup. There is deliberately no conversion into `FsError` that
/// discards recovery.
///
/// # Type Parameters
///
/// - `R`: Recovery session type retained after the provider open fails.
///
/// # Examples
///
/// ```rust
/// # mod support { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/common/rustdoc_support.rs")); }
///
/// # let filesystem = support::rustdoc_provider::filesystem();
/// use qubit_fs::Path;
/// use qubit_fs::metadata::AtomicityRequirement;
/// use qubit_fs::write::WriteOptions;
///
/// let path = Path::parse("/report")?;
/// let result = filesystem.open_writer(
///     &path,
///     WriteOptions::default().with_atomicity(AtomicityRequirement::Required),
/// );
/// let failure = result.expect_err("the fixture does not guarantee atomic writes");
/// assert_eq!(failure.stage(), qubit_fs::error::OpenFailureStage::Preflight);
/// assert_eq!(failure.error().operation(), qubit_fs::error::FsOperation::OpenWriter);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use = "inspect and retain recovery ownership before abandoning an open failure"]
pub struct OpenFailure<R> {
    /// Original contextual failure, unchanged by later recovery.
    error: Box<FsError>,
    /// Stage where opening stopped.
    stage: OpenFailureStage,
    /// Isolated session, present when a successfully opened envelope was
    /// rejected.
    recovery: Option<R>,
}

impl<R> OpenFailure<R> {
    /// Creates a failure after classifying the stage and transferring
    /// ownership.
    ///
    /// # Parameters
    ///
    /// - `error`: Filesystem error whose kind, source, and established facts
    ///   are retained.
    /// - `stage`: Stage at which the open operation failed.
    /// - `recovery`: Optional isolated session retained for safe recovery.
    ///
    /// # Returns
    /// A failure value retaining the original error, stage, and optional
    /// recovery session.
    pub(crate) fn new(error: FsError, stage: OpenFailureStage, recovery: Option<R>) -> Self {
        Self {
            error: Box::new(error),
            stage,
            recovery,
        }
    }

    /// Returns the original failure; cleanup does not rewrite it.
    ///
    /// # Returns
    /// A borrow of the original contextual filesystem error.
    #[must_use = "inspect the original filesystem error before discarding it"]
    #[inline]
    pub const fn error(&self) -> &FsError {
        &self.error
    }

    /// Returns the stage where opening stopped.
    ///
    /// # Returns
    /// The stage at which opening stopped.
    #[must_use = "use the opening stage to classify the failure"]
    #[inline]
    pub const fn stage(&self) -> OpenFailureStage {
        self.stage
    }

    /// Returns the retained recovery session, or None before an envelope
    /// arrived.
    ///
    /// # Returns
    /// `Some` with a shared borrow of the retained session, or `None` if no
    /// recovery session was retained.
    #[inline]
    pub const fn recovery(&self) -> Option<&R> {
        self.recovery.as_ref()
    }

    /// Borrows the retained session for explicit cleanup, when available.
    ///
    /// # Returns
    /// `Some` with a mutable borrow of the retained session, or `None` if no
    /// recovery session was retained.
    #[inline]
    pub fn recovery_mut(&mut self) -> Option<&mut R> {
        self.recovery.as_mut()
    }

    /// Transfers recovery ownership without changing the historical failure.
    ///
    /// # Returns
    /// `Some` with the owned recovery session, or `None` if recovery was
    /// already absent or previously taken.
    #[inline]
    pub fn take_recovery(&mut self) -> Option<R> {
        self.recovery.take()
    }

    /// Returns all failure facts and transfers any recovery ownership.
    ///
    /// # Returns
    /// The original error, opening stage, and optional retained recovery
    /// session.
    #[must_use = "retain or inspect the failure facts returned by this method"]
    pub fn into_parts(self) -> (FsError, OpenFailureStage, Option<R>) {
        (*self.error, self.stage, self.recovery)
    }
}

impl<R> Debug for OpenFailure<R> {
    /// Formats failure facts without formatting the session or its claimed
    /// identity.
    ///
    /// # Parameters
    ///
    /// - `f`: Formatter that receives the debug or display representation.
    ///
    /// # Returns
    /// The formatter result, indicating whether writing the debug output
    /// succeeded.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("OpenFailure")
            .field("error", &self.error)
            .field("stage", &self.stage)
            .field("has_recovery", &self.recovery.is_some())
            .finish()
    }
}

impl<R> Display for OpenFailure<R> {
    /// Formats only the original contextual failure.
    ///
    /// # Parameters
    ///
    /// - `f`: Formatter that receives the debug or display representation.
    ///
    /// # Returns
    /// The formatter result, indicating whether writing the display output
    /// succeeded.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(&self.error, f)
    }
}

impl<R: 'static> Error for OpenFailure<R> {
    /// Exposes the original error chain without surrendering recovery
    /// ownership.
    ///
    /// # Returns
    /// A reference to the original error as a trait object.
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.error.as_ref())
    }
}
