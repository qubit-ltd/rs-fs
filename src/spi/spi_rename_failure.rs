// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider rename failure facts.

use crate::{
    FsError,
    RenameFailureState,
};

/// Typed provider rename failure reserved for rename orchestration.
pub struct SpiRenameFailure {
    /// Provider failure with filesystem context.
    error: Box<FsError>,
    /// Provider-confirmed source and destination transition state.
    state: RenameFailureState,
}

impl SpiRenameFailure {
    /// Creates a typed provider rename failure.
    ///
    /// # Parameters
    /// - `error`: Provider failure with filesystem context.
    /// - `state`: Provider-confirmed rename state.
    ///
    /// # Returns
    /// A failure containing both facts.
    #[inline]
    #[must_use]
    pub fn new(error: FsError, state: RenameFailureState) -> Self {
        Self {
            error: Box::new(error),
            state,
        }
    }

    /// Returns the error.
    ///
    /// # Returns
    /// The provider failure with filesystem context.
    #[inline(always)]
    #[must_use]
    pub fn error(&self) -> &FsError {
        &self.error
    }

    /// Returns the typed rename state.
    ///
    /// # Returns
    /// The provider-confirmed rename state.
    #[inline(always)]
    #[must_use]
    pub const fn state(&self) -> RenameFailureState {
        self.state
    }

    /// Returns the contained error.
    ///
    /// # Returns
    /// The provider error and confirmed rename state.
    #[inline(always)]
    #[must_use]
    pub fn into_parts(self) -> (FsError, RenameFailureState) {
        (*self.error, self.state)
    }
}
