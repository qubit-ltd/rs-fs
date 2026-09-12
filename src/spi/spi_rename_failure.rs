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

use crate::error::FsError;
use crate::rename::RenameFailureState;

/// Typed provider rename failure reserved for rename orchestration.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
/// use qubit_fs::rename::RenameFailureState;
/// use qubit_fs::spi::SpiRenameFailure;
///
/// let failure = SpiRenameFailure::new(
///     FsError::new(FsErrorKind::NotFound, FsOperation::Rename, "missing"),
///     RenameFailureState::Unchanged,
/// );
/// assert_eq!(RenameFailureState::Unchanged, failure.state());
/// ```
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

    /// Returns the provider failure with filesystem context.
    ///
    /// # Returns
    /// A borrowed view of the contextual [`FsError`].
    #[inline]
    #[must_use]
    pub fn error(&self) -> &FsError {
        &self.error
    }

    /// Returns the typed rename state.
    ///
    /// # Returns
    /// The provider-confirmed rename state.
    #[inline]
    #[must_use]
    pub const fn state(&self) -> RenameFailureState {
        self.state
    }

    /// Returns the contained error.
    ///
    /// # Returns
    /// The provider error and confirmed rename state.
    #[inline]
    #[must_use]
    pub fn into_parts(self) -> (FsError, RenameFailureState) {
        (*self.error, self.state)
    }
}

#[cfg(test)]
mod tests {
    use super::SpiRenameFailure;
    use crate::error::FsError;
    use crate::error::FsErrorKind;
    use crate::error::FsOperation;
    use crate::rename::RenameFailureState;

    #[test]
    fn failure_facts_are_executed_at_runtime() {
        let failure = SpiRenameFailure::new(
            FsError::new(FsErrorKind::NotFound, FsOperation::Rename, "missing source"),
            RenameFailureState::Indeterminate,
        );
        assert_eq!(failure.error().kind(), FsErrorKind::NotFound);
        assert_eq!(failure.state(), RenameFailureState::Indeterminate);

        let (error, state) = failure.into_parts();
        assert_eq!(error.kind(), FsErrorKind::NotFound);
        assert_eq!(state, RenameFailureState::Indeterminate);
    }
}
