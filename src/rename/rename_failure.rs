// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow all -- facade integration tests exercise this API group.
//! Typed facade rename failure.

use crate::{FsError, RenameFailureState};
use std::fmt::{Debug, Formatter, Result as FmtResult};

/// A rename failure that preserves the provider's publication-state fact.
pub struct RenameFailure {
    error: FsError,
    state: RenameFailureState,
}
impl RenameFailure {
    /// Creates a typed facade rename failure.
    #[must_use]
    pub(crate) const fn new(error: FsError, state: RenameFailureState) -> Self {
        Self { error, state }
    }
    /// Returns the contextual filesystem error.
    #[must_use]
    pub const fn error(&self) -> &FsError {
        &self.error
    }
    /// Returns the state of the source/target transition at failure.
    #[must_use]
    pub const fn state(&self) -> RenameFailureState {
        self.state
    }
    /// Splits the failure into its error and state.
    #[must_use]
    pub fn into_parts(self) -> (FsError, RenameFailureState) {
        (self.error, self.state)
    }
}
impl Debug for RenameFailure {
    /// Formats the safe typed failure facts.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("RenameFailure")
            .field("error", &self.error)
            .field("state", &self.state)
            .finish()
    }
}
