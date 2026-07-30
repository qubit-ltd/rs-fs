// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider persistence failure facts.

use crate::{
    FsError,
    PersistFailureState,
};

/// Typed provider persist failure preserving partial publication state.
pub struct SpiPersistFailure {
    error: FsError,
    state: PersistFailureState,
}

impl SpiPersistFailure {
    /// Creates a typed provider persist failure.
    ///
    /// # Parameters
    /// - `error`: Provider failure with filesystem context.
    /// - `state`: Provider-confirmed persistence state.
    ///
    /// # Returns
    /// A failure containing both facts.
    #[inline]
    #[must_use]
    pub fn new(error: FsError, state: PersistFailureState) -> Self {
        Self { error, state }
    }

    /// Returns the underlying error.
    ///
    /// # Returns
    /// The provider failure with filesystem context.
    #[inline(always)]
    #[must_use]
    pub const fn error(&self) -> &FsError {
        &self.error
    }

    /// Returns confirmed persistence state.
    ///
    /// # Returns
    /// The provider-confirmed persistence state.
    #[inline(always)]
    #[must_use]
    pub const fn state(&self) -> PersistFailureState {
        self.state
    }

    /// Returns owned failure parts.
    ///
    /// # Returns
    /// The provider error and confirmed persistence state.
    #[inline(always)]
    #[must_use]
    pub fn into_parts(self) -> (FsError, PersistFailureState) {
        (self.error, self.state)
    }
}
