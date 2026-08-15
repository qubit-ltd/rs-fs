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
use crate::temp::PersistFailureState;

/// Persistence error paired with provider-confirmed partial progress.
#[derive(Debug)]
pub struct PersistFailure {
    /// Contextual filesystem error that interrupted persistence.
    error: FsError,
    /// Provider-confirmed source and destination progress.
    state: PersistFailureState,
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
    #[must_use]
    pub fn new(error: FsError, state: PersistFailureState) -> Self {
        Self { error, state }
    }

    /// Returns provider-confirmed partial progress.
    ///
    /// # Returns
    /// The recovery state for this persist attempt.
    #[inline]
    #[must_use]
    pub const fn state(&self) -> PersistFailureState {
        self.state
    }

    /// Returns the underlying filesystem error.
    ///
    /// # Returns
    /// The error with operation, path, provider, and source context.
    #[inline]
    #[must_use]
    pub const fn error(&self) -> &FsError {
        &self.error
    }

    /// Consumes this failure and returns the underlying filesystem error.
    ///
    /// # Returns
    /// The owned filesystem error.
    #[inline]
    #[must_use]
    pub fn into_error(self) -> FsError {
        self.error
    }

    /// Splits this facade failure into its causal error and state.
    #[inline(always)]
    #[must_use]
    pub fn into_parts(self) -> (FsError, PersistFailureState) {
        (self.error, self.state)
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
