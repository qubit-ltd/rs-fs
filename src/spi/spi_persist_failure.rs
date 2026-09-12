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

use crate::error::FsError;
use crate::temp::PersistFailureState;

/// Typed provider persist failure preserving partial publication state.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
/// use qubit_fs::spi::SpiPersistFailure;
/// use qubit_fs::temp::PersistFailureState;
///
/// let failure = SpiPersistFailure::new(
///     FsError::new(FsErrorKind::Io, FsOperation::PersistTemp, "failed"),
///     PersistFailureState::NotPublished,
/// );
/// assert_eq!(PersistFailureState::NotPublished, failure.state());
/// ```
pub struct SpiPersistFailure {
    /// Provider failure with filesystem context.
    error: FsError,
    /// Provider-confirmed persistence progress.
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
    #[inline]
    #[must_use]
    pub const fn error(&self) -> &FsError {
        &self.error
    }

    /// Returns confirmed persistence state.
    ///
    /// # Returns
    /// The provider-confirmed persistence state.
    #[inline]
    #[must_use]
    pub const fn state(&self) -> PersistFailureState {
        self.state
    }

    /// Returns owned failure parts.
    ///
    /// # Returns
    /// The provider error and confirmed persistence state.
    #[inline]
    #[must_use]
    pub fn into_parts(self) -> (FsError, PersistFailureState) {
        (self.error, self.state)
    }
}

#[cfg(test)]
mod tests {
    use super::SpiPersistFailure;
    use crate::error::FsError;
    use crate::error::FsErrorKind;
    use crate::error::FsOperation;
    use crate::temp::PersistFailureState;

    #[test]
    fn failure_facts_are_executed_at_runtime() {
        let failure = SpiPersistFailure::new(
            FsError::new(FsErrorKind::AlreadyExists, FsOperation::PersistTemp, "target exists"),
            PersistFailureState::NotPublished,
        );
        assert_eq!(failure.error().kind(), FsErrorKind::AlreadyExists);
        assert_eq!(failure.state(), PersistFailureState::NotPublished);

        let (error, state) = failure.into_parts();
        assert_eq!(error.kind(), FsErrorKind::AlreadyExists);
        assert_eq!(state, PersistFailureState::NotPublished);
    }
}
