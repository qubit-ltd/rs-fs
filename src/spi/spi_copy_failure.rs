// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider copy failure facts.

use crate::{
    CopyFailureState,
    CopyStats,
    FsError,
};

/// Typed provider copy failure reserved for copy orchestration.
pub struct SpiCopyFailure {
    error: Box<FsError>,
    state: CopyFailureState,
    partial_stats: CopyStats,
}

impl SpiCopyFailure {
    /// Creates a typed provider copy failure.
    ///
    /// # Parameters
    /// - `error`: Provider failure with filesystem context.
    /// - `state`: Provider-confirmed publication state.
    /// - `partial_stats`: Transfer progress confirmed before failure.
    ///
    /// # Returns
    /// A failure containing all provider-confirmed facts.
    #[inline]
    #[must_use]
    pub fn new(
        error: FsError,
        state: CopyFailureState,
        partial_stats: CopyStats,
    ) -> Self {
        Self {
            error: Box::new(error),
            state,
            partial_stats,
        }
    }

    /// Returns the provider error without exposing a recovery writer.
    ///
    /// # Returns
    /// The provider failure with filesystem context.
    #[inline(always)]
    #[must_use]
    pub fn error(&self) -> &FsError {
        &self.error
    }

    /// Returns the typed publication state.
    ///
    /// # Returns
    /// The provider-confirmed publication state.
    #[inline(always)]
    #[must_use]
    pub const fn state(&self) -> CopyFailureState {
        self.state
    }

    /// Splits this failure into its typed facts.
    ///
    /// # Returns
    /// The provider error, publication state, and partial transfer statistics.
    #[inline(always)]
    #[must_use]
    pub fn into_parts(self) -> (FsError, CopyFailureState, CopyStats) {
        (*self.error, self.state, self.partial_stats)
    }
}
