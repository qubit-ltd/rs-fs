// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! File writer lifecycle states.

use super::WriteFailureState;

/// Observable lifecycle state of a synchronous or asynchronous file writer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriterState {
    /// The session accepts bytes and may be committed or aborted.
    Open,
    /// Publication completed successfully.
    Committed,
    /// Publication definitely did not occur and only cleanup remains possible.
    NotPublished,
    /// Publication occurred, but provider cleanup remains possible.
    Published,
    /// The session was explicitly cancelled and cleaned up.
    Aborted,
    /// Publication or lifecycle cleanup may have occurred, but the provider
    /// cannot confirm the final state.
    Indeterminate,
}

impl WriterState {
    /// Returns the publication state to report when a commit is attempted
    /// after this writer has left the open state.
    ///
    /// This describes the destination's known historical state, rather than
    /// the validity of the repeated operation itself.
    #[inline]
    #[must_use]
    pub(crate) const fn publication_failure_state(self) -> WriteFailureState {
        match self {
            Self::Open => WriteFailureState::RetryableNotPublished,
            Self::Committed | Self::Published => WriteFailureState::Published,
            Self::NotPublished | Self::Aborted => WriteFailureState::NotPublished,
            Self::Indeterminate => WriteFailureState::Indeterminate,
        }
    }
}
