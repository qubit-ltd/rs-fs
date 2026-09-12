// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider write failure facts.

use crate::error::FsError;
use crate::write::WriteFailureState;

/// Typed provider write failure preserving recovery state.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
/// use qubit_fs::spi::SpiWriteFailure;
/// use qubit_fs::write::WriteFailureState;
///
/// let failure = SpiWriteFailure::new(
///     FsError::new(FsErrorKind::Io, FsOperation::Write, "failed"),
///     WriteFailureState::NotPublished,
/// );
/// assert_eq!(WriteFailureState::NotPublished, failure.state());
/// ```
pub struct SpiWriteFailure {
    /// Provider failure with filesystem context.
    error: FsError,
    /// Provider-confirmed publication and recovery state.
    state: WriteFailureState,
}

impl SpiWriteFailure {
    /// Creates a typed provider write failure.
    ///
    /// # Parameters
    /// - `error`: Provider failure with filesystem context.
    /// - `state`: Provider-confirmed publication state.
    ///
    /// # Returns
    /// A failure containing both facts.
    #[inline]
    #[must_use]
    pub fn new(error: FsError, state: WriteFailureState) -> Self {
        Self { error, state }
    }

    /// Returns the underlying error.
    ///
    /// # Returns
    /// The provider failure with filesystem context.
    #[inline]
    #[must_use]
    pub const fn error(&self) -> &FsError {
        &self.error
    }

    /// Returns confirmed publication state.
    ///
    /// # Returns
    /// The provider-confirmed publication state.
    #[inline]
    #[must_use]
    pub const fn state(&self) -> WriteFailureState {
        self.state
    }

    /// Returns owned failure parts.
    ///
    /// # Returns
    /// The provider error and confirmed publication state.
    #[inline]
    #[must_use]
    pub fn into_parts(self) -> (FsError, WriteFailureState) {
        (self.error, self.state)
    }
}

#[cfg(test)]
mod tests {
    use super::SpiWriteFailure;
    use crate::error::FsError;
    use crate::error::FsErrorKind;
    use crate::error::FsOperation;
    use crate::write::WriteFailureState;

    #[test]
    fn failure_facts_are_executed_at_runtime() {
        let failure = SpiWriteFailure::new(
            FsError::new(FsErrorKind::Io, FsOperation::CommitWriter, "commit failed"),
            WriteFailureState::Published,
        );
        assert_eq!(failure.error().kind(), FsErrorKind::Io);
        assert_eq!(failure.state(), WriteFailureState::Published);

        let (error, state) = failure.into_parts();
        assert_eq!(error.kind(), FsErrorKind::Io);
        assert_eq!(state, WriteFailureState::Published);
    }
}
