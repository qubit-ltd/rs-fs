// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// facade tests.
//! Typed facade rename failure.

use std::error::Error;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use crate::error::FsError;
use crate::rename::RenameFailureState;

/// A rename failure that preserves the provider's publication-state fact.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::rename::{RenameFailure, RenameFailureState, RenameOutcome};
///
/// fn inspect_rename(result: Result<RenameOutcome, RenameFailure>) {
///     if let Err(failure) = result {
///         let state: RenameFailureState = failure.state();
///         let operation = failure.error().operation();
///         println!("rename failed during {operation:?}: {state:?}");
///     }
/// }
/// ```
#[must_use = "inspect the rename state before deciding how to recover"]
pub struct RenameFailure {
    /// Contextual filesystem error that interrupted the rename.
    error: FsError,
    /// Provider-confirmed source and destination transition state.
    state: RenameFailureState,
}
impl RenameFailure {
    /// Creates a typed facade rename failure.
    ///
    /// # Parameters
    /// - `error`: Contextual error that interrupted the operation.
    /// - `state`: Provider-confirmed state of the source and destination.
    ///
    /// # Returns
    /// A failure retaining both the error and publication state.
    pub(crate) const fn new(error: FsError, state: RenameFailureState) -> Self {
        Self { error, state }
    }
    /// Borrows the contextual filesystem error that interrupted the rename.
    ///
    /// The returned error retains operation context and may carry a
    /// lower-level source error.
    ///
    /// # Returns
    /// The error retained by this failure.
    #[inline]
    #[must_use = "inspect the filesystem error before handling the rename failure"]
    pub const fn error(&self) -> &FsError {
        &self.error
    }
    /// Returns the provider-confirmed source and target transition state.
    ///
    /// `Indeterminate` means the provider cannot establish whether the rename
    /// completed, so callers must not assume either the old or new layout.
    ///
    /// # Returns
    /// The publication state observed when the operation failed.
    #[inline]
    #[must_use]
    pub const fn state(&self) -> RenameFailureState {
        self.state
    }
    /// Consumes the failure and returns its error and publication state.
    ///
    /// # Returns
    /// The retained contextual error and provider-confirmed transition state.
    #[inline]
    #[must_use = "retain and inspect the error and rename state"]
    pub fn into_parts(self) -> (FsError, RenameFailureState) {
        (self.error, self.state)
    }
}
impl Debug for RenameFailure {
    /// Formats the safe typed failure facts.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("RenameFailure")
            .field("error", &self.error)
            .field("state", &self.state)
            .finish()
    }
}

impl Display for RenameFailure {
    /// Formats the wrapped file-system error.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(&self.error, formatter)
    }
}

impl Error for RenameFailure {
    /// Returns the underlying file-system error.
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}
