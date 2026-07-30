// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow all -- facade integration tests exercise this API group.
//! Typed synchronous write failure.

use std::error::Error;
use std::fmt::{
    Display,
    Formatter,
    Result as FmtResult,
};

use crate::{
    FsError,
    WriteFailureState,
};

/// Write error paired with provider-confirmed publication progress.
#[derive(Debug)]
pub struct WriteFailure {
    /// Contextual filesystem error returned by the write attempt.
    error: FsError,
    /// Provider-confirmed publication and recovery state.
    state: WriteFailureState,
}

impl WriteFailure {
    /// Creates a typed write failure.
    ///
    /// # Parameters
    /// - `error`: Underlying filesystem failure.
    /// - `state`: Confirmed publication and recovery state.
    ///
    /// # Returns
    /// A failure preserving both the cause and recovery contract.
    #[inline]
    #[must_use]
    pub fn new(error: FsError, state: WriteFailureState) -> Self {
        Self { error, state }
    }

    /// Returns provider-confirmed publication progress.
    ///
    /// # Returns
    /// The recovery state for this commit attempt.
    #[inline]
    #[must_use]
    pub const fn state(&self) -> WriteFailureState {
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
    pub fn into_parts(self) -> (FsError, WriteFailureState) {
        (self.error, self.state)
    }
}

impl Display for WriteFailure {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        write!(formatter, "write {:?}: {}", self.state, self.error)
    }
}

impl Error for WriteFailure {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}
