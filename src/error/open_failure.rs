// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
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
/// # Examples
///
/// ```rust
/// use qubit_fs::error::{FsError, FsErrorKind, FsOperation, OpenFailure, OpenFailureStage};
///
/// assert!(std::any::type_name::<OpenFailure<()>>().contains("OpenFailure"));
/// let error = FsError::new(FsErrorKind::NotFound, FsOperation::OpenReader, "missing");
/// assert_eq!(FsOperation::OpenReader, error.operation());
/// assert_eq!(OpenFailureStage::Preflight, OpenFailureStage::Preflight);
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
    pub(crate) fn new(error: FsError, stage: OpenFailureStage, recovery: Option<R>) -> Self {
        Self {
            error: Box::new(error),
            stage,
            recovery,
        }
    }
    /// Returns the original failure; cleanup does not rewrite it.
    pub const fn error(&self) -> &FsError {
        &self.error
    }
    /// Returns the stage where opening stopped.
    pub const fn stage(&self) -> OpenFailureStage {
        self.stage
    }
    /// Returns the retained recovery session, or None before an envelope
    /// arrived.
    pub const fn recovery(&self) -> Option<&R> {
        self.recovery.as_ref()
    }
    /// Borrows the retained session for explicit cleanup, when available.
    pub fn recovery_mut(&mut self) -> Option<&mut R> {
        self.recovery.as_mut()
    }
    /// Transfers recovery ownership without changing the historical failure.
    pub fn take_recovery(&mut self) -> Option<R> {
        self.recovery.take()
    }
    /// Returns all failure facts and transfers any recovery ownership.
    ///
    /// # Returns
    /// The original error, opening stage, and optional retained recovery
    /// session.
    pub fn into_parts(self) -> (FsError, OpenFailureStage, Option<R>) {
        (*self.error, self.stage, self.recovery)
    }
}
impl<R> Debug for OpenFailure<R> {
    /// Formats failure facts without formatting the session or its claimed
    /// identity.
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
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(&self.error, f)
    }
}
impl<R: 'static> Error for OpenFailure<R> {
    /// Exposes the original error chain without surrendering recovery
    /// ownership.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.error.as_ref())
    }
}
