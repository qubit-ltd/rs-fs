// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Typed temporary persistence failure.

use std::error::Error;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use crate::error::FsError;
use crate::path::Path;
use crate::temp::PersistFailureState;

/// Persistence error paired with a retained snapshot of provider-confirmed
/// partial progress. That snapshot may include publication history from an
/// earlier persist attempt, not only the operation that just failed.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
/// use qubit_fs::temp::{PersistFailure, PersistFailureState};
///
/// let failure = PersistFailure::new(
///     FsError::new(FsErrorKind::Io, FsOperation::PersistTemp, "failed"),
///     PersistFailureState::NotPublished,
/// );
/// assert_eq!(PersistFailureState::NotPublished, failure.state());
/// ```
#[derive(Debug)]
#[must_use = "inspect and retain the persistence recovery state before discarding this failure"]
pub struct PersistFailure {
    /// Contextual filesystem error that interrupted persistence.
    error: FsError,
    /// Provider-confirmed source and destination progress.
    state: PersistFailureState,
    /// Destination path retained when publication reached a known target.
    publication_target: Option<Path>,
}

impl PersistFailure {
    /// Creates a typed persistence failure.
    ///
    /// # Parameters
    /// - `error`: Underlying filesystem failure.
    /// - `state`: Confirmed source and target progress.
    ///
    /// # Returns
    /// A failure preserving both the cause and recovery contract.
    #[inline]
    pub fn new(error: FsError, state: PersistFailureState) -> Self {
        Self {
            error,
            state,
            publication_target: None,
        }
    }

    /// Returns the retained provider-confirmed progress snapshot.
    ///
    /// # Returns
    /// The current retained recovery state, which may include an earlier
    /// publication fact.
    #[inline]
    #[must_use]
    pub const fn state(&self) -> PersistFailureState {
        self.state
    }

    /// Returns the underlying filesystem error.
    ///
    /// # Returns
    /// The error with operation, path, provider, and source context.
    #[must_use = "inspect the filesystem error details"]
    #[inline]
    pub const fn error(&self) -> &FsError {
        &self.error
    }

    /// Returns the most recently retained path for a positively confirmed
    /// publication, if one is known.
    ///
    /// `Some(path)` identifies that historical publication; it is not
    /// necessarily the target of the persist call that most recently failed.
    /// `None` means no confirmed publication path was retained, not that the
    /// failed operation definitely published nothing.
    ///
    /// # Returns
    /// The latest retained confirmed publication path, or `None` when no path
    /// is available in the recovery snapshot.
    #[inline]
    #[must_use]
    pub fn publication_target(&self) -> Option<&Path> {
        self.publication_target.as_ref()
    }

    /// Consumes this failure and returns only the underlying filesystem error.
    ///
    /// This discards the retained progress state and publication target. Use
    /// [`Self::into_recovery_parts`] when recovery facts may matter.
    ///
    /// # Returns
    /// The owned filesystem error.
    #[inline]
    pub fn into_error(self) -> FsError {
        self.error
    }

    /// Splits this facade failure into its causal error and state, omitting the
    /// retained publication target.
    ///
    /// The returned state may describe publication history; the target path is
    /// lost. Use [`Self::into_recovery_parts`] to retain the complete snapshot.
    #[must_use = "retain or inspect the causal error and recovery state"]
    #[inline]
    pub fn into_parts(self) -> (FsError, PersistFailureState) {
        (self.error, self.state)
    }

    /// Consumes the failure and returns its complete retained recovery
    /// snapshot.
    ///
    /// The state and optional target may describe an earlier confirmed
    /// publication, rather than the target of the persist call that just
    /// failed. Preserve all three values when reconciling publication and
    /// source cleanup.
    #[must_use = "retain or inspect the causal error and complete recovery snapshot"]
    #[inline]
    pub fn into_recovery_parts(self) -> (FsError, PersistFailureState, Option<Path>) {
        (self.error, self.state, self.publication_target)
    }

    /// Retains the destination associated with a known publication attempt.
    pub(crate) fn with_publication_target(mut self, target: Option<&Path>) -> Self {
        self.publication_target = target.cloned();
        self
    }
}

impl Display for PersistFailure {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        write!(formatter, "persist {:?}: {}", self.state, self.error)
    }
}

impl Error for PersistFailure {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}
