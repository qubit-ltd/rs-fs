// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Typed temporary persistence failure.

use std::error::Error;
use std::fmt::{
    Display,
    Formatter,
    Result as FmtResult,
};

use crate::{
    FsError,
    PersistFailureState,
};

/// Persistence error paired with provider-confirmed partial progress.
#[derive(Debug)]
pub struct PersistFailure {
    error: FsError,
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
}

impl Display for PersistFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        write!(formatter, "persist {:?}: {}", self.state, self.error)
    }
}

impl Error for PersistFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}
